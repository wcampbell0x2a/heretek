use std::borrow::Cow;
use std::collections::HashMap;

use log::debug;
use regex::{CaptureMatches, Regex};

/// Amount of bytes requested during deref to get symbol/asm
pub const INSTRUCTION_LEN: usize = 8;

fn match_inner_items(haystack: &str) -> CaptureMatches {
    // compile once and re-use
    // NOTE: this only parses nested 3 {} deep, more and this will fail!
    static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r#"\{(?:[^}{]|\{(?:[^}{]|\{(?:[^}{]|\{[^}{]*\})*\})*\})*\}"#).unwrap()
    });
    RE.captures_iter(haystack)
}

/// Seen on gdb 15.1
pub const MEMORY_MAP_START_STR_NEW: [&str; 8] =
    ["Start", "Addr", "End", "Addr", "Size", "Offset", "Perms", "objfile"];

/// Seen on gdb 7.12
pub const MEMORY_MAP_START_STR_OLD: [&str; 7] =
    ["Start", "Addr", "End", "Addr", "Size", "Offset", "objfile"];

pub enum Mapping {
    New,
    Old,
}

#[derive(Debug, Clone)]
pub struct MemoryMapping {
    pub start_address: u64,
    pub end_address: u64,
    pub size: u64,
    pub offset: u64,
    pub permissions: Option<String>,
    pub path: Option<String>,
}

impl MemoryMapping {
    /// Mapping is the stack
    pub fn is_stack(&self) -> bool {
        self.path == Some("[stack]".to_owned())
    }

    /// Mapping is the heap
    pub fn is_heap(&self) -> bool {
        self.path == Some("[heap]".to_owned())
    }

    /// Mapping filepath matches `filepath`
    pub fn is_path(&self, filepath: &str) -> bool {
        self.path == Some(filepath.to_owned())
    }

    pub fn is_exec(&self) -> bool {
        if let Some(permissions) = &self.permissions {
            permissions.contains('x')
        } else {
            false
        }
    }

    /// Mapping contains the `addr`
    pub fn contains(&self, addr: u64) -> bool {
        (addr > self.start_address) && (addr < self.end_address)
    }
}

impl MemoryMapping {
    /// Parse from `MEMORY_MAP_START_STR_NEW`
    fn from_str_new(line: &str) -> Result<Self, String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 5 {
            Ok(MemoryMapping {
                start_address: u64::from_str_radix(&parts[0][2..], 16)
                    .map_err(|_| "Invalid start address")?,
                end_address: u64::from_str_radix(&parts[1][2..], 16)
                    .map_err(|_| "Invalid end address")?,
                size: u64::from_str_radix(&parts[2][2..], 16).map_err(|_| "Invalid size")?,
                offset: u64::from_str_radix(&parts[3][2..], 16).map_err(|_| "Invalid offset")?,
                permissions: Some(parts[4..].join(" ")), // Combine the rest as the path
                path: None,
            })
        } else if parts.len() == 6 {
            Ok(MemoryMapping {
                start_address: u64::from_str_radix(&parts[0][2..], 16)
                    .map_err(|_| "Invalid start address")?,
                end_address: u64::from_str_radix(&parts[1][2..], 16)
                    .map_err(|_| "Invalid end address")?,
                size: u64::from_str_radix(&parts[2][2..], 16).map_err(|_| "Invalid size")?,
                offset: u64::from_str_radix(&parts[3][2..], 16).map_err(|_| "Invalid offset")?,
                permissions: Some(parts[4].to_string()),
                path: Some(parts[5..].join(" ")), // Combine the rest as the path
            })
        } else {
            return Err(format!("Invalid line format: {}", line));
        }
    }

    /// Parse from `MEMORY_MAP_START_STR_OLD`
    fn from_str_old(line: &str) -> Result<Self, String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 5 {
            Ok(MemoryMapping {
                start_address: u64::from_str_radix(&parts[0][2..], 16)
                    .map_err(|_| "Invalid start address")?,
                end_address: u64::from_str_radix(&parts[1][2..], 16)
                    .map_err(|_| "Invalid end address")?,
                size: u64::from_str_radix(&parts[2][2..], 16).map_err(|_| "Invalid size")?,
                offset: u64::from_str_radix(&parts[3][2..], 16).map_err(|_| "Invalid offset")?,
                permissions: None,
                path: Some(parts[4..].join(" ")), // Combine the rest as the path
            })
        } else {
            Err(format!("Invalid line format: {}", line))
        }
    }
}

/// Parse from `MEMORY_MAP_START_STR_NEW`
pub fn parse_memory_mappings_new(input: &str) -> Vec<MemoryMapping> {
    input.lines().skip(1).filter_map(|line| MemoryMapping::from_str_new(line).ok()).collect()
}

/// Parse from `MEMORY_MAP_START_STR_OLD`
pub fn parse_memory_mappings_old(input: &str) -> Vec<MemoryMapping> {
    input.lines().skip(1).filter_map(|line| MemoryMapping::from_str_old(line).ok()).collect()
}

// Define Register struct to hold register data
#[derive(Debug, Clone)]
pub struct Register {
    pub number: String,
    pub value: Option<String>,
    pub v2_int128: Option<String>,
    pub v8_int32: Option<String>,
    pub v4_int64: Option<String>,
    pub v8_float: Option<String>,
    pub v16_int8: Option<String>,
    pub v4_int32: Option<String>,
    pub error: Option<String>,
}

impl Register {
    /// Value is not set to anything readable
    pub fn is_set(&self) -> bool {
        self.error.is_none() && self.value != Some("<unavailable>".to_string())
    }
}

/// Info from Exec Result "asm_insns"
#[derive(Debug, Clone)]
pub struct Asm {
    pub address: u64,
    pub inst: String,
    pub offset: u64,
    pub func_name: Option<String>,
}

/// Normalizes a value: trims quotes around strings like "\"0\"" -> "0"
fn normalize_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string() // Remove surrounding quotes
    } else {
        trimmed.to_string()
    }
}

pub fn parse_key_value_pairs(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut inside_quotes = false;
    let mut bracket_count = 0;

    let mut is_parsing_value = false;

    for c in input.chars() {
        match c {
            '=' if !inside_quotes && bracket_count == 0 => {
                // Start parsing the value
                is_parsing_value = true;
            }
            ',' if !inside_quotes && bracket_count == 0 => {
                // End of a key-value pair
                if !current_key.is_empty() {
                    map.insert(current_key.trim().to_string(), normalize_value(&current_value));
                }
                current_key.clear();
                current_value.clear();
                is_parsing_value = false;
            }
            '[' if !inside_quotes => {
                // Start of a bracketed value
                bracket_count += 1;
                current_value.push(c);
            }
            ']' if !inside_quotes => {
                // End of a bracketed value
                bracket_count -= 1;
                current_value.push(c);
            }
            '"' => {
                // Toggle inside_quotes flag
                inside_quotes = !inside_quotes;
                if is_parsing_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
            _ => {
                // Add character to the current key or value
                if is_parsing_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
        }
    }

    // Add the last key-value pair
    if !current_key.is_empty() {
        map.insert(current_key.trim().to_string(), normalize_value(&current_value));
    }

    map
}

pub fn join_registers(
    register_names: &Vec<String>,
    registers: &[Option<Register>],
) -> Vec<(String, Option<Register>)> {
    let mut registers_arch = vec![];
    for (register, name) in registers.iter().zip(register_names.iter()) {
        if let Some(register) = register {
            if !register.number.is_empty() {
                registers_arch.push((name.to_string(), Some(register.clone())));
                // debug!("[{i}] register({name}): {:?}", register);
            }
        }
    }
    registers_arch
}

// Function to parse register-values as an array of Registers
pub fn parse_register_values(input: &str) -> Vec<Option<Register>> {
    let mut registers = Vec::new();

    // Capture each register block and parse it
    for capture in match_inner_items(input) {
        let cap_str = &capture[0];
        let cap_str = &cap_str[1..cap_str.len() - 1].to_string();
        debug!("CAPTURE: {}", cap_str);
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

        let key_values = parse_key_value_pairs(cap_str);
        let mut fail = false;
        for (key, val) in key_values {
            if val.starts_with("\"{") {
                // skipping, for now
                fail = true;
                break;
            }
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
        if fail {
            registers.push(None)
        } else {
            registers.push(Some(register));
        }
    }

    registers
}

// Function to parse register-values as an array of Registers
pub fn parse_register_names_values(input: &str) -> Vec<String> {
    let registers: Vec<String> = input
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|s| s.trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    registers
}

// Function to parse register-values as an array of Registers
pub fn parse_asm_insns_values(input: &str) -> Vec<Asm> {
    let mut asms = Vec::new();

    // Capture each register block and parse it
    for capture in match_inner_items(input) {
        let cap_str = &capture[0];
        let cap_str = &cap_str[1..cap_str.len() - 1].to_string();
        let mut asm = Asm { address: 0, inst: String::new(), offset: 0, func_name: None };

        let key_values = parse_key_value_pairs(cap_str);
        for (key, val) in key_values {
            match key.as_str() {
                "address" => {
                    asm.address = {
                        let val = val.strip_prefix("0x").unwrap();
                        u64::from_str_radix(val, 16).unwrap()
                    }
                }
                "inst" => asm.inst = val,
                "offset" => asm.offset = u64::from_str_radix(&val, 10).unwrap(),
                "func-name" => asm.func_name = Some(val),
                _ => {}
            }
        }
        asms.push(asm);
    }

    asms
}

// MIResponse enum to represent different types of GDB responses
#[derive(Debug)]
pub enum MIResponse {
    ExecResult(String, HashMap<String, String>),
    AsyncRecord(String, HashMap<String, String>),
    Notify(String, HashMap<String, String>),
    StreamOutput(String, String),
    Unknown(String),
}

pub fn parse_mi_response(line: &str) -> MIResponse {
    // debug!("line: {}", line);
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

fn parse_exec_result(input: &str) -> MIResponse {
    if let Some((prefix, rest)) = input.split_once(',') {
        let data = parse_key_value_pairs(rest);
        MIResponse::ExecResult(prefix.to_string(), data)
    } else {
        MIResponse::ExecResult(input.to_string(), HashMap::new())
    }
}

fn parse_async_record(input: &str) -> MIResponse {
    if let Some((prefix, rest)) = input.split_once(',') {
        let data = parse_key_value_pairs(rest);
        MIResponse::AsyncRecord(prefix.to_string(), data)
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
    let unescaped_content = unescape_gdb_output(content.trim_matches('"'));
    MIResponse::StreamOutput(kind.to_string(), unescaped_content.to_string())
}

fn unescape_gdb_output(input: &str) -> Cow<str> {
    input.replace("\\n", "\n").replace("\\t", "\t").into()
}

pub fn read_pc_value() -> String {
    "-data-evaluate-expression $pc".to_string()
}

pub fn data_read_sp_bytes(hex_offset: u64, len: u64) -> String {
    format!("-data-read-memory-bytes $sp+0x{hex_offset:02x} {len}")
}

pub fn data_read_memory_bytes(addr: u64, hex_offset: u64, len: u64) -> String {
    format!("-data-read-memory-bytes 0x{addr:02x}+0x{hex_offset:02x} {len}")
}

pub fn data_disassemble_pc(before: usize, amt: usize) -> String {
    format!("-data-disassemble -s $pc-{before} -e $pc+{amt} -- 0")
}

pub fn data_disassemble(start: usize, amt: usize) -> String {
    format!("-data-disassemble -s {start} -e {start}+{amt} -- 0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_result_register_values() {
        let input = r#"^done,register-values=[{number="0",value="0x0"},{number="1",value="0x1"}]"#;
        if let MIResponse::ExecResult(status, key_values) = parse_mi_response(input) {
            let register_values = &key_values["register-values"];
            let registers = parse_register_values(register_values);
            assert_eq!(registers.len(), 2);

            assert_eq!(registers[0].as_ref().unwrap().number, "0");
            assert_eq!(registers[0].as_ref().unwrap().value.as_deref(), Some("0x0"));
            assert_eq!(registers[1].as_ref().unwrap().number, "1");
            assert_eq!(registers[1].as_ref().unwrap().value.as_deref(), Some("0x1"));
        } else {
            panic!("Expected ExecResult response");
        }
    }

    #[test]
    fn test_async_record() {
        let input = r#"*stopped,reason="breakpoint-hit",disp="keep",bkptno="1""#;
        if let MIResponse::AsyncRecord(reason, key_values) = parse_mi_response(input) {
            assert_eq!(reason, "stopped");
            assert_eq!(key_values.get("reason").map(|s| s.as_str()), Some("breakpoint-hit"));
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
        let input = "*stopped,reason=\"breakpoint-hit\",disp=\"keep\",bkptno=\"1\",frame={addr=\"0x00007ffff7e04c48\",func=\"printf\",args=[],from=\"/usr/lib/libc.so.6\",arch=\"i386:x86-64\"},thread-id=\"1\",stopped-threads=\"all\",core=\"1\"";
        let response = parse_mi_response(input);

        if let MIResponse::AsyncRecord(reason, data) = response {
            assert_eq!(reason, "stopped");
            assert_eq!(data.get("reason"), Some(&"breakpoint-hit".to_string()));
            assert_eq!(data.get("disp"), Some(&"keep".to_string()));
            assert_eq!(data.get("bkptno"), Some(&"1".to_string()));
            // TODO: fix frame
        } else {
            panic!("Unexpected MIResponse type");
        }
    }

    #[test]
    fn test_parse_stopped_message() {
        let input = r#"
        *stopped,reason="breakpoint-hit",disp="keep",bkptno="1",frame={addr="0x00007ffff7e04c48",func="printf",args=[],from="/usr/lib/libc.so.6",arch="i386:x86-64"},thread-id="1",stopped-threads="all",core="2"
    "#;

        let parsed = parse_mi_response(input.trim());

        match parsed {
            MIResponse::AsyncRecord(record_type, data) => {
                // Verify the AsyncRecord type
                assert_eq!(record_type, "stopped");

                // Verify fields
                assert_eq!(data.get("reason"), Some(&"breakpoint-hit".to_string()));
                assert_eq!(data.get("disp"), Some(&"keep".to_string()));
                assert_eq!(data.get("bkptno"), Some(&"1".to_string()));
                assert_eq!(data.get("thread-id"), Some(&"1".to_string()));
                assert_eq!(data.get("stopped-threads"), Some(&"all".to_string()));
                assert_eq!(data.get("core"), Some(&"2".to_string()));
                // TODO: fix frame
            }
            _ => panic!("Failed to parse AsyncRecord"),
        }
    }
}
