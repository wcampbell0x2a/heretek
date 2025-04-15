use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

mod stream_output;
use stream_output::stream_output;

mod exec_result;
use exec_result::exec_result;

use log::{debug, trace};

use crate::deref::Deref;
use crate::mi::{
    data_read_sp_bytes, parse_key_value_pairs, parse_mi_response, Asm, MIResponse, MemoryMapping,
};
use crate::register::RegisterStorage;
use crate::{Bt, Written};

pub fn gdb_interact(
    gdb_stdout: BufReader<Box<dyn Read + Send>>,
    next_write: Arc<Mutex<Vec<String>>>,
    written: Arc<Mutex<VecDeque<Written>>>,
    thirty_two_bit: Arc<AtomicBool>,
    endian_arc: Arc<Mutex<Option<deku::ctx::Endian>>>,
    filepath_arc: Arc<Mutex<Option<PathBuf>>>,
    register_changed_arc: Arc<Mutex<Vec<u8>>>,
    register_names_arc: Arc<Mutex<Vec<String>>>,
    registers_arc: Arc<Mutex<Vec<RegisterStorage>>>,
    current_pc_arc: Arc<Mutex<u64>>,
    stack_arc: Arc<Mutex<BTreeMap<u64, Deref>>>,
    asm_arc: Arc<Mutex<Vec<Asm>>>,
    output_arc: Arc<Mutex<Vec<String>>>,
    stream_output_prompt_arc: Arc<Mutex<String>>,
    memory_map_arc: Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    hexdump_arc: Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    async_result_arc: Arc<Mutex<String>>,
    bt: Arc<Mutex<Vec<Bt>>>,
    completions: Arc<Mutex<Vec<String>>>,
) {
    let mut current_map = (None, String::new());

    for line in gdb_stdout.lines() {
        if let Ok(line) = line {
            let response = parse_mi_response(&line);
            trace!("response {:?}", response);
            match &response {
                MIResponse::AsyncRecord(reason, v) => {
                    if reason == "stopped" {
                        async_record_stopped(&async_result_arc, v, &next_write);
                    }
                }
                MIResponse::ExecResult(status, kv) => {
                    exec_result(
                        &next_write,
                        &written,
                        &thirty_two_bit,
                        &endian_arc,
                        &filepath_arc,
                        &register_changed_arc,
                        &register_names_arc,
                        &registers_arc,
                        &current_pc_arc,
                        &stack_arc,
                        &asm_arc,
                        &memory_map_arc,
                        &hexdump_arc,
                        &async_result_arc,
                        &bt,
                        &completions,
                        &mut current_map,
                        status,
                        kv,
                    );
                }
                MIResponse::StreamOutput(t, s) => {
                    stream_output(
                        t,
                        s,
                        &endian_arc,
                        &filepath_arc,
                        &mut current_map,
                        &output_arc,
                        &stream_output_prompt_arc,
                    );
                }
                MIResponse::Unknown(s) => {
                    let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
                    *stream_lock = s.to_string();
                }
                _ => (),
            }
        }
    }
}

fn async_record_stopped(
    async_result_arc: &Arc<Mutex<String>>,
    v: &HashMap<String, String>,
    next_write: &Arc<Mutex<Vec<String>>>,
) {
    // in the case of a breakpoint, save the output
    // Either it's a breakpoint event, step, signal
    let mut async_result = async_result_arc.lock().unwrap();
    async_result.clear();
    async_result.push_str("Status: ");
    if v.get("bkptno").is_some() {
        if let Some(val) = v.get("bkptno") {
            async_result.push_str(&format!("bkptno={val}, "));
        }
    } else if v.get("signal-name").is_some() {
        if let Some(val) = v.get("signal-name") {
            async_result.push_str(&format!("signal-name={val}"));
        }
        if let Some(val) = v.get("signal-meaning") {
            async_result.push_str(&format!(", signal-meaning={val}, "));
        }
    }
    if let Some(val) = v.get("reason") {
        async_result.push_str(&format!("reason={val}"));
    }
    if let Some(val) = v.get("stopped-threads") {
        async_result.push_str(&format!(", stopped-threads={val}"));
    }
    if let Some(val) = v.get("thread-id") {
        async_result.push_str(&format!(", thread-id={val}"));
    }

    let mut next_write = next_write.lock().unwrap();
    // get the memory mapping. We do this first b/c most of the deref logic needs
    // these locations
    next_write.push(r#"-interpreter-exec console "info proc mappings""#.to_string());
    // Get endian
    next_write.push(r#"-interpreter-exec console "show endian""#.to_string());
    // TODO: we could cache this, per file opened
    next_write.push("-data-list-register-names".to_string());
    // When a breakpoint is hit, query for register values
    next_write.push("-data-list-register-values x".to_string());
    // get a list of changed registers
    next_write.push("-data-list-changed-registers".to_string());
    // bt
    next_write.push("-stack-list-frames".to_string());
}

fn read_memory(memory: &String) -> (HashMap<String, String>, String) {
    let mem_str = memory.strip_prefix(r#"[{"#).unwrap();
    let mem_str = mem_str.strip_suffix(r#"}]"#).unwrap();
    let data = parse_key_value_pairs(mem_str);
    let begin = data["begin"].to_string();
    let begin = begin.strip_prefix("0x").unwrap().to_string();
    (data, begin)
}

fn dump_sp_bytes(
    next_write: &mut Vec<String>,
    written: &mut VecDeque<Written>,
    size: u64,
    amt: u64,
) {
    let mut curr_offset = 0;
    for _ in 0..amt {
        next_write.push(data_read_sp_bytes(curr_offset, size));
        written.push_back(Written::Stack(None));
        curr_offset += size;
    }
}

/// Unlock GDB stdin and write
pub fn write_mi(gdb_stdin_arc: &Arc<Mutex<dyn Write + Send>>, w: &str) {
    let mut stdin = gdb_stdin_arc.lock().unwrap();
    debug!("writing {}", w);
    writeln!(stdin, "{}", w).expect("Failed to send command");
}
