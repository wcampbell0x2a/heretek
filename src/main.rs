use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Define MI Response Types
#[derive(Debug)]
enum MIResponse {
    ExecResult(String, HashMap<String, String>), // E.g., ^done,key="value"
    AsyncRecord(String, HashMap<String, String>), // E.g., *stopped,key="value"
    Notify(String, HashMap<String, String>),     // E.g., =thread-created,key="value"
    StreamOutput(String, String),                // E.g., ~"output text\n"
    Unknown(String),                             // Unknown/unsupported lines
}

/// Parse a single GDB/MI line into MIResponse
fn parse_mi_response(line: &str) -> MIResponse {
    if line.starts_with('^') {
        parse_exec_result(&line[1..]) // Remove the `^` prefix
    } else if line.starts_with('*') {
        parse_async_record(&line[1..]) // Remove the `*` prefix
    } else if line.starts_with('=') {
        parse_notify(&line[1..]) // Remove the `=` prefix
    } else if line.starts_with('~') || line.starts_with('@') || line.starts_with('&') {
        parse_stream_output(line)
    } else {
        MIResponse::Unknown(line.to_string())
    }
}

// Helper to parse key-value pairs
fn parse_key_value_pairs(input: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let pairs: Vec<&str> = input.split(',').collect();

    for pair in pairs {
        if let Some((key, value)) = pair.split_once('=') {
            result.insert(key.to_string(), value.trim_matches('"').to_string());
        }
    }
    result
}

// Exec Result
fn parse_exec_result(input: &str) -> MIResponse {
    if let Some((status, rest)) = input.split_once(',') {
        let data = parse_key_value_pairs(rest);
        MIResponse::ExecResult(status.to_string(), data)
    } else {
        MIResponse::ExecResult(input.to_string(), HashMap::new())
    }
}

// Async Record
fn parse_async_record(input: &str) -> MIResponse {
    if let Some((reason, rest)) = input.split_once(',') {
        let data = parse_key_value_pairs(rest);
        MIResponse::AsyncRecord(reason.to_string(), data)
    } else {
        MIResponse::AsyncRecord(input.to_string(), HashMap::new())
    }
}

// Notify
fn parse_notify(input: &str) -> MIResponse {
    if let Some((event, rest)) = input.split_once(',') {
        let data = parse_key_value_pairs(rest);
        MIResponse::Notify(event.to_string(), data)
    } else {
        MIResponse::Notify(input.to_string(), HashMap::new())
    }
}

// Stream Output
fn parse_stream_output(input: &str) -> MIResponse {
    let (kind, content) = input.split_at(1);
    let output = content.trim_matches('"').to_string();
    MIResponse::StreamOutput(kind.to_string(), output)
}

fn main() {
    // Start GDB in MI mode
    let mut gdb_process = Command::new("gdb")
        .args(["--interpreter=mi2", "--quiet"]) // MI2 mode, no GDB banner
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start GDB process");

    // Access GDB stdin and stdout
    let stdin = gdb_process
        .stdin
        .as_mut()
        .expect("Failed to access GDB stdin");
    let stdout = gdb_process
        .stdout
        .as_mut()
        .expect("Failed to access GDB stdout");

    // Send GDB commands
    writeln!(stdin, "-file-exec-and-symbols /bin/ls").expect("Failed to load binary");
    writeln!(stdin, "-break-insert main").expect("Failed to set breakpoint at main");
    writeln!(stdin, "-exec-run").expect("Failed to start execution");

    // Read and parse GDB output
    let reader = BufReader::new(stdout);
    println!("GDB Output:");

    for line in reader.lines() {
        let line = line.expect("Failed to read line");

        // Parse the GDB/MI response
        let parsed = parse_mi_response(&line);
        println!("{:?}", parsed);

        // Example: Stop reading if execution is stopped
        if matches!(parsed, MIResponse::AsyncRecord(ref reason, _) if reason == "stopped") {
            println!("Program stopped at a breakpoint!");
            break;
        }
    }
}
