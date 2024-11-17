use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start GDB process
    let mut gdb_process = Command::new("gdb")
        .args(["--interpreter=mi2", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start GDB");

    let gdb_stdin = Arc::new(Mutex::new(gdb_process.stdin.take().unwrap()));
    let gdb_stdout = BufReader::new(gdb_process.stdout.take().unwrap());
    let parsed_responses = Arc::new(Mutex::new(Vec::new()));
    let responses_clone = Arc::clone(&parsed_responses);

    // Thread to read GDB output and parse it
    thread::spawn(move || {
        for line in gdb_stdout.lines() {
            if let Ok(line) = line {
                let response = parse_mi_response(&line);
                responses_clone.lock().unwrap().push(response);
            }
        }
    });

    // Setup terminal UI
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut user_input = String::new();

    loop {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
                .split(f.area());

            // Display parsed responses
            let response_text = {
                let responses = parsed_responses.lock().unwrap();
                Text::from(
                    responses
                        .iter()
                        .map(|response| Line::from(Span::raw(format!("{:?}", response))))
                        .collect::<Vec<_>>(),
                )
            };

            let response_widget = Paragraph::new(response_text).block(
                Block::default()
                    .title("Parsed Responses")
                    .borders(Borders::ALL),
            );
            f.render_widget(response_widget, chunks[0]);

            // Display input box
            let input_widget = Paragraph::new(user_input.clone())
                .block(Block::default().title("Input").borders(Borders::ALL))
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(input_widget, chunks[1]);
        })?;

        // Handle user input
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char(c) => {
                    user_input.push(c);
                }
                KeyCode::Enter => {
                    // Write input to GDB process
                    if !user_input.is_empty() {
                        let mut stdin = gdb_stdin.lock().unwrap();
                        writeln!(stdin, "{}", user_input)?;
                        user_input.clear();
                    }
                }
                KeyCode::Backspace => {
                    user_input.pop();
                }
                KeyCode::Esc => {
                    break;
                }
                _ => {}
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
