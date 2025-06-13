use std::collections::HashMap;

use recv::asm_insns::recv_exec_result_asm_insns;
use recv::result_memory::recv_exec_result_memory;

use crate::mi::Mapping;
use crate::State;

mod running;
use running::exec_result_running;

mod done;
use done::exec_result_done;

mod recv;
use recv::changed_registers::recv_exec_result_changed_registers;
use recv::register_names::recv_exec_result_register_names;
use recv::register_values::recv_exec_results_register_values;
use recv::value::recv_exec_result_value;

pub fn exec_result(
    state: &mut State,
    status: &String,
    current_map: &mut (Option<Mapping>, String),
    kv: &HashMap<String, String>,
) {
    // Parse the status
    if status == "running" {
        exec_result_running(state);
    } else if status == "done" {
        exec_result_done(state, kv, current_map);
    } else if status == "error" {
        // assume this is from us, pop off an unexpected
        // if we can
        let _removed = state.written.pop_front();
        // trace!("ERROR: {:02x?}", removed);
    }

    // Parse the key-value pairs
    if let Some(value) = kv.get("value") {
        recv_exec_result_value(state, value);
    } else if let Some(register_names) = kv.get("register-names") {
        recv_exec_result_register_names(register_names, &mut state.register_names);
    } else if let Some(changed_registers) = kv.get("changed-registers") {
        recv_exec_result_changed_registers(changed_registers, &mut state.register_changed);
    } else if let Some(register_values) = kv.get("register-values") {
        recv_exec_results_register_values(register_values, state);
    } else if let Some(memory) = kv.get("memory") {
        recv_exec_result_memory(state, memory);
    } else if let Some(asm) = kv.get("asm_insns") {
        recv_exec_result_asm_insns(state, asm);
    }
}
