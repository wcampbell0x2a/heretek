use std::sync::{Arc, Mutex};

use crate::mi::parse_register_names_values;

/// `MIResponse::ExecResult`, key: "changed-registers"
pub fn recv_exec_result_changed_registers(
    changed_registers: &String,
    register_changed_arc: &Arc<Mutex<Vec<u8>>>,
) {
    let changed_registers = parse_register_names_values(changed_registers);
    let result: Vec<u8> =
        changed_registers.iter().map(|s| s.parse::<u8>().expect("Invalid number")).collect();
    let mut reg_changed = register_changed_arc.lock().unwrap();
    *reg_changed = result;
}
