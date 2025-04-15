use std::sync::{Arc, Mutex};

use log::debug;

/// MIResponse::ExecResult, key: "value"
pub fn recv_exec_result_value(current_pc_arc: &Arc<Mutex<u64>>, value: &String) {
    // This works b/c we only use this for PC, but will most likely
    // be wrong sometime
    let mut cur_pc_lock = current_pc_arc.lock().unwrap();
    debug!("value: {value}");
    let pc: Vec<&str> = value.split_whitespace().collect();
    let pc = pc[0].strip_prefix("0x").unwrap();
    *cur_pc_lock = u64::from_str_radix(pc, 16).unwrap();
}
