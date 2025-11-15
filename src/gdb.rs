use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};

mod stream_output;
use stream_output::stream_output;

mod exec_result;
use exec_result::exec_result;

use log::{debug, trace, warn};

use crate::mi::{MIResponse, data_read_sp_bytes, parse_key_value_pairs, parse_mi_response};
use crate::{PtrSize, State, Written};

pub fn gdb_interact(gdb_stdout: BufReader<Box<dyn Read + Send>>, state: Arc<Mutex<State>>) {
    let mut current_map = (None, String::new());
    let mut current_symbols = String::new();

    for line in gdb_stdout.lines().map_while(Result::ok) {
        trace!("{line:?}");
        let mut state = state.lock().unwrap();
        let response = parse_mi_response(&line);
        trace!("response {response:?}");
        match &response {
            MIResponse::AsyncRecord(reason, kv) => {
                if reason == "stopped" {
                    async_record_stopped(&mut state, kv);
                }
            }
            MIResponse::ExecResult(status, kv) => {
                exec_result(&mut state, status, &mut current_map, &mut current_symbols, kv);
            }
            MIResponse::StreamOutput(t, s) => {
                stream_output(t, s, &mut state, &mut current_map, &mut current_symbols);
            }
            MIResponse::Unknown(s) => {
                state.stream_output_prompt = s.clone();
            }
            MIResponse::Notify(..) => (),
        }
    }
}

fn async_record_stopped(state: &mut State, kv: &HashMap<String, String>) {
    // in the case of a breakpoint, save the output
    // Either it's a breakpoint event, step, signal
    state.async_result.clear();
    state.async_result.push_str("Status: ");
    if kv.get("bkptno").is_some() {
        if let Some(val) = kv.get("bkptno") {
            state.async_result.push_str(&format!("bkptno={val}, "));
        }
    } else if kv.get("signal-name").is_some() {
        if let Some(val) = kv.get("signal-name") {
            state.async_result.push_str(&format!("signal-name={val}"));
        }
        if let Some(val) = kv.get("signal-meaning") {
            state.async_result.push_str(&format!(", signal-meaning={val}, "));
        }
    }
    if let Some(val) = kv.get("reason") {
        state.async_result.push_str(&format!("reason={val}"));
    }
    if let Some(val) = kv.get("stopped-threads") {
        state.async_result.push_str(&format!(", stopped-threads={val}"));
    }
    if let Some(val) = kv.get("thread-id") {
        state.async_result.push_str(&format!(", thread-id={val}"));
    }
    // query the size of the arch
    if state.ptr_size == PtrSize::Auto {
        // sizeof ptr in arch
        state.next_write.push("-data-evaluate-expression \"sizeof(long)\"".to_string());
        state.written.push_back(Written::SizeOfVoidStar);
    }

    // get the memory mapping. We do this first b/c most of the deref logic needs
    // these locations
    state.next_write.push(r#"-interpreter-exec console "info proc mappings""#.to_string());
    // TODO: We only need to do this once
    // Get endian
    state.next_write.push(r#"-interpreter-exec console "show endian""#.to_string());
    // TODO: We only need to do this once
    state.next_write.push("-data-list-register-names".to_string());
    // When a breakpoint is hit, query for register values
    state.next_write.push("-data-list-register-values x".to_string());
    // get a list of changed registers
    state.next_write.push("-data-list-changed-registers".to_string());
    // bt
    state.next_write.push("-stack-list-frames".to_string());

    // Extract source location directly from the stopped event
    if let (Some(fullname), Some(line)) = (kv.get("fullname"), kv.get("line")) {
        debug!("Source location from stopped event: {fullname}:{line}");

        if let Ok(line_num) = line.parse::<u32>() {
            state.current_source_file = Some(fullname.clone());
            state.current_source_line = Some(line_num);

            // Try to read the source file and store lines
            if let Ok(content) = std::fs::read_to_string(std::path::Path::new(fullname)) {
                state.source_lines =
                    content.lines().map(std::string::ToString::to_string).collect();
                debug!("Read {} lines from source file", state.source_lines.len());
            } else {
                warn!("Could not read source file: {fullname}");
                state.source_lines.clear();
            }
        }
    } else if let (Some(file), Some(line)) = (kv.get("file"), kv.get("line")) {
        // Fallback to 'file' if 'fullname' is not available
        debug!("Source location from stopped event (fallback): {file}:{line}");

        if let Ok(line_num) = line.parse::<u32>() {
            state.current_source_file = Some(file.clone());
            state.current_source_line = Some(line_num);

            // Try to read the source file and store lines
            if let Ok(content) = std::fs::read_to_string(std::path::Path::new(file)) {
                state.source_lines =
                    content.lines().map(std::string::ToString::to_string).collect();
                debug!("Read {} lines from source file", state.source_lines.len());
            } else {
                warn!("Could not read source file: {file}");
                state.source_lines.clear();
            }
        }
    } else {
        debug!("No source location information in stopped event");
        state.current_source_file = None;
        state.current_source_line = None;
        state.source_lines.clear();
    }
}

fn read_memory(memory: &String) -> (HashMap<String, String>, String) {
    let mem_str = memory.strip_prefix(r"[{").unwrap();
    let mem_str = mem_str.strip_suffix(r"}]").unwrap();
    let data = parse_key_value_pairs(mem_str);
    let begin = data["begin"].clone();
    let begin = begin.strip_prefix("0x").unwrap().to_string();
    (data, begin)
}

fn dump_sp_bytes(state: &mut State, size: u64, amt: u64) {
    let mut curr_offset = 0;
    for _ in 0..amt {
        state.next_write.push(data_read_sp_bytes(curr_offset, size));
        state.written.push_back(Written::Stack(None));
        curr_offset += size;
    }
}

/// Unlock GDB stdin and write
pub fn write_mi(gdb_stdin_arc: &Arc<Mutex<dyn Write + Send>>, w: &str) {
    let mut stdin = gdb_stdin_arc.lock().unwrap();
    debug!("writing {w}");
    writeln!(stdin, "{w}").expect("Failed to send command");
}
