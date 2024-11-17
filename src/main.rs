// use crossterm::{
//     event::{self, Event, KeyCode},
//     execute,
//     terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
// };
// use ratatui::{
//     backend::CrosstermBackend,
//     layout::{Constraint, Direction, Layout},
//     style::{Color, Style},
//     text::{Line, Span, Text},
//     widgets::{Block, Borders, Paragraph},
//     Terminal,
// };
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Define MI Response Types
#[derive(Debug)]
enum MIResponse {
    ExecResult(String, HashMap<String, String>),
    AsyncRecord(String, HashMap<String, String>),
    Notify(String, HashMap<String, String>),
    StreamOutput(String, String),
    Unknown(String),
}

// Parse a single GDB/MI line into MIResponse
fn parse_mi_response(line: &str) -> MIResponse {
    if line.starts_with('^') {
        parse_exec_result(&line[1..])
    } else if line.starts_with('*') {
        parse_async_record(&line[1..])
    } else if line.starts_with('=') {
        parse_notify(&line[1..])
    } else if line.starts_with('~') || line.starts_with('@') || line.starts_with('&') {
        parse_stream_output(line)
    } else {
        MIResponse::Unknown(line.to_string())
    }
}

// Helper to parse key-value pairs
fn parse_key_value_pairs(input: &str) -> HashMap<String, String> {
    input
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_string(), value.trim_matches('"').to_string()))
        .collect()
}

fn parse_exec_result(input: &str) -> MIResponse {
    if let Some((status, rest)) = input.split_once(',') {
        MIResponse::ExecResult(status.to_string(), parse_key_value_pairs(rest))
    } else {
        MIResponse::ExecResult(input.to_string(), HashMap::new())
    }
}

fn parse_async_record(input: &str) -> MIResponse {
    if let Some((reason, rest)) = input.split_once(',') {
        MIResponse::AsyncRecord(reason.to_string(), parse_key_value_pairs(rest))
    } else {
        MIResponse::AsyncRecord(input.to_string(), HashMap::new())
    }
}

fn parse_notify(input: &str) -> MIResponse {
    if let Some((event, rest)) = input.split_once(',') {
        MIResponse::Notify(event.to_string(), parse_key_value_pairs(rest))
    } else {
        MIResponse::Notify(input.to_string(), HashMap::new())
    }
}

fn parse_stream_output(input: &str) -> MIResponse {
    let (kind, content) = input.split_at(1);
    MIResponse::StreamOutput(kind.to_string(), content.trim_matches('"').to_string())
}

/// This example is taken from https://raw.githubusercontent.com/fdehau/tui-rs/master/examples/user_input.rs
use ratatui::prelude::*;
use ratatui::{
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::{error::Error, io};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

enum InputMode {
    Normal,
    Editing,
}

use std::collections::VecDeque;

struct LimitedBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> LimitedBuffer<T> {
    fn new(capacity: usize) -> Self {
        Self {
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
    parsed_responses: Arc<Mutex<LimitedBuffer<MIResponse>>>,
    gdb_stdin: Arc<Mutex<ChildStdin>>,
}

impl App {
    fn new(gdb_stdin: ChildStdin) -> App {
        let gdb_stdin = Arc::new(Mutex::new(gdb_stdin));
        App {
            input: Input::default(),
            input_mode: InputMode::Normal,
            messages: LimitedBuffer::new(10),
            parsed_responses: Arc::new(Mutex::new(LimitedBuffer::new(30))),
            gdb_stdin,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
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

    // create app and run it
    let app = App::new(gdb_stdin);

    let parsed_reponses_arc = Arc::clone(&app.parsed_responses);

    // Thread to read GDB output and parse it
    thread::spawn(move || {
        for line in gdb_stdout.lines() {
            // println!("{:?}", line);
            if let Ok(line) = line {
                let response = parse_mi_response(&line);
                // if let MIResponse::Unknown(content) = &response {
                //     let mut unknown = unknown_text_clone.lock().unwrap();
                //     unknown.push_str(content);
                // }
                parsed_reponses_arc.lock().unwrap().push(response);
            }
        }
    });
    let res = run_app(&mut terminal, app);

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('i') => {
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            app.messages.push(app.input.value().into());
                            let mut stdin = app.gdb_stdin.lock().unwrap();
                            // println!("{}", app.input.value());
                            writeln!(stdin, "{}", app.input.value())?;
                            app.input.reset();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
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

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(50),
                Constraint::Length(50),
            ]
            .as_ref(),
        )
        .split(f.area());

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to start editing."),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to record the message"),
            ],
            Style::default(),
        ),
    };
    let text = Text::from(Line::from(msg)).style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[0]);

    let width = chunks[0].width.max(3) - 3; // keep 2 for borders and 1 for cursor

    let scroll = app.input.visual_scroll(width as usize);
    let input = Paragraph::new(app.input.value())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .scroll((0, scroll as u16))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);

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
                chunks[1].x + ((app.input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            ))
        }
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
    let messages =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages"));
    f.render_widget(messages, chunks[2]);

    let response_widget = Paragraph::new(response_text).block(
        Block::default()
            .title("Parsed Responses")
            .borders(Borders::ALL),
    );
    f.render_widget(response_widget, chunks[3]);
}
