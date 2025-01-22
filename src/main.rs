use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, thread};
use std::{error::Error, io};

use clap::Parser;
use crossterm::event::KeyModifiers;
use deku::ctx::Endian;
use deref::Deref;
use env_logger::{Builder, Env};
use gdb::write_mi;
use log::{debug, error};
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::widgets::ScrollbarState;
use regex::Regex;
use register::RegisterStorage;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use mi::{data_read_memory_bytes, Asm, MemoryMapping};
use ui::hexdump::HEXDUMP_WIDTH;

mod deref;
mod gdb;
mod mi;
mod register;
mod ui;

enum InputMode {
    Normal,
    Editing,
}

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

#[derive(Parser, Debug, Clone, Default)]
#[command(version, about, long_about = None)]
struct Args {
    /// Override gdb executable path
    #[arg(long)]
    gdb_path: Option<String>,

    /// Connect to nc session
    ///
    /// `mkfifo gdb_pipe; cat gdb_pipe | gdb --interpreter=mi | nc -l -p 12345 > gdb_pipe`
    #[arg(short, long)]
    remote: Option<SocketAddr>,

    /// Switch into 32-bit mode
    #[arg(long = "32")]
    thirty_two_bit: bool,

    /// Execute GDB commands
    ///
    /// lines starting with # are ignored
    #[arg(short, long)]
    cmds: Option<String>,

    /// Path to write log
    ///
    /// Set env `RUST_LOG` to change log level
    #[arg(long)]
    log_path: Option<String>,
}

enum Mode {
    All,
    OnlyRegister,
    OnlyStack,
    OnlyInstructions,
    OnlyOutput,
    OnlyMapping,
    OnlyHexdump,
    OnlyHexdumpPopup,
}

impl Mode {
    pub fn next(&self) -> Self {
        match self {
            Mode::All => Mode::OnlyRegister,
            Mode::OnlyRegister => Mode::OnlyStack,
            Mode::OnlyStack => Mode::OnlyInstructions,
            Mode::OnlyInstructions => Mode::OnlyOutput,
            Mode::OnlyOutput => Mode::OnlyMapping,
            Mode::OnlyMapping => Mode::OnlyHexdump,
            Mode::OnlyHexdump => Mode::All,
            Mode::OnlyHexdumpPopup => Mode::OnlyHexdumpPopup,
        }
    }
}

// TODO: this could be split up, some of these fields
// are always set after the file is loaded in gdb
struct App {
    /// Gdb stdin
    gdb_stdin: Arc<Mutex<dyn Write + Send>>,
    /// Messages to write to gdb mi
    next_write: Arc<Mutex<Vec<String>>>,
    /// Stack of what was written to gdb that is expected back in order to parse correctly
    written: Arc<Mutex<VecDeque<Written>>>,
    /// -32 bit mode
    thirty_two_bit: Arc<AtomicBool>,
    /// Current filepath of .text
    filepath: Arc<Mutex<Option<PathBuf>>>,
    /// Current endian
    endian: Arc<Mutex<Option<Endian>>>,
    /// Current mode
    mode: Mode,
    /// TUI input
    input: Input,
    /// Currnt input mode of tui
    input_mode: InputMode,
    /// List of previously sent commands from our own input
    sent_input: LimitedBuffer<String>,
    /// Memory map TUI
    memory_map: Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    memory_map_scroll: usize,
    memory_map_scroll_state: ScrollbarState,
    /// Current $pc
    current_pc: Arc<Mutex<u64>>, // TODO: replace with AtomicU64?
    /// All output from gdb
    output: Arc<Mutex<Vec<String>>>,
    output_scroll: usize,
    output_scroll_state: ScrollbarState,
    /// Saved output such as (gdb) or > from gdb
    stream_output_prompt: Arc<Mutex<String>>,
    /// Register TUI
    register_changed: Arc<Mutex<Vec<u8>>>,
    register_names: Arc<Mutex<Vec<String>>>,
    registers: Arc<Mutex<Vec<RegisterStorage>>>,
    /// Saved Stack
    stack: Arc<Mutex<BTreeMap<u64, Deref>>>,
    /// Saved ASM
    asm: Arc<Mutex<Vec<Asm>>>,
    /// Hexdump
    hexdump: Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    hexdump_scroll: usize,
    hexdump_scroll_state: ScrollbarState,
    hexdump_popup: Input,
    /// Right side of status in TUI
    async_result: Arc<Mutex<String>>,
    /// Left side of status in TUI
    status: Arc<Mutex<String>>,
}

impl App {
    /// Create new stream to gdb
    /// - remote: Connect to gdb via a TCP connection
    ///
    ///
    /// # Returns
    /// `(gdb_stdin, App)`
    pub fn new_stream(args: Args) -> (BufReader<Box<dyn Read + Send>>, App) {
        let (reader, gdb_stdin): (BufReader<Box<dyn Read + Send>>, Arc<Mutex<dyn Write + Send>>) =
            match &args.remote {
                None => {
                    let mut gdb_process = Command::new(args.gdb_path.unwrap_or("gdb".to_owned()))
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
                Some(remote) => {
                    let tcp_stream = TcpStream::connect(remote).unwrap();
                    let reader = BufReader::new(
                        Box::new(tcp_stream.try_clone().unwrap()) as Box<dyn Read + Send>
                    );
                    let gdb_stdin = Arc::new(Mutex::new(tcp_stream.try_clone().unwrap()));

                    (reader, gdb_stdin)
                }
            };

        let app = App {
            gdb_stdin,
            next_write: Arc::new(Mutex::new(vec![])),
            written: Arc::new(Mutex::new(VecDeque::new())),
            thirty_two_bit: Arc::new(AtomicBool::new(args.thirty_two_bit)),
            filepath: Arc::new(Mutex::new(None)),
            endian: Arc::new(Mutex::new(None)),
            mode: Mode::All,
            input: Input::default(),
            input_mode: InputMode::Normal,
            sent_input: LimitedBuffer::new(100),
            memory_map: Arc::new(Mutex::new(None)),
            memory_map_scroll: 0,
            memory_map_scroll_state: ScrollbarState::new(0),
            current_pc: Arc::new(Mutex::new(0)),
            output: Arc::new(Mutex::new(Vec::new())),
            output_scroll: 0,
            output_scroll_state: ScrollbarState::new(0),
            stream_output_prompt: Arc::new(Mutex::new(String::new())),
            register_changed: Arc::new(Mutex::new(vec![])),
            register_names: Arc::new(Mutex::new(vec![])),
            registers: Arc::new(Mutex::new(vec![])),
            stack: Arc::new(Mutex::new(BTreeMap::new())),
            asm: Arc::new(Mutex::new(Vec::new())),
            hexdump: Arc::new(Mutex::new(None)),
            hexdump_scroll: 0,
            hexdump_scroll_state: ScrollbarState::new(0),
            hexdump_popup: Input::default(),
            async_result: Arc::new(Mutex::new(String::new())),
            status: Arc::new(Mutex::new(String::new())),
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

    pub fn find_first_heap(&self) -> Option<MemoryMapping> {
        let memory_map = self.memory_map.lock().unwrap();
        if let Some(memory_map) = memory_map.clone() {
            memory_map.iter().find(|a| a.is_heap()).cloned()
        } else {
            None
        }
    }

    pub fn find_first_stack(&self) -> Option<MemoryMapping> {
        let memory_map = self.memory_map.lock().unwrap();
        if let Some(memory_map) = memory_map.clone() {
            memory_map.iter().find(|a| a.is_stack()).cloned()
        } else {
            None
        }
    }

    pub fn classify_val(&self, val: u64, filepath: &str) -> (bool, bool, bool) {
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
                        } else if r.is_path(filepath) || r.is_exec() {
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
    /// Requested Memory Read (for hexdump)
    Memory,
    /// Requested Asm At $pc
    AsmAtPc,
    /// Requested symbol at addr for register (from deref)
    SymbolAtAddrRegister((String, u64)),
    /// Requested symbol at addr for stack (from deref)
    SymbolAtAddrStack(String),
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    init_logging(&args.log_path)?;

    // Start rx thread
    let (gdb_stdout, mut app) = App::new_stream(args.clone());

    // Setup terminal
    let mut terminal = ratatui::init();

    into_gdb(&app, gdb_stdout);

    if let Some(cmds) = args.cmds {
        let data = fs::read_to_string(cmds).unwrap();
        for cmd in data.lines() {
            if !cmd.starts_with("#") {
                app.sent_input.push(cmd.to_string());
                process_line(&mut app, &cmd);
            }
        }
    }

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

fn init_logging(log_path: &Option<String>) -> Result<(), Box<dyn Error>> {
    if let Some(log_path) = log_path {
        let log_file = Arc::new(Mutex::new(File::create(log_path)?));
        Builder::from_env(Env::default().default_filter_or("info"))
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
    }
    Ok(())
}

fn into_gdb(app: &App, gdb_stdout: BufReader<Box<dyn Read + Send>>) {
    // Clone all the variables that need to be accessed into gdb thread
    let filepath_arc = Arc::clone(&app.filepath);
    let thirty_two_bit_arc = Arc::clone(&app.thirty_two_bit);
    let next_write_arc = Arc::clone(&app.next_write);
    let written_arc = Arc::clone(&app.written);
    let endian_arc = Arc::clone(&app.endian);
    let current_pc_arc = Arc::clone(&app.current_pc);
    let output_arc = Arc::clone(&app.output);
    let stream_output_prompt_arc = Arc::clone(&app.stream_output_prompt);
    let register_changed_arc = Arc::clone(&app.register_changed);
    let register_names_arc = Arc::clone(&app.register_names);
    let registers_arc = Arc::clone(&app.registers);
    let memory_map_arc = Arc::clone(&app.memory_map);
    let stack_arc = Arc::clone(&app.stack);
    let asm_arc = Arc::clone(&app.asm);
    let hexdump_arc = Arc::clone(&app.hexdump);
    let async_result_arc = Arc::clone(&app.async_result);

    // Thread to read GDB output and parse it
    thread::spawn(move || {
        gdb::gdb_interact(
            gdb_stdout,
            next_write_arc,
            written_arc,
            thirty_two_bit_arc,
            endian_arc,
            filepath_arc,
            register_changed_arc,
            register_names_arc,
            registers_arc,
            current_pc_arc,
            stack_arc,
            asm_arc,
            output_arc,
            stream_output_prompt_arc,
            memory_map_arc,
            hexdump_arc,
            async_result_arc,
        )
    });
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, app))?;

        // check and see if we need to write to GBD MI
        {
            let mut next_write = app.next_write.lock().unwrap();
            if !next_write.is_empty() {
                for w in &*next_write {
                    write_mi(&app.gdb_stdin, w);
                }
                next_write.clear();
            }
        }

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    gdb::write_mi(&app.gdb_stdin, "-exec-interrupt");
                    continue;
                }
                match (&app.input_mode, key.code, &app.mode) {
                    // hexdump popup
                    (_, KeyCode::Esc, Mode::OnlyHexdumpPopup) => {
                        app.hexdump_popup = Input::default();
                        app.mode = Mode::OnlyHexdump;
                    }
                    (_, KeyCode::Char('S'), Mode::OnlyHexdumpPopup) => {
                        app.input.handle_event(&Event::Key(key));
                    }
                    (_, KeyCode::Enter, Mode::OnlyHexdumpPopup) => {
                        let val = app.hexdump_popup.clone();
                        let val = val.value();

                        let hexdump_lock = app.hexdump.lock().unwrap();
                        if let Some(hexdump) = hexdump_lock.as_ref() {
                            if let Some(path) = resolve_home(val) {
                                if std::fs::write(&path, &hexdump.1).is_ok() {
                                    let mut output_lock = app.output.lock().unwrap();
                                    output_lock.push(format!(
                                        "h> hexdump succesfully written to {}",
                                        path.to_str().unwrap()
                                    ));
                                }
                            }
                        }
                        app.hexdump_popup = Input::default();
                        app.mode = Mode::OnlyHexdump;
                    }
                    (_, _, Mode::OnlyHexdumpPopup) => {
                        app.hexdump_popup.handle_event(&Event::Key(key));
                    }
                    // Input
                    (InputMode::Normal, KeyCode::Char('i'), _) => {
                        app.input_mode = InputMode::Editing;
                    }
                    (InputMode::Normal, KeyCode::Char('q'), _) => {
                        return Ok(());
                    }
                    // Modes
                    (InputMode::Normal, KeyCode::Tab, _) => {
                        app.mode = app.mode.next();
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
                    (_, KeyCode::F(7), _) => {
                        app.mode = Mode::OnlyHexdump;
                    }
                    (InputMode::Editing, KeyCode::Esc, _) => {
                        app.input_mode = InputMode::Normal;
                    }
                    // output
                    (InputMode::Normal, KeyCode::Char('g'), Mode::OnlyOutput) => {
                        app.output_scroll = 0;
                        app.output_scroll_state = app.output_scroll_state.position(0);
                    }
                    (InputMode::Normal, KeyCode::Char('G'), Mode::OnlyOutput) => {
                        let output_lock = app.output.lock().unwrap();
                        let len = output_lock.len();
                        app.output_scroll = len;
                        app.output_scroll_state.last();
                    }
                    (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyOutput) => {
                        let output_lock = app.output.lock().unwrap();
                        let len = output_lock.len();
                        scroll_down(1, &mut app.output_scroll, &mut app.output_scroll_state, len);
                    }
                    (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyOutput) => {
                        scroll_up(1, &mut app.output_scroll, &mut app.output_scroll_state);
                    }
                    (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyOutput) => {
                        let output_lock = app.output.lock().unwrap();
                        let len = output_lock.len();
                        scroll_down(50, &mut app.output_scroll, &mut app.output_scroll_state, len);
                    }
                    (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyOutput) => {
                        scroll_up(50, &mut app.output_scroll, &mut app.output_scroll_state);
                    }
                    // memory mapping
                    (InputMode::Normal, KeyCode::Char('g'), Mode::OnlyMapping) => {
                        app.memory_map_scroll = 0;
                        app.memory_map_scroll_state = app.memory_map_scroll_state.position(0);
                    }
                    (InputMode::Normal, KeyCode::Char('G'), Mode::OnlyMapping) => {
                        let memory_lock = app.memory_map.lock().unwrap();
                        if let Some(memory) = memory_lock.as_ref() {
                            let len = memory.len();
                            app.memory_map_scroll = len;
                            app.memory_map_scroll_state.last();
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyMapping) => {
                        let memory_lock = app.memory_map.lock().unwrap();
                        if let Some(memory) = memory_lock.as_ref() {
                            let len = memory.len() / HEXDUMP_WIDTH;
                            scroll_down(
                                1,
                                &mut app.memory_map_scroll,
                                &mut app.memory_map_scroll_state,
                                len,
                            );
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyMapping) => {
                        scroll_up(1, &mut app.memory_map_scroll, &mut app.memory_map_scroll_state);
                    }
                    (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyMapping) => {
                        let memory_lock = app.memory_map.lock().unwrap();
                        if let Some(memory) = memory_lock.as_ref() {
                            let len = memory.len() / HEXDUMP_WIDTH;
                            scroll_down(
                                50,
                                &mut app.memory_map_scroll,
                                &mut app.memory_map_scroll_state,
                                len,
                            );
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyMapping) => {
                        scroll_up(50, &mut app.memory_map_scroll, &mut app.memory_map_scroll_state);
                    }
                    // hexdump
                    (InputMode::Normal, KeyCode::Char('g'), Mode::OnlyHexdump) => {
                        app.hexdump_scroll = 0;
                        app.hexdump_scroll_state = app.hexdump_scroll_state.position(0);
                    }
                    (InputMode::Normal, KeyCode::Char('G'), Mode::OnlyHexdump) => {
                        let hexdump = app.hexdump.lock().unwrap();
                        if let Some(hexdump) = hexdump.as_ref() {
                            let len = hexdump.1.len() / HEXDUMP_WIDTH;
                            app.hexdump_scroll = len;
                            app.hexdump_scroll_state.last();
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('S'), Mode::OnlyHexdump) => {
                        app.mode = Mode::OnlyHexdumpPopup;
                    }
                    (InputMode::Normal, KeyCode::Char('H'), Mode::OnlyHexdump) => {
                        if let Some(find_heap) = app.find_first_heap() {
                            let s =
                                data_read_memory_bytes(find_heap.start_address, 0, find_heap.size);
                            let mut next_write = app.next_write.lock().unwrap();
                            let mut written = app.written.lock().unwrap();
                            next_write.push(s);
                            written.push_back(Written::Memory);

                            // reset position
                            app.hexdump_scroll = 0;
                            app.hexdump_scroll_state = app.hexdump_scroll_state.position(0);
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('T'), Mode::OnlyHexdump) => {
                        if let Some(find_heap) = app.find_first_stack() {
                            let s =
                                data_read_memory_bytes(find_heap.start_address, 0, find_heap.size);
                            let mut next_write = app.next_write.lock().unwrap();
                            let mut written = app.written.lock().unwrap();
                            next_write.push(s);
                            written.push_back(Written::Memory);

                            // reset position
                            app.hexdump_scroll = 0;
                            app.hexdump_scroll_state = app.hexdump_scroll_state.position(0);
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyHexdump) => {
                        let hexdump = app.hexdump.lock().unwrap();
                        if let Some(hexdump) = hexdump.as_ref() {
                            let len = hexdump.1.len() / HEXDUMP_WIDTH;
                            scroll_down(
                                1,
                                &mut app.hexdump_scroll,
                                &mut app.hexdump_scroll_state,
                                len,
                            );
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyHexdump) => {
                        scroll_up(1, &mut app.hexdump_scroll, &mut app.hexdump_scroll_state);
                    }
                    (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyHexdump) => {
                        let hexdump = app.hexdump.lock().unwrap();
                        if let Some(hexdump) = hexdump.as_ref() {
                            let len = hexdump.1.len() / HEXDUMP_WIDTH;
                            scroll_down(
                                50,
                                &mut app.hexdump_scroll,
                                &mut app.hexdump_scroll_state,
                                len,
                            );
                        }
                    }
                    (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyHexdump) => {
                        scroll_up(50, &mut app.hexdump_scroll, &mut app.hexdump_scroll_state);
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

fn scroll_down(n: usize, scroll: &mut usize, state: &mut ScrollbarState, len: usize) {
    if scroll < &mut len.saturating_sub(1) {
        *scroll += n;
        *state = state.position(*scroll);
    }
}

fn scroll_up(n: usize, scroll: &mut usize, state: &mut ScrollbarState) {
    if *scroll > n {
        *scroll -= n;
    } else {
        *scroll = 0;
    }
    *state = state.position(*scroll);
}

fn key_up(app: &mut App) {
    if !app.sent_input.buffer.is_empty() {
        if app.sent_input.offset < app.sent_input.buffer.len() {
            app.sent_input.offset += 1;
        }
        update_from_previous_input(app);
    } else {
        app.sent_input.offset = 0;
    }
}

fn key_down(app: &mut App) {
    if !app.sent_input.buffer.is_empty() {
        if app.sent_input.offset != 0 {
            app.sent_input.offset -= 1;
            if app.sent_input.offset == 0 {
                app.input.reset();
            }
        }
        update_from_previous_input(app);
    } else {
        app.sent_input.offset = 0;
    }
}

fn key_enter(app: &mut App) -> Result<(), io::Error> {
    if app.input.value().is_empty() {
        app.sent_input.offset = 0;

        let messages = app.sent_input.clone();
        let messages = messages.as_slice().iter();
        if let Some(val) = messages.last() {
            process_line(app, val);
        }
    } else {
        app.sent_input.offset = 0;
        app.sent_input.push(app.input.value().into());

        let val = app.input.clone();
        let val = val.value();
        process_line(app, val)
    }

    Ok(())
}

fn process_line(app: &mut App, val: &str) {
    let mut val = val.to_owned();

    // Replace internal variables
    replace_internal_variables(app, &mut val);

    // Resolve parens with expresions
    resolve_paren_expressions(&mut val);

    if val == "r" || val == "ru" || val == "run" {
        // Replace run with -exec-run and target-async
        // This is to allow control+C to interrupt
        // gdb::write_mi(&app.gdb_stdin, "-gdb-set target-async on");

        let cmd = "-gdb-set mi-async on";
        let mut output_lock = app.output.lock().unwrap();
        output_lock.push(format!("h> {cmd}"));
        gdb::write_mi(&app.gdb_stdin, cmd);

        let cmd = "-exec-run";
        gdb::write_mi(&app.gdb_stdin, cmd);
        output_lock.push(val);

        app.input.reset();
        return;
    } else if val == "c"
        || val == "co"
        || val == "con"
        || val == "cont"
        || val == "conti"
        || val == "continu"
        || val == "continue"
    {
        let cmd = "-exec-continue";
        gdb::write_mi(&app.gdb_stdin, cmd);
        let mut output_lock = app.output.lock().unwrap();
        output_lock.push(val);

        app.input.reset();
        return;
    } else if val == "si" || val == "stepi" {
        let cmd = "-exec-step-instruction";
        gdb::write_mi(&app.gdb_stdin, cmd);
        let mut output_lock = app.output.lock().unwrap();
        output_lock.push(val);

        app.input.reset();
        return;
    } else if val == "step" {
        let cmd = "-exec-step";
        gdb::write_mi(&app.gdb_stdin, cmd);
        let mut output_lock = app.output.lock().unwrap();
        output_lock.push(val);

        app.input.reset();
        return;
    } else if val.starts_with("file") {
        // we parse file, but still send it on
        app.save_filepath(&val);
    } else if val.starts_with("hexdump") {
        debug!("hexdump: {val}");
        // don't send it on, parse the hexdump command
        let split: Vec<&str> = val.split_whitespace().collect();
        if split.len() < 3 {
            error!("Invalid arguments, expected 'hexdump addr len'");
            return;
        }
        let mut next_write = app.next_write.lock().unwrap();
        let mut written = app.written.lock().unwrap();
        let addr = split[1];
        let len = split[2];

        let addr_val = if addr.starts_with("0x") {
            u64::from_str_radix(&addr[2..], 16).unwrap()
        } else {
            addr.parse::<u64>().unwrap()
        };

        let len_val = if len.starts_with("0x") {
            u64::from_str_radix(&len[2..], 16).unwrap()
        } else {
            len.parse::<u64>().unwrap()
        };

        let s = data_read_memory_bytes(addr_val, 0, len_val);
        next_write.push(s);
        written.push_back(Written::Memory);
        app.input.reset();
        return;
    }
    gdb::write_mi(&app.gdb_stdin, &val);
    app.input.reset();
}

fn resolve_paren_expressions(val: &mut String) {
    static RE_PAREN: once_cell::sync::Lazy<Regex> =
        once_cell::sync::Lazy::new(|| Regex::new(r"\(([^()]+)\)").unwrap());

    *val = RE_PAREN
        .replace_all(&*val, |caps: &regex::Captures| {
            let expression = &caps[1];
            match evalexpr::eval(expression) {
                Ok(result) => result.to_string(),
                Err(_) => expression.to_string(),
            }
        })
        .to_string();
}

enum MappingType {
    Start,
    End,
    Len,
}

impl MappingType {
    fn env_start(&self) -> &str {
        match self {
            MappingType::Start => "$HERETEK_MAPPING_START_",
            MappingType::End => "$HERETEK_MAPPING_END_",
            MappingType::Len => "$HERETEK_MAPPING_LEN_",
        }
    }
}

fn replace_internal_variables(app: &mut App, line: &mut String) {
    replace_mapping(app, line, MappingType::Start);
    replace_mapping(app, line, MappingType::End);
    replace_mapping(app, line, MappingType::Len);
}

fn replace_mapping(app: &mut App, text: &mut String, mt: MappingType) {
    let ret = find_mapping(text, &mt);
    if let Some((path, prefix, start_idx, end_idx)) = ret {
        let memory_map = app.memory_map.lock().unwrap();
        if let Some(ref memory_map) = *memory_map {
            let resolve =
                memory_map.iter().filter(|a| a.path == Some(path.to_owned())).nth(prefix as usize);
            let addr = match mt {
                MappingType::Start => resolve.map(|a| a.start_address),
                MappingType::End => resolve.map(|a| a.end_address),
                MappingType::Len => resolve.map(|a| a.size),
            };
            if let Some(addr) = addr {
                text.replace_range(start_idx..end_idx, &format!("{:#08x?}", addr));
            }
        }
    }
}

fn find_mapping(text: &mut String, mt: &MappingType) -> Option<(String, u32, usize, usize)> {
    let start = mt.env_start();
    let ret = if let Some(start_idx) = text.find(start) {
        let prefix_len = start.len();
        let end_idx =
            text[start_idx..].find(' ').unwrap_or_else(|| text.len() - start_idx) + start_idx;

        let content = &text[start_idx + prefix_len..end_idx];

        let (prefix, path) = if let Some((prefix, path)) = content.split_once('_') {
            if prefix.chars().all(char::is_numeric) {
                (Some(prefix.to_string()), path.to_string())
            } else {
                (None, content.to_string())
            }
        } else {
            (None, content.to_string())
        };

        let prefix = u32::from_str_radix(&prefix.unwrap_or("0".to_string()), 10).unwrap();

        Some((path, prefix, start_idx, end_idx))
    } else {
        None
    };
    ret
}

fn update_from_previous_input(app: &mut App) {
    if app.sent_input.buffer.len() >= app.sent_input.offset {
        if let Some(msg) =
            app.sent_input.buffer.get(app.sent_input.buffer.len() - app.sent_input.offset)
        {
            app.input = Input::new(msg.clone())
        }
    }
}

// Now in tests module:
#[cfg(test)]
mod tests {
    use std::{ffi::CString, time::Instant};

    use super::*;
    use insta::assert_snapshot;
    use libc::{chmod, S_IRGRP, S_IROTH, S_IRUSR, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};

    use ratatui::{backend::TestBackend, Terminal};
    use test_assets_ureq::{dl_test_files_backoff, TestAssetDef};

    fn run_a_bit(args: Args) -> (App, Terminal<TestBackend>) {
        let (gdb_stdout, mut app) = App::new_stream(args.clone());
        into_gdb(&app, gdb_stdout);

        if let Some(cmds) = args.cmds {
            let data = fs::read_to_string(cmds).unwrap();
            for cmd in data.lines() {
                if !cmd.starts_with("#") {
                    process_line(&mut app, &cmd);
                }
            }
        }
        let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
        let start_time = Instant::now();
        let duration = Duration::from_secs(10);

        while Instant::now() - start_time < duration {
            terminal.draw(|f| ui::ui(f, &mut app)).unwrap();

            // check and see if we need to write to GBD MI
            {
                let mut next_write = app.next_write.lock().unwrap();
                if !next_write.is_empty() {
                    for w in &*next_write {
                        write_mi(&app.gdb_stdin, w);
                    }
                    next_write.clear();
                }
            }
        }

        (app, terminal)
    }

    #[test]
    fn test_repeated_ptr() {
        // gcc repeated.c -g -fno-stack-protector -static
        // repeated.c
        // ```c
        // #include <stdio.h>
        // int this() {
        //   return 0;
        // }
        //
        // int main() {
        //     int *ptr, *ptr2, *ptr3, *ptr4;
        //
        //     ptr = (int*)&ptr2;    // ptr points to ptr2
        //     ptr2 = (int*)&ptr3;   // ptr2 points to ptr3
        //     ptr3 = (int*)&ptr4;   // ptr2 points to ptr3
        //     ptr4 = (int*)&ptr;    // ptr3 points back to ptr
        //
        //     printf("Address of ptr: %p\n", (void*)ptr);
        //
        //     this();
        //     return 0;
        // }
        // ```
        const FILE_NAME: &str = "a.out";
        const TEST_PATH: &str = "test-assets/test_repeated_ptr/";
        let file_path = format!("{TEST_PATH}/{FILE_NAME}");
        let asset_defs = [TestAssetDef {
            filename: FILE_NAME.to_string(),
            hash: "ccbde92a79b40bdd07c620b47c4f21af7ca447f93839807b243d225e05e9025d".to_string(),
            url: "https://wcampbell.dev/heretek/test_repeated_ptr/a.out".to_string(),
        }];

        dl_test_files_backoff(&asset_defs, TEST_PATH, true, Duration::from_secs(1)).unwrap();
        let c_path = CString::new(file_path.to_string()).expect("CString::new failed");
        let mode = S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
        unsafe { chmod(c_path.as_ptr(), mode) };

        let mut args = Args::default();
        args.cmds = Some("test-sources/repeated_ptr.source".to_string());

        let (app, terminal) = run_a_bit(args);
        let _output = terminal.backend();
        let registers = app.registers.lock().unwrap();
        let stack = app.stack.lock().unwrap();

        // rsi repeating
        assert!(registers[4].deref.repeated_pattern);

        // stack repeating
        let mut stack: Vec<_> = stack.clone().into_iter().collect();
        stack.sort_by(|a, b| a.0.cmp(&b.0));
        assert!(stack[2].1.repeated_pattern);
        assert!(stack[3].1.repeated_pattern);
        assert!(stack[4].1.repeated_pattern);
        assert!(stack[5].1.repeated_pattern);
    }

    #[test]
    fn test_render_app() {
        // gcc test.c -g -fno-stack-protector -static
        // test.c
        // ```c
        // #include <stdio.h>
        // #include <unistd.h>
        // #include <stdint.h>
        //
        // void this(void) {
        //     sleep(10);
        //     printf("what\n");
        // }
        //
        // int main(void) {
        //     volatile uint64_t val1 = 0x11111111;
        //     volatile uint64_t val2 = 0x22222222;
        //     volatile uint64_t val3 = 0x33333333;
        //     volatile uint64_t val4 = 0x44444444;
        //     volatile uint64_t val5 = 0x55555555;
        //     volatile uint64_t val6 = 0x66666666;
        //     volatile uint64_t val7 = 0x77777777;
        //     volatile uint64_t val8 = 0x88888887;
        //     while (1) {
        //         this();
        //     }
        // }
        // ```
        const FILE_NAME: &str = "a.out";
        const TEST_PATH: &str = "test-assets/test_render_app/";
        let file_path = format!("{TEST_PATH}/{FILE_NAME}");
        let asset_defs = [TestAssetDef {
            filename: FILE_NAME.to_string(),
            hash: "ecda3a4b9eac62c1cae84184710238b2b4ae5c41e6fa94e1df4b1125b7bf0084".to_string(),
            url: "https://wcampbell.dev/heretek/test_render_app/a.out".to_string(),
        }];

        dl_test_files_backoff(&asset_defs, TEST_PATH, true, Duration::from_secs(1)).unwrap();
        let c_path = CString::new(file_path.to_string()).expect("CString::new failed");
        let mode = S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
        unsafe { chmod(c_path.as_ptr(), mode) };

        let mut args = Args::default();
        args.cmds = Some("test-sources/test.source".to_string());

        let (app, terminal) = run_a_bit(args);
        let output = terminal.backend();

        // Now, we need to rewrite all the addresses that change for the registers and stack
        // this makes this work for any (hopefully) computer that runs these commands.
        // I'm not in love with this testing plan! If this becomes a problem, these
        // could be removed.
        let output = output.to_string();
        if let Ok(stack) = app.stack.lock() {
            let mut entries: Vec<_> = stack.clone().into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let first_stack = entries[0].0;
            let from = format!("0x{:02x}", first_stack);
            let output = output.replace(&from, "<stack_0>");

            let from = format!("0x{:02x}", first_stack + 8);
            let output = output.replace(&from, "<stack_1>");

            let from = format!("0x{:02x}", first_stack + 16);
            let output = output.replace(&from, "<stack_2>");

            let from = format!("0x{:02x}", first_stack + 24);
            let output = output.replace(&from, "<stack_3>");

            let from = format!("0x{:02x}", first_stack + 32);
            let output = output.replace(&from, "<stack_4>");

            let from = format!("0x{:02x}", first_stack + 40);
            let output = output.replace(&from, "<stack_5>");

            let from = format!("0x{:02x}", first_stack + 48);
            let output = output.replace(&from, "<stack_6>");
            let from = format!("0x{:02x}", entries[6].1.map[0]);
            let output = output.replace(&from, "<stack_6_0>   ");
            let from = format!("0x{:02x}", entries[6].1.map[1]);
            let output = output.replace(&from, "<stack_6_1>   ");

            let from = format!("0x{:02x}", first_stack + 56);
            let output = output.replace(&from, "<stack_7>");

            let from = format!("0x{:02x}", first_stack + 64);
            let output = output.replace(&from, "<stack_8>");

            let from = format!("0x{:02x}", first_stack + 72);
            let output = output.replace(&from, "<stack_9>");

            let from = format!("0x{:02x}", first_stack + 80);
            let output = output.replace(&from, "<stack_10>");

            let from = format!("0x{:02x}", first_stack + 88);
            let output = output.replace(&from, "<stack_11>");

            let from = format!("0x{:02x}", first_stack + 96);
            let output = output.replace(&from, "<stack_12>");

            let from = format!("0x{:02x}", first_stack + 104);
            let output = output.replace(&from, "<stack_13>");

            let from = format!("0x{:02x}", first_stack + 112);
            let output = output.replace(&from, "<stack_14>");

            if let Ok(registers) = app.registers.lock() {
                let from = format!(
                    "0x{:02x}",
                    u64::from_str_radix(
                        &registers[2].register.as_ref().unwrap().value.as_ref().unwrap()[2..],
                        16
                    )
                    .unwrap()
                );
                let output = output.replace(&from, "<rcx_0>");

                let from = format!(
                    "0x{:02x}",
                    u64::from_str_radix(
                        &registers[3].register.as_ref().unwrap().value.as_ref().unwrap()[2..],
                        16
                    )
                    .unwrap()
                );
                let output = output.replace(&from, "<rdx_0>");

                let from = format!(
                    "0x{:02x}",
                    u64::from_str_radix(
                        &registers[4].register.as_ref().unwrap().value.as_ref().unwrap()[2..],
                        16
                    )
                    .unwrap()
                );
                let output = output.replace(&from, "<rsi_0>");

                let from = format!(
                    "0x{:02x}",
                    u64::from_str_radix(
                        &registers[6].register.as_ref().unwrap().value.as_ref().unwrap()[2..],
                        16
                    )
                    .unwrap()
                );
                let output = output.replace(&from, "<rbp_0>");

                // rdx
                let from = format!("0x{:02x}", registers[3].deref.map[0]);
                let output = output.replace(&from, "<rdx_1>");
                let mut ret_s = "\"".to_string();
                for r in registers[3].deref.map.iter().skip(1) {
                    ret_s.push_str(std::str::from_utf8(&r.to_le_bytes()).unwrap());
                }
                ret_s.push_str("\"");
                let padding_width = ret_s.len() + 7;
                let output = output
                    .replace(&ret_s, &format!("<rdx_2>{:padding$}", "", padding = padding_width));

                // rsi
                let from = format!("0x{:02x}", registers[4].deref.map[0]);
                let output = output.replace(&from, "<rsi_1>");
                let mut ret_s = "\"".to_string();
                for r in registers[4].deref.map.iter().skip(1) {
                    ret_s.push_str(std::str::from_utf8(&r.to_le_bytes()).unwrap());
                }
                ret_s.push_str("\"");
                let padding_width = ret_s.len() + 7;
                let output = output
                    .replace(&ret_s, &format!("<rsi_2>{:padding$}", "", padding = padding_width));

                let from = format!("0x{:02x}", registers[6].deref.map[0]);
                let output = output.replace(&from, "<rbp_1>");
                let from = format!("0x{:02x}", registers[6].deref.map[1]);
                let output = output.replace(&from, "<rbp_2>");

                assert_snapshot!(output);
            } else {
                unreachable!();
            }
        } else {
            unreachable!();
        };
    }

    #[test]
    fn test_find_mapping() {
        let mut line = "hexdump $HERETEK_MAPPING_START_0_/test.so6".to_string();
        assert_eq!(
            Some(("/test.so6".to_string(), 0, 8, 42)),
            find_mapping(&mut line, &MappingType::Start)
        );

        let mut line = "hexdump    $HERETEK_MAPPING_START_/test.so6".to_string();
        assert_eq!(
            Some(("/test.so6".to_string(), 0, 11, 43)),
            find_mapping(&mut line, &MappingType::Start)
        );

        let mut line = "hexdump $HERETEK_MAPPING_START_1_/lib/so".to_string();
        assert_eq!(
            Some(("/lib/so".to_string(), 1, 8, 40)),
            find_mapping(&mut line, &MappingType::Start)
        );

        let mut line = "hexdump $HERETEK_MAPPING_END_1_/lib/so".to_string();
        assert_eq!(
            Some(("/lib/so".to_string(), 1, 8, 38)),
            find_mapping(&mut line, &MappingType::End)
        );
        let mut line = "hexdump $HERETEK_MAPPING_LEN_1_/lib/so".to_string();
        assert_eq!(
            Some(("/lib/so".to_string(), 1, 8, 38)),
            find_mapping(&mut line, &MappingType::Len)
        );
    }
}
