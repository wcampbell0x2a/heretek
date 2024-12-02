use mi::{
    data_disassemble, data_read_memory_bytes, parse_asm_insns_values, parse_key_value_pairs,
    parse_register_values, register_x86_64, Asm, MIResponse, Register,
};
use ratatui::widgets::{Cell, Row, Table, TableState};
use regex::Regex;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::{error::Error, io};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use env_logger::{Builder, Env};
use log::debug;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::{
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use Constraint::{Fill, Length, Max, Min};

mod mi;

enum InputMode {
    Normal,
    Editing,
}

use std::collections::{HashMap, VecDeque};

struct LimitedBuffer<T> {
    offset: usize,
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> LimitedBuffer<T> {
    fn new(capacity: usize) -> Self {
        Self {
            offset: 0,
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, value: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front(); // Remove the oldest element
        }
        self.buffer.push_back(value);
    }

    fn as_slice(&self) -> &[T] {
        self.buffer.as_slices().0
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: Input,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: LimitedBuffer<String>,
    current_pc: Arc<Mutex<u64>>, // TODO: replace with AtomicU64?
    parsed_responses: Arc<Mutex<LimitedBuffer<MIResponse>>>,
    gdb_stdin: Arc<Mutex<ChildStdin>>,
    registers: Arc<Mutex<Vec<(String, Register)>>>,
    stack: Arc<Mutex<HashMap<u64, u64>>>,
    asm: Arc<Mutex<Vec<Asm>>>,
}

impl App {
    fn new(gdb_stdin: Arc<Mutex<ChildStdin>>) -> App {
        App {
            input: Input::default(),
            input_mode: InputMode::Normal,
            messages: LimitedBuffer::new(10),
            current_pc: Arc::new(Mutex::new(0)),
            parsed_responses: Arc::new(Mutex::new(LimitedBuffer::new(30))),
            gdb_stdin,
            registers: Arc::new(Mutex::new(vec![])),
            stack: Arc::new(Mutex::new(HashMap::new())),
            asm: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
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

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Start GDB process
    let mut gdb_process = Command::new("gdb")
        .args(["--interpreter=mi2", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start GDB");
    // let gdb_stdin = Arc::new(Mutex::new(gdb_process.stdin.take().unwrap()));
    let gdb_stdin = gdb_process.stdin.take().unwrap();
    let gdb_stdout = BufReader::new(gdb_process.stdout.take().unwrap());
    let gdb_stdin = Arc::new(Mutex::new(gdb_stdin));

    // create app and run it
    let mut app = App::new(gdb_stdin);

    let gdb_stdin_arc = Arc::clone(&app.gdb_stdin);
    let current_pc_arc = Arc::clone(&app.current_pc);
    let parsed_reponses_arc = Arc::clone(&app.parsed_responses);
    let registers_arc = Arc::clone(&app.registers);
    let stack_arc = Arc::clone(&app.stack);
    let asm_arc = Arc::clone(&app.asm);

    // Thread to read GDB output and parse it
    thread::spawn(move || {
        let mut next_write = vec![String::new()];
        for line in gdb_stdout.lines() {
            if let Ok(line) = line {
                let response = mi::parse_mi_response(&line);
                match &response {
                    MIResponse::AsyncRecord(reason, v) => {
                        if reason == "stopped" {
                            debug!("{v:?}");
                            if let Some(arch) = v.get("arch") {
                                debug!("{arch}");
                            }
                            // When a breakpoint is hit, query for register values
                            next_write.push("-data-list-register-values x".to_string());
                        }
                    }
                    MIResponse::ExecResult(_, kv) => {
                        if let Some(register_values) = kv.get("register-values") {
                            let registers = parse_register_values(&register_values);
                            // Check if response is register data
                            let mut regs = registers_arc.lock().unwrap();
                            let registers = register_x86_64(&registers);
                            for s in registers.iter() {
                                if s.0 == "rsp" {
                                    let start_addr = s.1.value.as_ref().unwrap();
                                    next_write.push(data_read_memory_bytes(start_addr, 0, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 8, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 16, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 24, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 32, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 40, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 48, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 56, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 62, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 70, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 78, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 86, 8));
                                    next_write.push(data_read_memory_bytes(start_addr, 94, 8));
                                }
                                if s.0 == "rip" {
                                    let val =
                                        s.1.value.as_ref().unwrap().strip_prefix("0x").unwrap();
                                    let mut cur_pc_lock = current_pc_arc.lock().unwrap();
                                    *cur_pc_lock = u64::from_str_radix(val, 16).unwrap();
                                }
                            }
                            *regs = registers.clone();
                            let mut stack = stack_arc.lock().unwrap();
                            stack.clear();

                            // update current asm at pc
                            let instruction_length = 8;
                            next_write.push(data_disassemble(
                                instruction_length * 5,
                                instruction_length * 15,
                            ));
                            let mut asm = asm_arc.lock().unwrap();
                            asm.clear();
                        } else if let Some(memory) = kv.get("memory") {
                            let mut stack = stack_arc.lock().unwrap();
                            let mem_str = memory.strip_prefix(r#"[{"#).unwrap();
                            let mem_str = mem_str.strip_suffix(r#"}]"#).unwrap();
                            let data = parse_key_value_pairs(mem_str);
                            let begin = data["begin"].to_string();
                            let begin = begin.strip_prefix("0x").unwrap();
                            debug!("{:?}", data);
                            debug!("{}", data["contents"]);
                            debug!("{}", begin);
                            stack.insert(
                                u64::from_str_radix(begin, 16).unwrap(),
                                u64::from_str_radix(&data["contents"], 16).unwrap(),
                            );
                            debug!("{:?}", data);
                        } else if let Some(asm) = kv.get("asm_insns") {
                            let new_asms = parse_asm_insns_values(&asm);
                            let mut asm = asm_arc.lock().unwrap();
                            *asm = new_asms.clone();
                        }
                    }
                    MIResponse::Unknown(_) => {
                        if !next_write.is_empty() {
                            for w in &next_write {
                                let mut stdin = gdb_stdin_arc.lock().unwrap();
                                debug!("writing {}", w);
                                writeln!(stdin, "{}", w).expect("Failed to send command");
                            }
                            next_write.clear();
                        }
                    }
                    _ => (),
                }
                debug!("response {:?}", response);
                parsed_reponses_arc.lock().unwrap().push(response);
            }
        }
    });
    let res = run_app(&mut terminal, &mut app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Enter => {
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
                        }
                        KeyCode::Char('i') => {
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Down => {
                            if !app.messages.buffer.is_empty() {
                                if app.messages.offset != 0 {
                                    app.messages.offset -= 1;
                                } else {
                                    app.messages.offset = 0;
                                }
                                update_from_previous_input(app);
                            } else {
                                app.messages.offset = 0;
                            }
                        }
                        KeyCode::Up => {
                            if !app.messages.buffer.is_empty() {
                                if app.messages.offset < app.messages.buffer.len() {
                                    app.messages.offset += 1;
                                }
                                update_from_previous_input(app);
                            } else {
                                app.messages.offset = 0;
                            }
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        // TODO: Same as above
                        KeyCode::Enter => {
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
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        // TODO: Same as above
                        KeyCode::Down => {
                            if !app.messages.buffer.is_empty() {
                                if app.messages.offset != 0 {
                                    app.messages.offset -= 1;
                                } else {
                                    app.messages.offset = 0;
                                }
                                update_from_previous_input(app);
                            } else {
                                app.messages.offset = 0;
                            }
                        }
                        // TODO: Same as above
                        KeyCode::Up => {
                            if !app.messages.buffer.is_empty() {
                                if app.messages.offset < app.messages.buffer.len() {
                                    app.messages.offset += 1;
                                }
                                update_from_previous_input(app);
                            } else {
                                app.messages.offset = 0;
                            }
                        }
                        _ => {
                            app.input.handle_event(&Event::Key(key));
                        }
                    },
                }
            }
        }
    }
}

fn update_from_previous_input(app: &mut App) {
    if app.messages.buffer.len() >= app.messages.offset {
        if let Some(msg) = app
            .messages
            .buffer
            .get(app.messages.buffer.len() - app.messages.offset)
        {
            app.input = Input::new(msg.clone())
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let vertical = Layout::vertical([Length(1), Min(30), Length(20), Max(3)]);
    let [title_area, info, parsed, input] = vertical.areas(f.area());
    let horizontal = Layout::horizontal([Max(30), Fill(1), Min(80), Fill(1)]);
    let [register, stack, asm, other] = horizontal.areas(info);

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to enter input"),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send input"),
            ],
            Style::default(),
        ),
    };
    let text = Text::from(Line::from(msg)).style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, title_area);

    let width = title_area.width.max(3) - 3; // keep 2 for borders and 1 for cursor

    let scroll = app.input.visual_scroll(width as usize);
    let txt_input = Paragraph::new(app.input.value())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Blue),
        })
        .scroll((0, scroll as u16))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(txt_input, input);

    // Registers
    let mut rows = vec![];
    match app.registers.lock() {
        Ok(regs) => {
            for (name, register) in regs.iter() {
                rows.push(Row::new(vec![
                    name.to_string(),
                    register.value.clone().unwrap(),
                ]));
            }
        }
        Err(_) => (),
    }

    let widths = [Constraint::Length(5), Constraint::Length(20)];
    let table = Table::new(rows, widths).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Registers".blue()),
    );

    f.render_widget(table, register);

    // Stack
    let mut rows = vec![];
    match app.stack.lock() {
        Ok(stack) => {
            let mut entries: Vec<_> = stack.clone().into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (addr, value) in entries.iter() {
                rows.push(Row::new(vec![
                    Cell::from(format!("0x{:02x}", addr)).yellow(),
                    Cell::from(format!("0x{:02x}", value)),
                ]));
            }
        }
        Err(_) => (),
    }

    let widths = [Constraint::Length(16), Fill(1)];
    let table = Table::new(rows, widths)
        .block(Block::default().borders(Borders::ALL).title("Stack".blue()));

    f.render_widget(table, stack);

    // Display parsed responses
    let response_text = {
        let responses = app.parsed_responses.lock().unwrap();
        Text::from(
            responses
                .buffer
                .iter()
                .map(|response| Line::from(Span::raw(format!("{:?}", response))))
                .collect::<Vec<_>>(),
        )
    };
    match app.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor_position((
                // Put cursor past the end of the input text
                input.x + ((app.input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
                // Move one line down, from the border to the input line
                input.y + 1,
            ))
        }
    }

    // Asm
    let mut rows = vec![];
    let mut pc_index = None;
    match app.asm.lock() {
        Ok(asm) => {
            let mut entries: Vec<_> = asm.clone().into_iter().collect();
            entries.sort_by(|a, b| a.address.cmp(&b.address));
            let mut index = 0;
            let app_cur_lock = app.current_pc.lock().unwrap();
            for a in entries.iter() {
                if a.address == *app_cur_lock {
                    pc_index = Some(index);
                }
                rows.push(Row::new(vec![
                    Cell::from(format!("0x{:02x}", a.address)).yellow(),
                    Cell::from(format!("{}", a.inst)),
                ]));
                index += 1;
            }
        }
        Err(_) => (),
    }

    if let Some(pc_index) = pc_index {
        let widths = [Constraint::Length(16), Fill(1)];
        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Instructions".blue()),
            )
            .row_highlight_style(Style::new().reversed())
            .highlight_symbol(">>");

        // println!("{:?}", pc_index);
        let mut table_state = TableState::default()
            .with_offset(pc_index - 5)
            .with_selected(pc_index);
        f.render_stateful_widget(table, asm, &mut table_state);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Instructions".blue());
        f.render_widget(block, asm);
    }

    let messages: Vec<ListItem> = app
        .messages
        .buffer
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let content = vec![Line::from(Span::raw(format!("{}: {}", i, m)))];
            ListItem::new(content)
        })
        .collect();
    let messages = List::new(messages).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Messages".blue()),
    );
    f.render_widget(messages, other);

    let response_widget = Paragraph::new(response_text).block(
        Block::default()
            .title("Parsed Responses".blue())
            .borders(Borders::ALL),
    );
    f.render_widget(response_widget, parsed);
}
