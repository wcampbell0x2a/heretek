#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::manual_strip)]
#![allow(clippy::format_push_string)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::type_complexity)]
#![allow(clippy::zombie_processes)]

use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File};
use std::io;
use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, thread};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use clap_cargo::style::CLAP_STYLING;
use deku::ctx::Endian;
use deref::Deref;
use env_logger::{Builder, Env};
use gdb::write_mi;
use log::{debug, error};
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{LeaveAlternateScreen, disable_raw_mode},
};
use ratatui::prelude::*;
use ratatui::widgets::ScrollbarState;
use regex::Regex;
use register::RegisterStorage;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use mi::{Asm, MemoryMapping, data_read_memory_bytes};
use ui::hexdump::HEXDUMP_WIDTH;

mod deref;
mod gdb;
mod mi;
mod register;
mod ui;

#[derive(Debug, Copy, Clone)]
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
#[command(version, about, long_about = None, styles = CLAP_STYLING)]
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
    ///
    /// Heretek will do it's best to figure this out on it's own,
    /// but this will force the pointers to be evaluated as 32 bit
    #[arg(long)]
    #[arg(value_enum)]
    #[arg(default_value_t = PtrSize::default())]
    ptr_size: PtrSize,

    /// Execute GDB commands line-by-line from file
    ///
    /// lines starting with # are ignored
    #[arg(short, long)]
    cmds: Option<PathBuf>,

    /// Path to write log
    ///
    /// Set env `RUST_LOG` to change log level
    #[arg(long)]
    log_path: Option<String>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum PtrSize {
    #[value(name = "32")]
    Size32,
    #[value(name = "64")]
    Size64,
    #[default]
    Auto,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Mode {
    All,
    OnlyRegister,
    OnlyStack,
    OnlyInstructions,
    OnlyOutput,
    OnlyMapping,
    OnlyHexdump,
    OnlyHexdumpPopup,
    OnlySymbols,
    OnlySource,
    QuitConfirmation,
}

impl Mode {
    pub fn ui_index(&self) -> usize {
        match self {
            Mode::All => 0,
            Mode::OnlyRegister => 1,
            Mode::OnlyStack => 2,
            Mode::OnlyInstructions => 3,
            Mode::OnlyOutput => 4,
            Mode::OnlyMapping => 5,
            Mode::OnlyHexdump => 6,
            Mode::OnlyHexdumpPopup => 6,
            Mode::OnlySymbols => 7,
            Mode::OnlySource => 8,
            Mode::QuitConfirmation => 0,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Mode::All => Mode::OnlyRegister,
            Mode::OnlyRegister => Mode::OnlyStack,
            Mode::OnlyStack => Mode::OnlyInstructions,
            Mode::OnlyInstructions => Mode::OnlyOutput,
            Mode::OnlyOutput => Mode::OnlyMapping,
            Mode::OnlyMapping => Mode::OnlyHexdump,
            Mode::OnlyHexdump => Mode::OnlySymbols,
            Mode::OnlyHexdumpPopup => Mode::OnlyHexdumpPopup,
            Mode::OnlySymbols => Mode::OnlySource,
            Mode::OnlySource => Mode::All,
            Mode::QuitConfirmation => Mode::QuitConfirmation,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Bt {
    location: u64,
    function: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Symbol {
    pub address: u64,
    pub name: String,
    /// True if this symbol's address is not yet resolved and needs `info address` lookup
    pub needs_address_resolution: bool,
}

// TODO: this could be split up, some of these fields
// are always set after the file is loaded in gdb
struct App {
    /// Gdb stdin
    gdb_stdin: Arc<Mutex<dyn Write + Send>>,
}

// TODO: this could be split up, some of these fields
// are always set after the file is loaded in gdb
struct StateShare {
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Default, Clone)]
struct Scroll {
    scroll: usize,
    state: ScrollbarState,
}

impl Scroll {
    pub fn reset(&mut self) {
        self.scroll = 0;
        self.state = self.state.position(0);
    }

    pub fn end(&mut self, len: usize) {
        self.scroll = len;
        self.state.last();
    }

    pub fn down(&mut self, n: usize, len: usize) {
        if self.scroll < len.saturating_sub(1) {
            self.scroll += n;
            self.state = self.state.position(self.scroll);
        }
    }

    pub fn up(&mut self, n: usize) {
        if self.scroll > n {
            self.scroll -= n;
        } else {
            self.scroll = 0;
        }
        self.state = self.state.position(self.scroll);
    }
}

#[derive(Clone, Debug)]
struct State {
    /// Messages to write to gdb mi
    next_write: Vec<String>,
    /// Stack of what was written to gdb that is expected back in order to parse correctly
    written: VecDeque<Written>,
    /// Waiting for execution to stop (after si, continue, step, run, etc.)
    executing: bool,
    /// -32 bit mode
    ptr_size: PtrSize,
    /// Current filepath of .text
    filepath: Option<PathBuf>,
    /// Current endian
    endian: Option<Endian>,
    /// Current mode
    mode: Mode,
    /// Previous mode (for quit confirmation)
    previous_mode: Mode,
    /// TUI input
    input: Input,
    /// Currnt input mode of tui
    input_mode: InputMode,
    /// List of previously sent commands from our own input
    sent_input: LimitedBuffer<String>,
    /// Memory map TUI
    memory_map: Option<Vec<MemoryMapping>>,
    memory_map_scroll: Scroll,
    memory_map_selected: usize,
    memory_map_viewport_height: u16,
    /// Current $pc
    current_pc: u64, // TODO: replace with AtomicU64?
    /// All output from gdb
    output: Vec<String>,
    output_scroll: Scroll,
    /// Saved output such as (gdb) or > from gdb
    stream_output_prompt: String,
    /// Register TUI
    register_changed: Vec<u16>,
    register_names: Vec<String>,
    registers: Vec<RegisterStorage>,
    registers_scroll: Scroll,
    /// Saved Stack
    stack: BTreeMap<u64, Deref>,
    /// Saved ASM
    asm: Vec<Asm>,
    /// Hexdump
    hexdump: Option<(u64, Vec<u8>)>,
    hexdump_scroll: Scroll,
    hexdump_popup: Input,
    /// Right side of status in TUI
    async_result: String,
    /// Left side of status in TUI
    status: String,
    bt: Vec<Bt>,
    completions: Vec<String>,
    /// Current source file and line info
    current_source_file: Option<String>,
    current_source_line: Option<u32>,
    source_lines: Vec<String>,
    /// Current source language detected by GDB
    source_language: Option<String>,
    /// Symbol browser
    symbols: Vec<Symbol>,
    symbols_scroll: Scroll,
    symbols_selected: usize,
    symbols_viewport_height: u16,
    symbol_asm: Vec<Asm>,
    symbol_asm_scroll: Scroll,
    /// Name of the symbol currently being viewed in ASM
    symbol_asm_name: String,
    /// Whether we're viewing assembly for a selected symbol
    symbols_viewing_asm: bool,
    /// Symbol search
    symbols_search_active: bool,
    symbols_search_input: Input,
}

impl State {
    pub fn new(args: Args) -> State {
        State {
            next_write: vec![],
            written: VecDeque::new(),
            executing: false,
            ptr_size: args.ptr_size,
            filepath: None,
            endian: None,
            mode: Mode::All,
            previous_mode: Mode::All,
            input: Input::default(),
            input_mode: InputMode::Normal,
            sent_input: LimitedBuffer::new(100),
            memory_map: None,
            memory_map_scroll: Scroll::default(),
            memory_map_selected: 0,
            memory_map_viewport_height: 0,
            current_pc: 0,
            output: Vec::new(),
            output_scroll: Scroll::default(),
            stream_output_prompt: String::new(),
            register_changed: vec![],
            register_names: vec![],
            registers: vec![],
            registers_scroll: Scroll::default(),
            stack: BTreeMap::new(),
            asm: Vec::new(),
            hexdump: None,
            hexdump_scroll: Scroll::default(),
            hexdump_popup: Input::default(),
            async_result: String::new(),
            status: String::new(),
            bt: vec![],
            completions: vec![],
            current_source_file: None,
            current_source_line: None,
            source_lines: Vec::new(),
            source_language: None,
            symbols: Vec::new(),
            symbols_scroll: Scroll::default(),
            symbols_selected: 0,
            symbols_viewport_height: 0,
            symbol_asm: Vec::new(),
            symbol_asm_scroll: Scroll::default(),
            symbol_asm_name: String::new(),
            symbols_viewing_asm: false,
            symbols_search_active: false,
            symbols_search_input: Input::default(),
        }
    }
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
                        .stderr(Stdio::null())
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

        let app = App { gdb_stdin };

        (reader, app)
    }
}

impl State {
    // Parse a "file filepath" command and save
    fn save_filepath(&mut self, val: &str) {
        let filepath: Vec<&str> = val.split_whitespace().collect();
        if filepath.len() > 1 {
            let filepath = resolve_home(filepath[1]).unwrap();
            self.filepath = Some(filepath);
        }
    }

    pub fn find_first_heap(&mut self) -> Option<MemoryMapping> {
        if let Some(memory_map) = self.memory_map.clone() {
            memory_map.iter().find(|a| a.is_heap()).cloned()
        } else {
            None
        }
    }

    pub fn find_first_stack(&self) -> Option<MemoryMapping> {
        if let Some(memory_map) = self.memory_map.clone() {
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
            // trace!("{:02x?}", memory_map);
            if self.memory_map.is_some() {
                for r in self.memory_map.as_ref().unwrap() {
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

    /// Get filtered symbols based on search input
    pub fn get_filtered_symbols(&self) -> Vec<(usize, &Symbol)> {
        // Filter based on search input, regardless of whether search mode is active
        if self.symbols_search_input.value().is_empty() {
            return self.symbols.iter().enumerate().collect();
        }

        let search_term = self.symbols_search_input.value().to_lowercase();
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, sym)| {
                // Simple fuzzy matching: check if all characters in search appear in order
                let name_lower = sym.name.to_lowercase();
                let mut name_chars = name_lower.chars();
                search_term.chars().all(|c| name_chars.any(|nc| nc == c))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
enum Written {
    /// Requested Register Value deref
    // TODO: Could this just be the register name?
    RegisterValue((String, u64)),
    /// Requested Stack Bytes
    ///
    /// None - This is the first time this is requested
    /// Some - This has alrady been read, and this is a deref, trust
    ///        the `base_reg` of .0
    Stack(Option<String>),
    /// Requested Memory Read (for hexdump)
    Memory,
    /// Requested Asm At $pc
    AsmAtPc,
    /// Requested symbol at addr for register (from deref)
    SymbolAtAddrRegister((String, u64)),
    /// Requested symbol at addr for stack (from deref)
    SymbolAtAddrStack(String),
    /// Requested size of arch ptr for `ptr_size`
    SizeOfVoidStar,
    /// Requested list of all symbols
    SymbolList,
    /// Requested disassembly of a specific symbol by name
    SymbolDisassembly(String),
    /// Requested address lookup for symbol (to disassemble it next)
    SymbolAddressLookup(String),
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // initialize logging, to log_path if available
    init_logging(args.log_path.as_ref())?;

    // Check for valid cmd file
    if let Some(cmds) = &args.cmds
        && !cmds.exists()
    {
        anyhow::bail!("Filepath for --cmds does not exist: `{}`", cmds.display());
    }
    // Start rx thread
    let (gdb_stdout, mut app) = App::new_stream(args.clone());
    let state = State::new(args.clone());
    let mut state_share = StateShare { state: Arc::new(Mutex::new(state)) };

    // Setup terminal
    let mut terminal = ratatui::init();

    spawn_gdb_interact(&state_share, gdb_stdout);

    // Now that we have a gdb, run each command
    if let Some(cmds) = args.cmds {
        let data = fs::read_to_string(cmds).unwrap();
        for cmd in data.lines() {
            if !cmd.starts_with('#') {
                let mut state = state_share.state.lock().unwrap();
                state.sent_input.push(cmd.to_string());
                process_line(&mut app, &mut state, cmd);
            }
        }
    }

    // Run tui application
    let res = run_app(&mut terminal, &mut app, &mut state_share);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        anyhow::bail!("{err:?}")
    }

    Ok(())
}

fn init_logging(log_path: Option<&String>) -> anyhow::Result<()> {
    if let Some(log_path) = log_path {
        let log_file =
            Arc::new(Mutex::new(File::create(log_path).context("Could not create log file")?));
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

fn spawn_gdb_interact(state: &StateShare, gdb_stdout: BufReader<Box<dyn Read + Send>>) {
    let state_arc = Arc::clone(&state.state);

    // Thread to read GDB output and parse it
    thread::spawn(move || gdb::gdb_interact(gdb_stdout, state_arc));
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    state_share: &mut StateShare,
) -> io::Result<()> {
    loop {
        {
            let mut state = state_share.state.lock().unwrap();
            terminal.draw(|f| ui::ui(f, &mut state))?;
        }

        // check and see if we need to write to GBD MI
        {
            let mut state = state_share.state.lock().unwrap();
            let next_write = &mut state.next_write;
            if !next_write.is_empty() {
                for w in &*next_write {
                    write_mi(&app.gdb_stdin, w);
                }
                next_write.clear();
            }
        }

        // check if completions are back and we need to replace the input
        {
            let mut state = state_share.state.lock().unwrap();
            if !state.completions.is_empty() {
                // Just replace if completions is 1
                if state.completions.len() == 1 {
                    state.input = Input::new(state.completions[0].clone());
                    // we are done with the values, clear them
                    state.completions.clear();
                }

                // if else, we display them
            }
        }
        // Use fast polling when expecting GDB responses, slow polling when idle
        let poll_timeout = {
            let state = state_share.state.lock().unwrap();
            if state.written.is_empty() && state.next_write.is_empty() && !state.executing {
                // Idle: reduce CPU usage
                Duration::from_millis(250)
            } else {
                // Active: fast updates
                Duration::from_millis(10)
            }
        };

        if event::poll(poll_timeout)?
            && let Event::Key(key) = event::read()?
        {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                gdb::write_mi(&app.gdb_stdin, "-exec-interrupt");
                continue;
            }
            let (input_mode, mode) = {
                let state = state_share.state.lock().unwrap();
                (state.input_mode, state.mode)
            };
            match (&input_mode, key.code, &mode) {
                // hexdump popup
                (_, KeyCode::Esc, Mode::OnlyHexdumpPopup) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.hexdump_popup = Input::default();
                    state.mode = Mode::OnlyHexdump;
                }
                (_, KeyCode::Char('S'), Mode::OnlyHexdumpPopup) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.input.handle_event(&Event::Key(key));
                }
                (_, KeyCode::Enter, Mode::OnlyHexdumpPopup) => {
                    let mut state = state_share.state.lock().unwrap();
                    let val = state.hexdump_popup.clone();
                    let val = val.value();

                    if let Some(hexdump) = state.hexdump.as_ref()
                        && let Some(path) = resolve_home(val)
                        && std::fs::write(&path, &hexdump.1).is_ok()
                    {
                        state.output.push(format!(
                            "h> hexdump succesfully written to {}",
                            path.to_str().unwrap()
                        ));
                    }
                    state.hexdump_popup = Input::default();
                    state.mode = Mode::OnlyHexdump;
                }
                (_, _, Mode::OnlyHexdumpPopup) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.hexdump_popup.handle_event(&Event::Key(key));
                }
                // quit confirmation
                (_, KeyCode::Enter, Mode::QuitConfirmation) => {
                    return Ok(());
                }
                (_, KeyCode::Esc, Mode::QuitConfirmation) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = state.previous_mode;
                }
                // Input
                (InputMode::Normal, KeyCode::Char('i'), _)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !(state.mode == Mode::OnlySymbols && state.symbols_search_active)
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    state.input_mode = InputMode::Editing;
                }
                (InputMode::Normal, KeyCode::Char('q'), _)
                    if {
                        let state = state_share.state.lock().unwrap();
                        state.mode != Mode::QuitConfirmation
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    state.previous_mode = state.mode;
                    state.mode = Mode::QuitConfirmation;
                }
                // Modes
                (InputMode::Normal, KeyCode::Tab, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = state.mode.next();
                }
                (_, KeyCode::F(1), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::All;
                }
                (_, KeyCode::F(2), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyRegister;
                }
                (_, KeyCode::F(3), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyStack;
                }
                (_, KeyCode::F(4), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyInstructions;
                }
                (_, KeyCode::F(5), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyOutput;
                }
                (_, KeyCode::F(6), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyMapping;
                }
                (_, KeyCode::F(7), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyHexdump;
                }
                (_, KeyCode::F(8), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlySymbols;
                    if state.symbols.is_empty() {
                        state.next_write.push(mi::info_functions());
                        state.written.push_back(Written::SymbolList);
                    }
                }
                (_, KeyCode::F(9), _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlySource;
                }
                (InputMode::Editing, KeyCode::Esc, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.input_mode = InputMode::Normal;
                }
                (InputMode::Normal, KeyCode::Char('j'), Mode::All) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.registers.len();
                    state.registers_scroll.down(1, len);
                }
                (InputMode::Normal, KeyCode::Char('k'), Mode::All) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.registers_scroll.up(1);
                }
                (InputMode::Normal, KeyCode::Char('J'), Mode::All) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.registers.len();
                    state.registers_scroll.down(50, len);
                }
                (InputMode::Normal, KeyCode::Char('K'), Mode::All) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.registers_scroll.up(50);
                }
                (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyRegister) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.registers.len();
                    state.registers_scroll.down(1, len);
                }
                (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyRegister) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.registers_scroll.up(1);
                }
                (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyRegister) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.registers.len();
                    state.registers_scroll.down(50, len);
                }
                (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyRegister) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.registers_scroll.up(50);
                }
                // output
                (InputMode::Normal, KeyCode::Char('g'), Mode::OnlyOutput) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.output_scroll.reset();
                }
                (InputMode::Normal, KeyCode::Char('G'), Mode::OnlyOutput) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.output.len();
                    state.output_scroll.end(len);
                }
                (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyOutput) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.output.len();
                    state.output_scroll.down(1, len);
                }
                (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyOutput) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.output_scroll.up(1);
                }
                (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyOutput) => {
                    let mut state = state_share.state.lock().unwrap();
                    let len = state.output.len();
                    state.output_scroll.down(50, len);
                }
                (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyOutput) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.output_scroll.up(50);
                }
                // memory mapping
                (InputMode::Normal, KeyCode::Char('g'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.memory_map_selected = 0;
                    state.memory_map_scroll.reset();
                }
                (InputMode::Normal, KeyCode::Char('G'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(memory) = state.memory_map.as_ref() {
                        let len = memory.len();
                        if len > 0 {
                            state.memory_map_selected = len - 1;
                            state.memory_map_scroll.end(len);
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(memory) = state.memory_map.as_ref() {
                        let len = memory.len();
                        if state.memory_map_selected < len.saturating_sub(1) {
                            state.memory_map_selected += 1;
                            let selected_screen_pos = (state.memory_map_selected + 1)
                                .saturating_sub(state.memory_map_scroll.scroll);
                            if selected_screen_pos >= state.memory_map_viewport_height as usize {
                                let target_scroll = state.memory_map_selected + 2
                                    - state.memory_map_viewport_height as usize;
                                state.memory_map_scroll.scroll = target_scroll;
                                state.memory_map_scroll.state =
                                    state.memory_map_scroll.state.position(target_scroll);
                            }
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    if state.memory_map_selected > 0 {
                        state.memory_map_selected -= 1;
                        if (state.memory_map_selected + 1) < state.memory_map_scroll.scroll {
                            let target_scroll = state.memory_map_selected + 1;
                            state.memory_map_scroll.scroll = target_scroll;
                            state.memory_map_scroll.state =
                                state.memory_map_scroll.state.position(target_scroll);
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(memory) = state.memory_map.as_ref() {
                        let len = memory.len();
                        let new_selected =
                            (state.memory_map_selected + 50).min(len.saturating_sub(1));
                        state.memory_map_selected = new_selected;
                        let selected_screen_pos = (state.memory_map_selected + 1)
                            .saturating_sub(state.memory_map_scroll.scroll);
                        if selected_screen_pos >= state.memory_map_viewport_height as usize {
                            let target_scroll = state.memory_map_selected + 2
                                - state.memory_map_viewport_height as usize;
                            state.memory_map_scroll.scroll = target_scroll;
                            state.memory_map_scroll.state =
                                state.memory_map_scroll.state.position(target_scroll);
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    let new_selected = state.memory_map_selected.saturating_sub(50);
                    state.memory_map_selected = new_selected;
                    if (state.memory_map_selected + 1) < state.memory_map_scroll.scroll {
                        let target_scroll = state.memory_map_selected + 1;
                        state.memory_map_scroll.scroll = target_scroll;
                        state.memory_map_scroll.state =
                            state.memory_map_scroll.state.position(target_scroll);
                    }
                }
                (InputMode::Normal, KeyCode::Char('H'), Mode::OnlyMapping) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(memory_map) = state.memory_map.as_ref()
                        && let Some(selected_mapping) = memory_map.get(state.memory_map_selected)
                    {
                        let s = data_read_memory_bytes(
                            selected_mapping.start_address,
                            0,
                            selected_mapping.size,
                        );
                        state.next_write.push(s);
                        state.written.push_back(Written::Memory);

                        state.mode = Mode::OnlyHexdump;
                        state.hexdump_scroll.reset();
                    }
                }
                // hexdump
                (InputMode::Normal, KeyCode::Char('g'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.hexdump_scroll.reset();
                }
                (InputMode::Normal, KeyCode::Char('G'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(hexdump) = state.hexdump.as_ref() {
                        let len = hexdump.1.len() / HEXDUMP_WIDTH;
                        state.hexdump_scroll.end(len);
                    }
                }
                (InputMode::Normal, KeyCode::Char('S'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.mode = Mode::OnlyHexdumpPopup;
                }
                (InputMode::Normal, KeyCode::Char('H'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(find_heap) = state.find_first_heap() {
                        let s = data_read_memory_bytes(find_heap.start_address, 0, find_heap.size);
                        state.next_write.push(s);
                        state.written.push_back(Written::Memory);

                        // reset position
                        state.hexdump_scroll.reset();
                    }
                }
                (InputMode::Normal, KeyCode::Char('T'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    if let Some(find_heap) = state.find_first_stack() {
                        let s = data_read_memory_bytes(find_heap.start_address, 0, find_heap.size);
                        state.next_write.push(s);
                        state.written.push_back(Written::Memory);

                        // reset position
                        state.hexdump_scroll.reset();
                    }
                }
                (InputMode::Normal, KeyCode::Char('j'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    let hexdump = &state.hexdump;
                    if let Some(hexdump) = hexdump.as_ref() {
                        let len = hexdump.1.len() / HEXDUMP_WIDTH;
                        state.hexdump_scroll.down(1, len);
                    }
                }
                (InputMode::Normal, KeyCode::Char('k'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.hexdump_scroll.up(1);
                }
                (InputMode::Normal, KeyCode::Char('J'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    let hexdump = &state.hexdump;
                    if let Some(hexdump) = hexdump.as_ref() {
                        let len = hexdump.1.len() / HEXDUMP_WIDTH;
                        state.hexdump_scroll.down(50, len);
                    }
                }
                (InputMode::Normal, KeyCode::Char('K'), Mode::OnlyHexdump) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.hexdump_scroll.up(50);
                }
                // symbols - list navigation
                (InputMode::Normal, KeyCode::Char('r' | 'R'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    state.next_write.push(mi::info_functions());
                    state.written.push_back(Written::SymbolList);
                }
                (InputMode::Normal, KeyCode::Char('g'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_viewing_asm {
                        state.symbol_asm_scroll.reset();
                    } else {
                        state.symbols_selected = 0;
                        state.symbols_scroll.reset();
                    }
                }
                (InputMode::Normal, KeyCode::Char('G'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_viewing_asm {
                        let len = state.symbol_asm.len() + 1; // +1 for header
                        state.symbol_asm_scroll.end(len);
                    } else {
                        let len = state.get_filtered_symbols().len();
                        if len > 0 {
                            state.symbols_selected = len - 1;
                            state.symbols_scroll.end(len + 1); // +1 for header
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('j'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_viewing_asm {
                        let len = state.symbol_asm.len() + 1;
                        state.symbol_asm_scroll.down(1, len);
                    } else {
                        let len = state.get_filtered_symbols().len();
                        if state.symbols_selected < len.saturating_sub(1) {
                            state.symbols_selected += 1;
                            let selected_screen_pos = (state.symbols_selected + 1)
                                .saturating_sub(state.symbols_scroll.scroll);
                            if selected_screen_pos >= state.symbols_viewport_height as usize {
                                let target_scroll = state.symbols_selected + 2
                                    - state.symbols_viewport_height as usize;
                                state.symbols_scroll.scroll = target_scroll;
                                state.symbols_scroll.state =
                                    state.symbols_scroll.state.position(target_scroll);
                            }
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('k'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_viewing_asm {
                        state.symbol_asm_scroll.up(1);
                    } else if state.symbols_selected > 0 {
                        state.symbols_selected -= 1;
                        if (state.symbols_selected + 1) < state.symbols_scroll.scroll {
                            let target_scroll = state.symbols_selected + 1;
                            state.symbols_scroll.scroll = target_scroll;
                            state.symbols_scroll.state =
                                state.symbols_scroll.state.position(target_scroll);
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('J'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_viewing_asm {
                        let len = state.symbol_asm.len() + 1;
                        state.symbol_asm_scroll.down(50, len);
                    } else {
                        let len = state.get_filtered_symbols().len();
                        let new_selected = (state.symbols_selected + 50).min(len.saturating_sub(1));
                        state.symbols_selected = new_selected;
                        let selected_screen_pos = (state.symbols_selected + 1)
                            .saturating_sub(state.symbols_scroll.scroll);
                        if selected_screen_pos >= state.symbols_viewport_height as usize {
                            let target_scroll =
                                state.symbols_selected + 2 - state.symbols_viewport_height as usize;
                            state.symbols_scroll.scroll = target_scroll;
                            state.symbols_scroll.state =
                                state.symbols_scroll.state.position(target_scroll);
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Char('K'), Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_viewing_asm {
                        state.symbol_asm_scroll.up(50);
                    } else {
                        let new_selected = state.symbols_selected.saturating_sub(50);
                        state.symbols_selected = new_selected;
                        if (state.symbols_selected + 1) < state.symbols_scroll.scroll {
                            let target_scroll = state.symbols_selected + 1;
                            state.symbols_scroll.scroll = target_scroll;
                            state.symbols_scroll.state =
                                state.symbols_scroll.state.position(target_scroll);
                        }
                    }
                }
                (InputMode::Normal, KeyCode::Enter, Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        !state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    if !state.symbols_viewing_asm {
                        let filtered = state.get_filtered_symbols();
                        if let Some((_original_index, symbol)) =
                            filtered.get(state.symbols_selected)
                        {
                            let symbol = (*symbol).clone();
                            drop(filtered);

                            // For symbols that need address resolution, query address first
                            if symbol.needs_address_resolution {
                                // Extract function name before generic parameters/arguments for GDB
                                let name_for_gdb = if let Some(lt_pos) = symbol.name.find('<') {
                                    symbol.name[..lt_pos].to_string()
                                } else if let Some(paren_pos) = symbol.name.find('(') {
                                    symbol.name[..paren_pos].to_string()
                                } else {
                                    symbol.name.clone()
                                };
                                let cmd = mi::info_address(&name_for_gdb);
                                state.next_write.push(cmd);
                                state.written.push_back(Written::SymbolAddressLookup(symbol.name));
                            } else {
                                // Use address directly for normal symbols
                                let cmd = mi::data_disassemble(symbol.address as usize, 500);
                                state.next_write.push(cmd);
                                state.written.push_back(Written::SymbolDisassembly(symbol.name));
                            }
                            state.symbol_asm_scroll.reset();
                            state.symbols_viewing_asm = true;
                        }
                    }
                }
                (_, KeyCode::Esc, Mode::OnlySymbols) => {
                    let mut state = state_share.state.lock().unwrap();
                    if state.symbols_search_active {
                        state.symbols_search_active = false;
                    } else if state.symbols_viewing_asm {
                        state.symbols_viewing_asm = false;
                    }
                }
                (InputMode::Normal, KeyCode::Enter, Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    state.symbols_search_active = false;
                }
                (InputMode::Normal, KeyCode::Char('/'), Mode::OnlySymbols) => {
                    let mut state = state_share.state.lock().unwrap();
                    if !state.symbols_viewing_asm {
                        state.symbols_search_input = Input::default();
                        state.symbols_search_active = true;
                        state.symbols_selected = 0;
                        state.symbols_scroll.reset();
                    }
                }
                (_, KeyCode::Tab, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    completion(app, &mut state)?;
                }
                (_, KeyCode::Enter, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    key_enter(app, &mut state)?;
                }
                (_, KeyCode::Down, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    key_down(&mut state);
                }
                (_, KeyCode::Up, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    key_up(&mut state);
                }
                (InputMode::Normal, _, Mode::OnlySymbols)
                    if {
                        let state = state_share.state.lock().unwrap();
                        state.symbols_search_active
                    } =>
                {
                    let mut state = state_share.state.lock().unwrap();
                    state.symbols_search_input.handle_event(&Event::Key(key));
                    state.symbols_selected = 0;
                    state.symbols_scroll.reset();
                }
                (InputMode::Editing, _, _) => {
                    let mut state = state_share.state.lock().unwrap();
                    state.completions.clear();
                    state.input.handle_event(&Event::Key(key));
                }
                _ => (),
            }
        }
    }
}

fn key_up(state: &mut State) {
    if state.sent_input.buffer.is_empty() {
        state.sent_input.offset = 0;
    } else {
        if state.sent_input.offset < state.sent_input.buffer.len() {
            state.sent_input.offset += 1;
        }
        update_from_previous_input(state);
    }
}

fn key_down(state: &mut State) {
    if state.sent_input.buffer.is_empty() {
        state.sent_input.offset = 0;
    } else {
        if state.sent_input.offset != 0 {
            state.sent_input.offset -= 1;
            if state.sent_input.offset == 0 {
                state.input.reset();
            }
        }
        update_from_previous_input(state);
    }
}

fn completion(app: &mut App, state: &mut State) -> Result<(), io::Error> {
    let val = state.input.clone();
    let val = val.value();
    let cmd = format!("-complete \"{val}\"");
    gdb::write_mi(&app.gdb_stdin, &cmd);

    Ok(())
}

fn key_enter(app: &mut App, state: &mut State) -> Result<(), io::Error> {
    if state.input.value().is_empty() {
        state.sent_input.offset = 0;

        let messages = state.sent_input.clone();
        let messages = messages.as_slice().iter();
        if let Some(val) = messages.last() {
            process_line(app, state, val);
        }
    } else {
        state.sent_input.offset = 0;
        state.sent_input.push(state.input.value().into());

        let val = state.input.clone();
        let val = val.value();
        process_line(app, state, val);
    }

    Ok(())
}

fn process_line(app: &mut App, state: &mut State, val: &str) {
    let mut val = val.to_owned();

    // Replace internal variables
    {
        replace_internal_variables(state, &mut val);
    }

    // Resolve parens with expressions
    resolve_paren_expressions(&mut val);

    if val == "r" || val == "ru" || val == "run" {
        // Replace run with -exec-run and target-async
        // This is to allow control+C to interrupt
        // gdb::write_mi(&app.gdb_stdin, "-gdb-set target-async on");

        let cmd = "-gdb-set mi-async on";
        state.output.push(format!("h> {cmd}"));
        gdb::write_mi(&app.gdb_stdin, cmd);

        let cmd = "-exec-run";
        gdb::write_mi(&app.gdb_stdin, cmd);

        let cmd = "-gdb-set disassembly-flavor intel";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val.starts_with("at")
        || val.starts_with("att")
        || val.starts_with("atta")
        || val.starts_with("attac")
        || val.starts_with("attach")
    {
        // Write original cmd
        gdb::write_mi(&app.gdb_stdin, &val);
        state.output.push(val);
        state.executing = true;
        state.input.reset();

        let cmd = "-gdb-set disassembly-flavor intel";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(cmd.to_owned());
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
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val == "si" || val == "stepi" {
        let cmd = "-exec-step-instruction";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val == "step" {
        let cmd = "-exec-step";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val == "ni" || val == "nexti" {
        let cmd = "-exec-next-instruction";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val == "n" || val == "next" {
        let cmd = "-exec-next";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val == "finish" || val == "fin" {
        let cmd = "-exec-finish";
        gdb::write_mi(&app.gdb_stdin, cmd);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val.starts_with("until") || val.starts_with("u ") {
        // For until, just pass through but mark as executing
        gdb::write_mi(&app.gdb_stdin, &val);
        state.output.push(val);

        state.executing = true;
        state.input.reset();
        return;
    } else if val.starts_with("file") {
        // we parse file, but still send it on
        state.save_filepath(&val);
    } else if val.starts_with("hexdump") {
        debug!("hexdump: {val}");
        // don't send it on, parse the hexdump command
        let split: Vec<&str> = val.split_whitespace().collect();
        if split.len() < 3 {
            error!("Invalid arguments, expected 'hexdump addr len'");
            return;
        }
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
        state.next_write.push(s);
        state.written.push_back(Written::Memory);
        state.input.reset();
        return;
    }
    gdb::write_mi(&app.gdb_stdin, &val);
    state.input.reset();
}

fn resolve_paren_expressions(val: &mut String) {
    static RE_PAREN: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\(([^()]+)\)").unwrap());

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

fn replace_internal_variables(state: &mut State, line: &mut String) {
    replace_mapping(state, line, MappingType::Start);
    replace_mapping(state, line, MappingType::End);
    replace_mapping(state, line, MappingType::Len);
}

fn replace_mapping(state: &mut State, text: &mut String, mt: MappingType) {
    let ret = find_mapping(text, &mt);
    if let Some((path, prefix, start_idx, end_idx)) = ret
        && let Some(ref memory_map) = state.memory_map
    {
        let resolve =
            memory_map.iter().filter(|a| a.path == Some(path.clone())).nth(prefix as usize);
        let addr = match mt {
            MappingType::Start => resolve.map(|a| a.start_address),
            MappingType::End => resolve.map(|a| a.end_address),
            MappingType::Len => resolve.map(|a| a.size),
        };
        if let Some(addr) = addr {
            text.replace_range(start_idx..end_idx, &format!("{addr:#08x?}"));
        }
    }
}

fn find_mapping(text: &mut str, mt: &MappingType) -> Option<(String, u32, usize, usize)> {
    let start = mt.env_start();
    if let Some(start_idx) = text.find(start) {
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

        let prefix = prefix.unwrap_or("0".to_string()).parse::<u32>().unwrap();

        Some((path, prefix, start_idx, end_idx))
    } else {
        None
    }
}

fn update_from_previous_input(state: &mut State) {
    if state.sent_input.buffer.len() >= state.sent_input.offset
        && let Some(msg) =
            state.sent_input.buffer.get(state.sent_input.buffer.len() - state.sent_input.offset)
    {
        state.input = Input::new(msg.clone());
    }
}

// Now in tests module:
#[cfg(test)]
mod tests {
    use std::{ffi::CString, time::Instant};

    use super::*;
    use insta::assert_snapshot;
    use libc::{S_IRGRP, S_IROTH, S_IRUSR, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR, chmod};

    use ratatui::{Terminal, backend::TestBackend};
    use test_assets_ureq::{TestAssetDef, dl_test_files_backoff};

    fn run_a_bit(args: Args) -> (App, StateShare, Terminal<TestBackend>) {
        let (gdb_stdout, mut app) = App::new_stream(args.clone());
        let state = State::new(args.clone());
        let state_share = StateShare { state: Arc::new(Mutex::new(state)) };
        spawn_gdb_interact(&state_share, gdb_stdout);

        if let Some(cmds) = args.cmds {
            let data = fs::read_to_string(cmds).unwrap();
            for cmd in data.lines() {
                if !cmd.starts_with('#') {
                    let mut state = state_share.state.lock().unwrap();
                    state.sent_input.push(cmd.to_string());
                    process_line(&mut app, &mut state, cmd);
                }
            }
        }
        let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
        let start_time = Instant::now();
        let duration = Duration::from_secs(10);

        while start_time.elapsed() < duration {
            // Sleep, to make sure that the gdb thread can act
            thread::sleep(Duration::from_millis(100));

            let mut state = state_share.state.lock().unwrap();
            terminal.draw(|f| ui::ui(f, &mut state)).unwrap();

            // check and see if we need to write to GBD MI
            if !state.next_write.is_empty() {
                for w in &*state.next_write {
                    write_mi(&app.gdb_stdin, w);
                }
                state.next_write.clear();
            }
        }

        (app, state_share, terminal)
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
        let c_path = CString::new(file_path.clone()).expect("CString::new failed");
        let mode = S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
        unsafe { chmod(c_path.as_ptr(), mode) };

        let mut args = Args::default();
        args.cmds = Some(PathBuf::from("test-sources/repeated_ptr.source"));

        let (_, state, terminal) = run_a_bit(args);
        let _output = terminal.backend();
        let registers = state.state.lock().unwrap().registers.clone();
        let stack = state.state.lock().unwrap().stack.clone();

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
        let c_path = CString::new(file_path.clone()).expect("CString::new failed");
        let mode = S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
        unsafe { chmod(c_path.as_ptr(), mode) };

        let mut args = Args::default();
        args.cmds = Some(PathBuf::from("test-sources/test.source"));

        let (_, state, terminal) = run_a_bit(args);
        let output = terminal.backend();

        // Now, we need to rewrite all the addresses that change for the registers and stack
        // this makes this work for any (hopefully) computer that runs these commands.
        // I'm not in love with this testing plan! If this becomes a problem, these
        // could be removed.
        let output = output.to_string();
        let stack = state.state.lock().unwrap().stack.clone();
        let mut entries: Vec<_> = stack.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let first_stack = entries[0].0;
        let from = format!("0x{first_stack:02x}");
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

        let registers = state.state.lock().unwrap().registers.clone();
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
        ret_s.push('"');
        let padding_width = ret_s.len() + 7;
        let output =
            output.replace(&ret_s, &format!("<rdx_2>{:padding$}", "", padding = padding_width));

        // rsi
        let from = format!("0x{:02x}", registers[4].deref.map[0]);
        let output = output.replace(&from, "<rsi_1>");
        let mut ret_s = "\"".to_string();
        for r in registers[4].deref.map.iter().skip(1) {
            ret_s.push_str(std::str::from_utf8(&r.to_le_bytes()).unwrap());
        }
        ret_s.push('"');
        let padding_width = ret_s.len() + 7;
        let output =
            output.replace(&ret_s, &format!("<rsi_2>{:padding$}", "", padding = padding_width));

        let from = format!("0x{:02x}", registers[6].deref.map[0]);
        let output = output.replace(&from, "<rbp_1>");
        let from = format!("0x{:02x}", registers[6].deref.map[1]);
        let output = output.replace(&from, "<rbp_2>");

        assert_snapshot!(output);
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

    #[test]
    fn test_resolve_home() {
        unsafe {
            env::set_var("HOME", "/home/testuser");
        }

        let result = resolve_home("~/Documents/file.txt");
        assert_eq!(result, Some(PathBuf::from("/home/testuser/Documents/file.txt")));

        let result = resolve_home("/absolute/path");
        assert_eq!(result, Some(PathBuf::from("/absolute/path")));

        let result = resolve_home("relative/path");
        assert_eq!(result, Some(PathBuf::from("relative/path")));
    }

    #[test]
    fn test_resolve_paren_expressions() {
        let mut val = "Value is (2 + 3)".to_string();
        resolve_paren_expressions(&mut val);
        assert_eq!(val, "Value is 5");

        let mut val = "Calculation (10 * 2)".to_string();
        resolve_paren_expressions(&mut val);
        assert_eq!(val, "Calculation 20");

        let mut val = "Multiple (1 + 1) and (2 * 3)".to_string();
        resolve_paren_expressions(&mut val);
        assert_eq!(val, "Multiple 2 and 6");

        let mut val = "Invalid (abc) expression".to_string();
        resolve_paren_expressions(&mut val);
        assert_eq!(val, "Invalid abc expression");

        let mut val = "No parentheses here".to_string();
        resolve_paren_expressions(&mut val);
        assert_eq!(val, "No parentheses here");
    }

    #[test]
    fn test_limited_buffer_push() {
        let mut buffer: LimitedBuffer<i32> = LimitedBuffer::new(3);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        assert_eq!(buffer.buffer.len(), 3);

        // When full, oldest element is removed
        buffer.push(4);
        assert_eq!(buffer.buffer.len(), 3);
        assert_eq!(*buffer.buffer.front().unwrap(), 2);
        assert_eq!(*buffer.buffer.back().unwrap(), 4);
    }

    #[test]
    fn test_limited_buffer_new() {
        let buffer: LimitedBuffer<String> = LimitedBuffer::new(5);
        assert_eq!(buffer.buffer.len(), 0);
        assert_eq!(buffer.capacity, 5);
        assert_eq!(buffer.offset, 0);
    }

    #[test]
    fn test_limited_buffer_as_slice() {
        let mut buffer: LimitedBuffer<i32> = LimitedBuffer::new(3);

        buffer.push(1);
        buffer.push(2);
        let slice = buffer.as_slice();
        assert_eq!(slice.len(), 2);

        buffer.push(3);
        let slice = buffer.as_slice();
        assert_eq!(slice.len(), 3);
    }

    #[test]
    fn test_mapping_type_env_start() {
        assert_eq!(MappingType::Start.env_start(), "$HERETEK_MAPPING_START_");
        assert_eq!(MappingType::End.env_start(), "$HERETEK_MAPPING_END_");
        assert_eq!(MappingType::Len.env_start(), "$HERETEK_MAPPING_LEN_");
    }
}
