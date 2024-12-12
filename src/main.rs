use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, thread};
use std::{error::Error, io};

use clap::Parser;
use deku::ctx::Endian;
use env_logger::{Builder, Env};
use log::debug;
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use regex::Regex;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use mi::{Asm, MemoryMapping, Register};

mod gdb;
mod mi;
mod ui;

enum InputMode {
    Normal,
    Editing,
}

use std::collections::{HashMap, VecDeque};

fn resolve_home(path: &str) -> Option<PathBuf> {
    if path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            return Some(Path::new(&home).join(&path[2..]));
        }
        None
    } else {
        Some(PathBuf::from(path))
    }
}

#[derive(Debug, Clone)]
struct LimitedBuffer<T> {
    offset: usize,
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> LimitedBuffer<T> {
    fn as_slice(&self) -> &[T] {
        self.buffer.as_slices().0
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn new(capacity: usize) -> Self {
        Self { offset: 0, buffer: VecDeque::with_capacity(capacity), capacity }
    }

    fn push(&mut self, value: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(value);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Run gdb as child process from PATH
    #[arg(short, long)]
    local: bool,

    /// Connect to nc session
    ///
    /// `mkfifo gdb_sock; cat gdb_pipe | gdb --interpreter=mi | nc -l -p 12345 > gdb_pipe`
    #[arg(short, long)]
    remote: Option<SocketAddr>,

    /// Switch into 32-bit mode
    #[arg(long = "32")]
    thirty_two_bit: bool,
}

enum Mode {
    All,
    OnlyRegister,
    OnlyStack,
    OnlyInstructions,
    OnlyOutput,
    OnlyMapping,
}

// TODO: this could be split up, some of these fields
// are always set after the file is loaded in gdb
struct App {
    thirty_two_bit: Arc<Mutex<bool>>,
    filepath: Arc<Mutex<Option<PathBuf>>>,
    endian: Arc<Mutex<Option<Endian>>>,
    mode: Mode,
    input: Input,
    input_mode: InputMode,
    messages: LimitedBuffer<String>,
    memory_map: Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    current_pc: Arc<Mutex<u64>>, // TODO: replace with AtomicU64?
    output_scroll: usize,
    output: Arc<Mutex<Vec<String>>>,
    stream_output_prompt: Arc<Mutex<String>>,
    gdb_stdin: Arc<Mutex<dyn Write + Send>>,
    register_changed: Arc<Mutex<Vec<u8>>>,
    register_names: Arc<Mutex<Vec<String>>>,
    registers: Arc<Mutex<Vec<(String, Option<Register>, Vec<u64>)>>>,
    stack: Arc<Mutex<HashMap<u64, Vec<u64>>>>,
    asm: Arc<Mutex<Vec<Asm>>>,
}

impl App {
    /// Create new stream to gdb
    /// - remote: Connect to gdb via a TCP connection
    /// - local: Connect to gdb via spawning a gdb process
    ///
    ///
    /// # Returns
    /// `(gdb_stdin, App)`
    pub fn new_stream(args: Args) -> (BufReader<Box<dyn Read + Send>>, App) {
        let (reader, gdb_stdin): (BufReader<Box<dyn Read + Send>>, Arc<Mutex<dyn Write + Send>>) =
            match (&args.local, &args.remote) {
                (true, None) => {
                    let mut gdb_process = Command::new("gdb")
                        .args(["--interpreter=mi2", "--quiet", "-nx"])
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .spawn()
                        .expect("Failed to start GDB");

                    let reader = BufReader::new(
                        Box::new(gdb_process.stdout.unwrap()) as Box<dyn Read + Send>
                    );
                    let gdb_stdin = gdb_process.stdin.take().unwrap();
                    let gdb_stdin = Arc::new(Mutex::new(gdb_stdin));

                    (reader, gdb_stdin)
                }
                (false, Some(remote)) => {
                    let tcp_stream = TcpStream::connect(remote).unwrap(); // Example address
                    let reader = BufReader::new(
                        Box::new(tcp_stream.try_clone().unwrap()) as Box<dyn Read + Send>
                    );
                    let gdb_stdin = Arc::new(Mutex::new(tcp_stream.try_clone().unwrap()));

                    (reader, gdb_stdin)
                }
                _ => panic!("Invalid configuration"),
            };

        let app = App {
            thirty_two_bit: Arc::new(Mutex::new(args.thirty_two_bit)),
            filepath: Arc::new(Mutex::new(None)),
            endian: Arc::new(Mutex::new(None)),
            mode: Mode::All,
            input: Input::default(),
            input_mode: InputMode::Normal,
            messages: LimitedBuffer::new(10),
            current_pc: Arc::new(Mutex::new(0)),
            output_scroll: 0,
            memory_map: Arc::new(Mutex::new(None)),
            output: Arc::new(Mutex::new(Vec::new())),
            stream_output_prompt: Arc::new(Mutex::new(String::new())),
            register_changed: Arc::new(Mutex::new(vec![])),
            register_names: Arc::new(Mutex::new(vec![])),
            gdb_stdin,
            registers: Arc::new(Mutex::new(vec![])),
            stack: Arc::new(Mutex::new(HashMap::new())),
            asm: Arc::new(Mutex::new(Vec::new())),
        };

        (reader, app)
    }

    // Parse a "file filepath" command and save
    fn save_filepath(&mut self, val: &str) {
        let filepath: Vec<&str> = val.split_whitespace().collect();
        let filepath = resolve_home(filepath[1]).unwrap();
        // debug!("filepath: {filepath:?}");
        self.filepath = Arc::new(Mutex::new(Some(filepath)));
    }

    pub fn classify_val(&self, val: u64, filepath: &std::borrow::Cow<str>) -> (bool, bool, bool) {
        let mut is_stack = false;
        let mut is_heap = false;
        let mut is_text = false;
        if val != 0 {
            // look through, add see if the value is part of the stack
            let memory_map = self.memory_map.lock().unwrap();
            // trace!("{:02x?}", memory_map);
            if memory_map.is_some() {
                for r in memory_map.as_ref().unwrap() {
                    if r.contains(val) {
                        if r.is_stack() {
                            is_stack = true;
                            break;
                        } else if r.is_heap() {
                            is_heap = true;
                            break;
                        } else if r.is_path(filepath) {
                            // TODO(23): This could be expanded to all segments loaded in
                            // as executable
                            is_text = true;
                            break;
                        }
                    }
                }
            }
        }
        (is_stack, is_heap, is_text)
    }
}

#[derive(Debug)]
enum Written {
    /// Requested Register Value deref
    // TODO: Could this just be the register name?
    RegisterValue((String, u64)),
    /// Requested Stack Bytes
    ///
    /// None - This is the first time this is requested
    /// Some - This has alrady been read, and this is a deref, trust
    ///        the base_reg of .0
    Stack(Option<String>),
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Configure logging to a file
    let log_file = Arc::new(Mutex::new(File::create("app.log")?));
    Builder::from_env(Env::default().default_filter_or("debug"))
        .format(move |buf, record| {
            let mut log_file = log_file.lock().unwrap();
            let log_msg = format!(
                "{} [{}] - {}\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            );
            log_file.write_all(log_msg.as_bytes()).unwrap();
            writeln!(buf, "{}", log_msg.trim_end())
        })
        .target(env_logger::Target::Pipe(Box::new(std::io::sink()))) // Disable stdout/stderr
        .init();

    // Start rx thread
    let (gdb_stdout, mut app) = App::new_stream(args);

    // Setup terminal
    let mut terminal = ratatui::init();

    let filepath_arc = Arc::clone(&app.filepath);
    let thirty_two_bit_arc = Arc::clone(&app.thirty_two_bit);
    let endian_arc = Arc::clone(&app.endian);
    let gdb_stdin_arc = Arc::clone(&app.gdb_stdin);
    let current_pc_arc = Arc::clone(&app.current_pc);
    let output_arc = Arc::clone(&app.output);
    let stream_output_prompt_arc = Arc::clone(&app.stream_output_prompt);
    let register_changed_arc = Arc::clone(&app.register_changed);
    let register_names_arc = Arc::clone(&app.register_names);
    let registers_arc = Arc::clone(&app.registers);
    let memory_map_arc = Arc::clone(&app.memory_map);
    let stack_arc = Arc::clone(&app.stack);
    let asm_arc = Arc::clone(&app.asm);

    // Thread to read GDB output and parse it
    thread::spawn(move || {
        gdb::gdb_interact(
            gdb_stdout,
            thirty_two_bit_arc,
            endian_arc,
            filepath_arc,
            register_changed_arc,
            register_names_arc,
            registers_arc,
            current_pc_arc,
            stack_arc,
            asm_arc,
            gdb_stdin_arc,
            output_arc,
            stream_output_prompt_arc,
            memory_map_arc,
        )
    });

    // Run tui application
    let res = run_app(&mut terminal, &mut app);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, app))?;

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                match (&app.input_mode, key.code, &app.mode) {
                    (InputMode::Normal, KeyCode::Char('i'), _) => {
                        app.input_mode = InputMode::Editing;
                    }
                    (InputMode::Normal, KeyCode::Char('q'), _) => {
                        return Ok(());
                    }
                    (_, KeyCode::F(1), _) => {
                        app.mode = Mode::All;
                    }
                    (_, KeyCode::F(2), _) => {
                        app.mode = Mode::OnlyRegister;
                    }
                    (_, KeyCode::F(3), _) => {
                        app.mode = Mode::OnlyStack;
                    }
                    (_, KeyCode::F(4), _) => {
                        app.mode = Mode::OnlyInstructions;
                    }
                    (_, KeyCode::F(5), _) => {
                        app.mode = Mode::OnlyOutput;
                    }
                    (_, KeyCode::F(6), _) => {
                        app.mode = Mode::OnlyMapping;
                    }
                    (InputMode::Editing, KeyCode::Esc, _) => {
                        app.input_mode = InputMode::Normal;
                    }
                    (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyOutput) => {
                        let output_lock = app.output.lock().unwrap();
                        if app.output_scroll < output_lock.len().saturating_sub(1) {
                            app.output_scroll += 1;
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyOutput) => {
                        if app.output_scroll > 0 {
                            app.output_scroll -= 1;
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyOutput) => {
                        let output_lock = app.output.lock().unwrap();
                        if app.output_scroll < output_lock.len().saturating_sub(1) {
                            app.output_scroll += 50;
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyOutput) => {
                        if app.output_scroll > 50 {
                            app.output_scroll -= 50;
                        } else {
                            app.output_scroll = 0;
                        }
                    }
                    (_, KeyCode::Enter, _) => {
                        key_enter(app)?;
                    }
                    (_, KeyCode::Down, _) => {
                        key_down(app);
                    }
                    (_, KeyCode::Up, _) => {
                        key_up(app);
                    }
                    (InputMode::Editing, _, _) => {
                        app.input.handle_event(&Event::Key(key));
                    }
                    _ => (),
                }
            }
        }
    }
}

fn key_up(app: &mut App) {
    if !app.messages.buffer.is_empty() {
        if app.messages.offset < app.messages.buffer.len() {
            app.messages.offset += 1;
        }
        update_from_previous_input(app);
    } else {
        app.messages.offset = 0;
    }
}

fn key_down(app: &mut App) {
    if !app.messages.buffer.is_empty() {
        if app.messages.offset != 0 {
            app.messages.offset -= 1;
            if app.messages.offset == 0 {
                app.input.reset();
            }
        }
        update_from_previous_input(app);
    } else {
        app.messages.offset = 0;
    }
}

fn key_enter(app: &mut App) -> Result<(), io::Error> {
    if app.input.value().is_empty() {
        app.messages.offset = 0;

        let messages = app.messages.clone();
        let messages = messages.as_slice().iter();
        if let Some(val) = messages.last() {
            let mut val = val.to_owned();
            if val.starts_with("file") {
                app.save_filepath(&val);
            }
            replace_mapping_start(app, &mut val);
            replace_mapping_end(app, &mut val);
            gdb::write_mi(&app.gdb_stdin, &val);
            app.input.reset();
        }
    } else {
        app.messages.offset = 0;
        app.messages.push(app.input.value().into());
        let val = app.input.clone();
        let val = val.value();
        let mut val = val.to_owned();
        if val.starts_with("file") {
            app.save_filepath(&val);
        }
        replace_mapping_start(app, &mut val);
        replace_mapping_end(app, &mut val);
        gdb::write_mi(&app.gdb_stdin, &val);
        app.input.reset();
    }

    Ok(())
}

fn replace_mapping_start(app: &mut App, val: &mut String) {
    let memory_map = app.memory_map.lock().unwrap();
    if let Some(ref memory_map) = *memory_map {
        let pattern = Regex::new(r"\$HERETEK_MAPPING_START_([\w\[\]/.-]+)").unwrap();
        *val = pattern
            .replace_all(&*val, |caps: &regex::Captures| {
                let filename = &caps[1];
                format!(
                    "0x{:02x}",
                    memory_map
                        .iter()
                        .find(|a| a.path == filename)
                        .map(|a| a.start_address)
                        .unwrap_or(0)
                )
            })
            .to_string();
    }
}

fn replace_mapping_end(app: &mut App, val: &mut String) {
    let memory_map = app.memory_map.lock().unwrap();
    if let Some(ref memory_map) = *memory_map {
        let pattern = Regex::new(r"\$HERETEK_MAPPING_END_([\w\[\]/.-]+)").unwrap();
        *val = pattern
            .replace_all(&*val, |caps: &regex::Captures| {
                let filename = &caps[1];
                format!(
                    "0x{:02x}",
                    memory_map
                        .iter()
                        .find(|a| a.path == filename)
                        .map(|a| a.end_address)
                        .unwrap_or(0)
                )
            })
            .to_string();
    }
}

fn update_from_previous_input(app: &mut App) {
    if app.messages.buffer.len() >= app.messages.offset {
        if let Some(msg) = app.messages.buffer.get(app.messages.buffer.len() - app.messages.offset)
        {
            app.input = Input::new(msg.clone())
        }
    }
}
