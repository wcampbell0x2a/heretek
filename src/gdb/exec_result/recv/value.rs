use log::debug;

/// MIResponse::ExecResult, key: "value"
pub fn recv_exec_result_value(current_pc: &mut u64, value: &String) {
    // This works b/c we only use this for PC, but will most likely
    // be wrong sometime
    debug!("value: {value}");
    let pc: Vec<&str> = value.split_whitespace().collect();
    let pc = pc[0].strip_prefix("0x").unwrap();
    *current_pc = u64::from_str_radix(pc, 16).unwrap();
}
