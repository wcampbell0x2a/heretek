use std::sync::{Arc, Mutex};

use crate::mi::parse_register_names_values;

/// `MIResponse::ExecResult`, key: "register-names"
pub fn recv_exec_result_register_names(
    register_names: &String,
    register_names_arc: &Arc<Mutex<Vec<String>>>,
) {
    let register_names = parse_register_names_values(register_names);
    let mut regs_names = register_names_arc.lock().unwrap();
    *regs_names = register_names;
}
