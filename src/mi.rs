use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{error::Error, io};

use log::debug;
use regex::Regex;

// Define Register struct to hold register data
#[derive(Debug)]
pub struct Register {
    number: String,
    value: Option<String>,
    v2_int128: Option<String>,
    v8_int32: Option<String>,
    v4_int64: Option<String>,
    v8_float: Option<String>,
    v16_int8: Option<String>,
    v4_int32: Option<String>,
    error: Option<String>,
}

// Recursive function to parse key-value pairs
fn parse_key_value_pairs(input: &str) -> HashMap<String, String> {
    input
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_string(), value.trim_matches('"').to_string()))
        .collect()
}

// Function to parse register-values as an array of Registers
fn parse_register_values(input: &str) -> Vec<Register> {
    let mut registers = Vec::new();
    let re = Regex::new(r#"\{(.*?)\}"#).unwrap(); // Match entire register block

    // Capture each register block and parse it
    for capture in re.captures_iter(input) {
        let register_str = &capture[1];
        let mut register = Register {
            number: String::new(),
            value: None,
            v2_int128: None,
            v8_int32: None,
            v4_int64: None,
            v8_float: None,
            v16_int8: None,
            v4_int32: None,
            error: None,
        };

        let key_values = parse_key_value_pairs(register_str);
        for (key, val) in key_values {
            match key.as_str() {
                "number" => register.number = val,
                "value" => register.value = Some(val),
                "v2_int128" => register.v2_int128 = Some(val),
                "v8_int32" => register.v8_int32 = Some(val),
                "v4_int64" => register.v4_int64 = Some(val),
                "v8_float" => register.v8_float = Some(val),
                "v16_int8" => register.v16_int8 = Some(val),
                "v4_int32" => register.v4_int32 = Some(val),
                "error" => register.error = Some(val),
                _ => {}
            }
        }
        registers.push(register);
    }
    let register_names = vec![
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15", "rip", "eflags", "cs", "ss", "ds", "es", "fs", "gs",
    ];
    for (i, (register, name)) in registers.iter().zip(register_names.iter()).enumerate() {
        if !register.number.is_empty() {
            debug!("[{i}] register({name}): {:?}", register);
        }
    }
    registers
}

// MIResponse enum to represent different types of GDB responses
#[derive(Debug)]
pub enum MIResponse {
    ExecResult(String, HashMap<String, String>, Vec<Register>),
    AsyncRecord(String, HashMap<String, String>),
    Notify(String, HashMap<String, String>),
    StreamOutput(String, String),
    Unknown(String),
}

// Function to parse a single GDB/MI line into MIResponse
pub fn parse_mi_response(line: &str) -> MIResponse {
    debug!("{}", line);
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

// Helper function to parse ExecResult responses
fn parse_exec_result(input: &str) -> MIResponse {
    if let Some((status, rest)) = input.split_once(',') {
        let register_values = parse_register_values(rest); // Parse register values from the rest
        MIResponse::ExecResult(
            status.to_string(),
            parse_key_value_pairs(rest),
            register_values,
        )
    } else {
        MIResponse::ExecResult(input.to_string(), HashMap::new(), Vec::new())
    }
}

// Helper function to parse AsyncRecord responses
fn parse_async_record(input: &str) -> MIResponse {
    if let Some((reason, rest)) = input.split_once(',') {
        MIResponse::AsyncRecord(reason.to_string(), parse_key_value_pairs(rest))
    } else {
        MIResponse::AsyncRecord(input.to_string(), HashMap::new())
    }
}

// Helper function to parse Notify responses
fn parse_notify(input: &str) -> MIResponse {
    if let Some((event, rest)) = input.split_once(',') {
        MIResponse::Notify(event.to_string(), parse_key_value_pairs(rest))
    } else {
        MIResponse::Notify(input.to_string(), HashMap::new())
    }
}

// Helper function to parse StreamOutput responses
fn parse_stream_output(input: &str) -> MIResponse {
    let (kind, content) = input.split_at(1);
    MIResponse::StreamOutput(kind.to_string(), content.trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_result_register_values() {
        let input = r#"^done,register-values=[{number="0",value="0x0"},{number="1",value="0x1"}]"#;
        if let MIResponse::ExecResult(status, key_values, registers) = parse_mi_response(input) {
            assert_eq!(status, "done");
            assert!(key_values.contains_key("register-values"));
            assert_eq!(registers.len(), 2);

            assert_eq!(registers[0].number, "0");
            assert_eq!(registers[0].value.as_deref(), Some("0x0"));
            assert_eq!(registers[1].number, "1");
            assert_eq!(registers[1].value.as_deref(), Some("0x1"));
        } else {
            panic!("Expected ExecResult response");
        }
    }

    #[test]
    fn test_async_record() {
        let input = r#"*stopped,reason="breakpoint-hit",disp="keep",bkptno="1""#;
        if let MIResponse::AsyncRecord(reason, key_values) = parse_mi_response(input) {
            assert_eq!(reason, "stopped");
            assert_eq!(
                key_values.get("reason").map(|s| s.as_str()),
                Some("breakpoint-hit")
            );
            assert_eq!(key_values.get("disp").map(|s| s.as_str()), Some("keep"));
            assert_eq!(key_values.get("bkptno").map(|s| s.as_str()), Some("1"));
        } else {
            panic!("Expected AsyncRecord response");
        }
    }

    #[test]
    fn test_notify() {
        let input = r#"=thread-group-added,id="i1""#;
        if let MIResponse::Notify(event, key_values) = parse_mi_response(input) {
            assert_eq!(event, "thread-group-added");
            assert_eq!(key_values.get("id").map(|s| s.as_str()), Some("i1"));
        } else {
            panic!("Expected Notify response");
        }
    }

    #[test]
    fn test_stream_output() {
        let input = r#"~"GNU gdb (GDB) 12.1\n""#;
        if let MIResponse::StreamOutput(kind, content) = parse_mi_response(input) {
            assert_eq!(kind, "~");
            assert_eq!(content, "GNU gdb (GDB) 12.1\n");
        } else {
            panic!("Expected StreamOutput response");
        }
    }

    #[test]
    fn test_unknown_response() {
        let input = r#"unsupported-command-output"#;
        if let MIResponse::Unknown(response) = parse_mi_response(input) {
            assert_eq!(response, "unsupported-command-output");
        } else {
            panic!("Expected Unknown response");
        }
    }

    #[test]
    fn test_recursive_parsing() {
        let input = r#"^done,frame={addr="0x00007ffff7e04c48",func="printf",args=[],from="/usr/lib/libc.so.6"}"#;
        if let MIResponse::ExecResult(status, key_values, _) = parse_mi_response(input) {
            assert_eq!(status, "done");
            if let Some(frame_str) = key_values.get("frame") {
                let frame_values = parse_key_value_pairs(frame_str);
                assert_eq!(
                    frame_values.get("addr").map(|s| s.as_str()),
                    Some("0x00007ffff7e04c48")
                );
                assert_eq!(frame_values.get("func").map(|s| s.as_str()), Some("printf"));
                assert_eq!(
                    frame_values.get("from").map(|s| s.as_str()),
                    Some("/usr/lib/libc.so.6")
                );
            } else {
                panic!("Expected frame key in ExecResult");
            }
        } else {
            panic!("Expected ExecResult response");
        }
    }
}
