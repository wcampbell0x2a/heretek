use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::{error::Error, io};

use clap::Parser;
use env_logger::{Builder, Env};
use log::debug;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Cell, Row, Table, TableState};
use ratatui::{
    crossterm::{
        event::{self, DisableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, LeaveAlternateScreen},
    },
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use Constraint::{Fill, Length, Min};

mod mi;
use mi::{
    data_disassemble, data_read_sp_bytes, join_registers, parse_asm_insns_values,
    parse_key_value_pairs, parse_register_names_values, parse_register_values, read_pc_value, Asm,
    MIResponse, Register,
};

enum InputMode {
    Normal,
    Editing,
}

// Ayu bell colors
const BLUE: Color = Color::Rgb(0x59, 0xc2, 0xff);
const PURPLE: Color = Color::Rgb(0xd2, 0xa6, 0xff);
const ORANGE: Color = Color::Rgb(0xff, 0x8f, 0x40);
const YELLOW: Color = Color::Rgb(0xe6, 0xb4, 0x50);
const GREEN: Color = Color::Rgb(0xaa, 0xd9, 0x4c);
const RED: Color = Color::Rgb(0xff, 0x33, 0x33);

const SAVED_OUTPUT: usize = 10;

use std::collections::{HashMap, VecDeque};

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
}

enum Mode {
    All,
    OnlyRegister,
    OnlyStack,
    OnlyInstructions,
    OnlyOutput,
}

struct App {
    mode: Mode,
    input: Input,
    input_mode: InputMode,
    messages: LimitedBuffer<String>,
    current_pc: Arc<Mutex<u64>>, // TODO: replace with AtomicU64?
    output_scroll: usize,
    output: Arc<Mutex<Vec<String>>>,
    stream_output_prompt: Arc<Mutex<String>>,
    gdb_stdin: Arc<Mutex<dyn Write + Send>>,
    register_changed: Arc<Mutex<Vec<u8>>>,
    register_names: Arc<Mutex<Vec<String>>>,
    registers: Arc<Mutex<Vec<(String, Option<Register>)>>>,
    stack: Arc<Mutex<HashMap<u64, u64>>>,
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
                        .args(["--interpreter=mi2", "--quiet"])
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
            mode: Mode::All,
            input: Input::default(),
            input_mode: InputMode::Normal,
            messages: LimitedBuffer::new(10),
            current_pc: Arc::new(Mutex::new(0)),
            output_scroll: 0,
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

    let gdb_stdin_arc = Arc::clone(&app.gdb_stdin);
    let current_pc_arc = Arc::clone(&app.current_pc);
    let output_arc = Arc::clone(&app.output);
    let stream_output_prompt_arc = Arc::clone(&app.stream_output_prompt);
    let register_changed_arc = Arc::clone(&app.register_changed);
    let register_names_arc = Arc::clone(&app.register_names);
    let registers_arc = Arc::clone(&app.registers);
    let stack_arc = Arc::clone(&app.stack);
    let asm_arc = Arc::clone(&app.asm);

    // Thread to read GDB output and parse it
    thread::spawn(move || {
        gdb_interact(
            gdb_stdout,
            register_changed_arc,
            register_names_arc,
            registers_arc,
            current_pc_arc,
            stack_arc,
            asm_arc,
            gdb_stdin_arc,
            output_arc,
            stream_output_prompt_arc,
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

fn gdb_interact(
    gdb_stdout: BufReader<Box<dyn Read + Send>>,
    register_changed_arc: Arc<Mutex<Vec<u8>>>,
    register_names_arc: Arc<Mutex<Vec<String>>>,
    registers_arc: Arc<Mutex<Vec<(String, Option<Register>)>>>,
    current_pc_arc: Arc<Mutex<u64>>,
    stack_arc: Arc<Mutex<HashMap<u64, u64>>>,
    asm_arc: Arc<Mutex<Vec<Asm>>>,
    gdb_stdin_arc: Arc<Mutex<dyn Write + Send>>,
    output_arc: Arc<Mutex<Vec<String>>>,
    stream_output_prompt_arc: Arc<Mutex<String>>,
) {
    let mut next_write = vec![String::new()];
    for line in gdb_stdout.lines() {
        if let Ok(line) = line {
            let response = mi::parse_mi_response(&line);
            // TODO: I really hate the flow of this function, the reading and writing should be split into some
            // sort of state machine instead of just writing stuff and hoping the next state makes us read the right thing...
            debug!("response {:?}", response);
            match &response {
                MIResponse::AsyncRecord(reason, v) => {
                    if reason == "stopped" {
                        debug!("{v:?}");
                        // TODO: we could cache this, per file opened
                        if let Some(arch) = v.get("arch") {
                            debug!("{arch}");
                        }
                        // TODO: we could cache this, per file opened
                        next_write.push("-data-list-register-names".to_string());
                        // When a breakpoint is hit, query for register values
                        next_write.push("-data-list-register-values x".to_string());
                        next_write.push("-data-list-changed-registers".to_string());
                    }
                }
                MIResponse::ExecResult(status, kv) => {
                    if status == "running" {
                        // TODO: this causes a bunch of re-drawing, but
                        // I'm sure in the future we could make sure we are leaving our own
                        // state or something?

                        // reset the stack
                        let mut stack = stack_arc.lock().unwrap();
                        stack.clear();

                        // reset the asm
                        let mut asm = asm_arc.lock().unwrap();
                        asm.clear();

                        // reset the regs
                        let mut regs = registers_arc.lock().unwrap();
                        regs.clear();
                    }

                    if let Some(value) = kv.get("value") {
                        // This works b/c we only use this for PC, but will most likely
                        // be wrong sometime
                        let mut cur_pc_lock = current_pc_arc.lock().unwrap();
                        let pc: Vec<&str> = value.split_whitespace().collect();
                        let pc = pc[0].strip_prefix("0x").unwrap();
                        *cur_pc_lock = u64::from_str_radix(pc, 16).unwrap();
                    } else if let Some(register_names) = kv.get("register-names") {
                        let register_names = parse_register_names_values(register_names);
                        let mut regs_names = register_names_arc.lock().unwrap();
                        *regs_names = register_names;
                    } else if let Some(changed_registers) = kv.get("changed-registers") {
                        let changed_registers = parse_register_names_values(changed_registers);
                        debug!("cr: {:?}", changed_registers);
                        let result: Vec<u8> = changed_registers
                            .iter()
                            .map(|s| s.parse::<u8>().expect("Invalid number"))
                            .collect();
                        let mut reg_changed = register_changed_arc.lock().unwrap();
                        *reg_changed = result;
                    } else if let Some(register_values) = kv.get("register-values") {
                        // parse the response and save it
                        let registers = parse_register_values(register_values);
                        let mut regs = registers_arc.lock().unwrap();
                        let regs_names = register_names_arc.lock().unwrap();
                        let registers = join_registers(&regs_names, &registers);
                        *regs = registers.clone();

                        // assuming we have a valid $pc, get the bytes
                        let val = read_pc_value();
                        next_write.push(val);

                        // assuming we have a valid $sp, get the bytes
                        next_write.push(data_read_sp_bytes(0, 8));
                        next_write.push(data_read_sp_bytes(8, 8));
                        next_write.push(data_read_sp_bytes(16, 8));
                        next_write.push(data_read_sp_bytes(24, 8));
                        next_write.push(data_read_sp_bytes(32, 8));
                        next_write.push(data_read_sp_bytes(40, 8));
                        next_write.push(data_read_sp_bytes(48, 8));
                        next_write.push(data_read_sp_bytes(56, 8));
                        next_write.push(data_read_sp_bytes(62, 8));
                        next_write.push(data_read_sp_bytes(70, 8));
                        next_write.push(data_read_sp_bytes(78, 8));
                        next_write.push(data_read_sp_bytes(86, 8));
                        next_write.push(data_read_sp_bytes(94, 8));

                        // update current asm at pc
                        let instruction_length = 8;
                        next_write.push(data_disassemble(
                            instruction_length * 5,
                            instruction_length * 15,
                        ));
                    } else if let Some(memory) = kv.get("memory") {
                        let mut stack = stack_arc.lock().unwrap();
                        let mem_str = memory.strip_prefix(r#"[{"#).unwrap();
                        let mem_str = mem_str.strip_suffix(r#"}]"#).unwrap();
                        let data = parse_key_value_pairs(mem_str);
                        let begin = data["begin"].to_string();
                        let begin = begin.strip_prefix("0x").unwrap();
                        debug!("{:?}", data);
                        debug!("{}", begin);
                        // TODO: this is insane and should be cached
                        stack.insert(
                            u64::from_str_radix(begin, 16).unwrap(),
                            u64::from_str_radix(&data["contents"], 16).unwrap(),
                        );
                        debug!("{:?}", data);
                    } else if let Some(asm) = kv.get("asm_insns") {
                        let new_asms = parse_asm_insns_values(asm);
                        let mut asm = asm_arc.lock().unwrap();
                        *asm = new_asms.clone();
                    }
                }
                MIResponse::StreamOutput(t, s) => {
                    // let split: Vec<String> = s.to_string().split('\n').collect();
                    let mut split: Vec<String> =
                        s.split('\n').map(String::from).map(|a| a.trim_end().to_string()).collect();
                    for s in split {
                        if !s.is_empty() {
                            debug!("{s}");
                            output_arc.lock().unwrap().push(s);
                        }
                    }

                    // console-stream-output
                    if t == "~" {
                        if !s.contains('\n') {
                            let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
                            *stream_lock = s.to_string();
                        }
                    }
                }
                MIResponse::Unknown(s) => {
                    let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
                    *stream_lock = s.to_string();
                }
                _ => (),
            }
            if !next_write.is_empty() {
                for w in &next_write {
                    let mut stdin = gdb_stdin_arc.lock().unwrap();
                    debug!("writing {}", w);
                    writeln!(stdin, "{}", w).expect("Failed to send command");
                }
                next_write.clear();
            }
        }
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

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

        if let Some(val) = app.messages.as_slice().iter().last() {
            let mut stdin = app.gdb_stdin.lock().unwrap();
            writeln!(stdin, "{}", val)?;
            debug!("writing {}", val);
            app.input.reset();
        }
    } else {
        app.messages.offset = 0;
        app.messages.push(app.input.value().into());
        let mut stdin = app.gdb_stdin.lock().unwrap();
        writeln!(stdin, "{}", app.input.value())?;
        debug!("writing {}", app.input.value());
        app.input.reset();
    }

    Ok(())
}

fn update_from_previous_input(app: &mut App) {
    if app.messages.buffer.len() >= app.messages.offset {
        if let Some(msg) = app.messages.buffer.get(app.messages.buffer.len() - app.messages.offset)
        {
            app.input = Input::new(msg.clone())
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    // TODO: register size should depend on arch
    let top_size = Fill(1);

    // If only output, then no top and fill all with output
    if let Mode::OnlyOutput = app.mode {
        let output_size = Fill(1);
        let vertical = Layout::vertical([Length(2), output_size, Length(3)]);
        let [title_area, output, input] = vertical.areas(f.area());

        draw_title_area(app, f, title_area);
        draw_output(app, f, output, true);
        draw_input(title_area, app, f, input);
        return;
    }

    // the rest will include the top
    let output_size = Length(SAVED_OUTPUT as u16);

    let vertical = Layout::vertical([Length(2), top_size, output_size, Length(3)]);
    let [title_area, top, output, input] = vertical.areas(f.area());

    draw_title_area(app, f, title_area);
    draw_output(app, f, output, false);
    draw_input(title_area, app, f, input);

    match app.mode {
        Mode::All => {
            let register_size = Min(30);
            let stack_size = Min(10);
            let asm_size = Min(15);
            let vertical = Layout::vertical([register_size, stack_size, asm_size]);
            let [register, stack, asm] = vertical.areas(top);

            draw_registers(app, f, register);
            draw_stack(app, f, stack);
            draw_asm(app, f, asm);
        }
        Mode::OnlyRegister => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_registers(app, f, all);
        }
        Mode::OnlyStack => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_stack(app, f, all);
        }
        Mode::OnlyInstructions => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_asm(app, f, all);
        }
        _ => (),
    }
}

fn draw_input(title_area: Rect, app: &App, f: &mut Frame, input: Rect) {
    // Input
    let width = title_area.width.max(3) - 3;
    // keep 2 for borders and 1 for cursor

    let scroll = app.input.visual_scroll(width as usize);
    let stream_lock = app.stream_output_prompt.lock().unwrap();
    let prompt_len = stream_lock.len();

    let txt_input = Paragraph::new(format!("{}{}", stream_lock, app.input.value()))
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(GREEN),
        })
        .scroll((0, scroll as u16))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input".fg(YELLOW).add_modifier(Modifier::BOLD)),
        );
    f.render_widget(txt_input, input);
    match app.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor_position((
                // Put cursor past the end of the input text
                input.x
                    + ((app.input.visual_cursor()).max(scroll) - scroll) as u16
                    + 1
                    + prompt_len as u16,
                // Move one line down, from the border to the input line
                input.y + 1,
            ))
        }
    }
}

fn draw_output(app: &App, f: &mut Frame, output: Rect, full: bool) {
    let output_lock = app.output.lock().unwrap();

    let len = output_lock.len();
    let max = output.height;
    let skip = if full {
        if len <= max as usize {
            0
        } else {
            app.output_scroll
        }
    } else {
        if len <= max as usize {
            0
        } else {
            len - max as usize + 2
        }
    };

    let outputs: Vec<ListItem> = output_lock
        .iter()
        .skip(skip)
        .take(max as usize)
        .map(|m| {
            let m = m.replace('\t', "    ");
            let content = vec![Line::from(Span::raw(format!("{}", m)))];
            ListItem::new(content)
        })
        .collect();
    let help = if full { "(up(k), down(j), 50 up(K), 50 down(J))" } else { "" };
    let output_block = List::new(outputs).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Output {help}").fg(BLUE).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(output_block, output);
}

fn draw_asm(app: &App, f: &mut Frame, asm: Rect) {
    // Asm
    // TODO: cache the pc_index if this doesn't change
    let mut rows = vec![];
    let mut pc_index = None;
    let mut function_name = None;
    if let Ok(asm) = app.asm.lock() {
        let mut entries: Vec<_> = asm.clone().into_iter().collect();
        entries.sort_by(|a, b| a.address.cmp(&b.address));
        let mut index = 0;
        let app_cur_lock = app.current_pc.lock().unwrap();
        for a in entries.iter() {
            if a.address == *app_cur_lock {
                pc_index = Some(index);
                if let Some(func_name) = &a.func_name {
                    function_name = Some(func_name.clone());
                }
            }
            let addr_cell =
                Cell::from(format!("0x{:02x}", a.address)).style(Style::default().fg(PURPLE));
            let mut row = vec![addr_cell];

            if let Some(function_name) = &a.func_name {
                let function_cell = Cell::from(format!("{}+{:02x}", function_name, a.offset))
                    .style(Style::default().fg(PURPLE));
                row.push(function_cell);
            } else {
                row.push(Cell::from(""));
            }

            let inst_cell = if let Some(pc_index) = pc_index {
                if pc_index == index {
                    Cell::from(a.inst.to_string()).fg(GREEN)
                } else {
                    Cell::from(a.inst.to_string()).white()
                }
            } else {
                Cell::from(a.inst.to_string()).dark_gray()
            };
            row.push(inst_cell);

            rows.push(Row::new(row));
            index += 1;
        }
    }

    let tital = if let Some(function_name) = function_name {
        Title::from(format!("Instructions ({})", function_name).fg(ORANGE))
    } else {
        Title::from("Instructions".fg(ORANGE))
    };
    if let Some(pc_index) = pc_index {
        let widths = [Constraint::Length(16), Constraint::Percentage(10), Fill(1)];
        let table = Table::new(rows, widths)
            .block(Block::default().borders(Borders::TOP).title(tital).add_modifier(Modifier::BOLD))
            .row_highlight_style(Style::new().fg(GREEN))
            .highlight_symbol(">>");
        let start_offset = if pc_index < 5 { 0 } else { pc_index - 5 };
        let mut table_state =
            TableState::default().with_offset(start_offset).with_selected(pc_index);
        f.render_stateful_widget(table, asm, &mut table_state);
    } else {
        let block =
            Block::default().borders(Borders::TOP).title(tital).add_modifier(Modifier::BOLD);
        f.render_widget(block, asm);
    }
}

fn draw_title_area(app: &App, f: &mut Frame, title_area: Rect) {
    let vertical_title = Layout::vertical([Length(1), Length(1)]);
    let [first, second] = vertical_title.areas(title_area);
    f.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .title(vec![
                "|".fg(Color::Rgb(100, 100, 100)),
                env!("CARGO_PKG_NAME").bold(),
                "-".fg(Color::Rgb(100, 100, 100)),
                "v".into(),
                env!("CARGO_PKG_VERSION").into(),
                "|".fg(Color::Rgb(100, 100, 100)),
            ])
            .title_alignment(Alignment::Center),
        first,
    );
    // Title Area
    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to enter input | "),
                Span::styled("F1", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" main | "),
                Span::styled("F2", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" registers | "),
                Span::styled("F3", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" stacks | "),
                Span::styled("F4", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" instructions | "),
                Span::styled("F5", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" output"),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send input | "),
                Span::styled("F1", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" main | "),
                Span::styled("F2", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" registers | "),
                Span::styled("F3", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" stacks | "),
                Span::styled("F4", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" instructions | "),
                Span::styled("F5", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" output"),
            ],
            Style::default(),
        ),
    };
    let text = Text::from(Line::from(msg)).style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, second);
}

fn draw_stack(app: &App, f: &mut Frame, stack: Rect) {
    // Stack
    let mut rows = vec![];
    if let Ok(stack) = app.stack.lock() {
        let mut entries: Vec<_> = stack.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (addr, value) in entries.iter() {
            rows.push(Row::new(vec![
                Cell::from(format!("0x{:02x}", addr)).style(Style::new().fg(PURPLE)),
                Cell::from(format!("0x{:02x}", value)),
            ]));
        }
    }

    let widths = [Constraint::Length(16), Fill(1)];
    let table = Table::new(rows, widths).block(
        Block::default()
            .borders(Borders::TOP)
            .title("Stack".fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(table, stack);
}

fn draw_registers(app: &App, f: &mut Frame, register: Rect) {
    // Registers
    let mut rows = vec![];
    let reg_changed_lock = app.register_changed.lock().unwrap();
    if let Ok(regs) = app.registers.lock() {
        for (i, (name, register)) in regs.iter().enumerate() {
            if let Some(reg) = register {
                if reg.value == Some("<unavailable>".to_string()) {
                    continue;
                }
                let changed = reg_changed_lock.contains(&(i as u8));
                let mut addr = Cell::from(name.to_string()).style(Style::new().fg(PURPLE));
                let val = Cell::from(reg.value.clone().unwrap());
                if changed {
                    addr = addr.style(Style::new().fg(RED));
                }
                rows.push(Row::new(vec![addr, val]));
            }
        }
    }

    let widths = [Constraint::Length(5), Constraint::Length(20)];
    let table = Table::new(rows, widths).block(
        Block::default()
            .borders(Borders::TOP)
            .title("Registers".fg(ORANGE))
            .add_modifier(Modifier::BOLD),
    );

    f.render_widget(table, register);
}
