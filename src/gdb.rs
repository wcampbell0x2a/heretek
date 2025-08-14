use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};

mod stream_output;
use stream_output::stream_output;

mod exec_result;
use exec_result::exec_result;

use log::{debug, trace};

use crate::mi::{MIResponse, data_read_sp_bytes, parse_key_value_pairs, parse_mi_response};
use crate::{PtrSize, State, Written};

pub fn gdb_interact(gdb_stdout: BufReader<Box<dyn Read + Send>>, state: Arc<Mutex<State>>) {
    let mut current_map = (None, String::new());

    for line in gdb_stdout.lines() {
        if let Ok(line) = line {
            trace!("{:?}", line);
            let mut state = state.lock().unwrap();
            let response = parse_mi_response(&line);
            trace!("response {:?}", response);
            match &response {
                MIResponse::AsyncRecord(reason, kv) => {
                    if reason == "stopped" {
                        async_record_stopped(&mut state, kv);
                    }
                }
                MIResponse::ExecResult(status, kv) => {
                    exec_result(&mut state, status, &mut current_map, kv);
                }
                MIResponse::StreamOutput(t, s) => {
                    stream_output(t, s, &mut state, &mut current_map);
                }
                MIResponse::Unknown(s) => {
                    state.stream_output_prompt = s.to_string();
                }
                _ => (),
            }
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
}

fn read_memory(memory: &String) -> (HashMap<String, String>, String) {
    let mem_str = memory.strip_prefix(r#"[{"#).unwrap();
    let mem_str = mem_str.strip_suffix(r#"}]"#).unwrap();
    let data = parse_key_value_pairs(mem_str);
    let begin = data["begin"].to_string();
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
    debug!("writing {}", w);
    writeln!(stdin, "{}", w).expect("Failed to send command");
}
