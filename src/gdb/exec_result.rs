use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use deku::ctx::Endian;
use recv::asm_insns::recv_exec_result_asm_insns;
use recv::result_memory::recv_exec_result_memory;

use crate::deref::Deref;
use crate::mi::{Asm, Mapping, MemoryMapping};
use crate::register::RegisterStorage;
use crate::{Bt, Written};

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
    next_write: &Arc<Mutex<Vec<String>>>,
    written: &Arc<Mutex<VecDeque<Written>>>,
    thirty_two_bit: &Arc<AtomicBool>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
    register_changed_arc: &Arc<Mutex<Vec<u8>>>,
    register_names_arc: &Arc<Mutex<Vec<String>>>,
    registers_arc: &Arc<Mutex<Vec<RegisterStorage>>>,
    current_pc_arc: &Arc<Mutex<u64>>,
    stack_arc: &Arc<Mutex<BTreeMap<u64, Deref>>>,
    asm_arc: &Arc<Mutex<Vec<Asm>>>,
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    hexdump_arc: &Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    async_result_arc: &Arc<Mutex<String>>,
    bt: &Arc<Mutex<Vec<Bt>>>,
    completions: &Arc<Mutex<Vec<String>>>,
    current_map: &mut (Option<Mapping>, String),
    status: &String,
    kv: &HashMap<String, String>,
) {
    // Parse the status
    if status == "running" {
        let mut next_write = next_write.lock().unwrap();
        let mut written = written.lock().unwrap();
        exec_result_running(
            stack_arc,
            asm_arc,
            registers_arc,
            hexdump_arc,
            async_result_arc,
            &mut written,
            &mut next_write,
        );
    } else if status == "done" {
        exec_result_done(filepath_arc, memory_map_arc, bt, completions, current_map, kv);
    } else if status == "error" {
        // assume this is from us, pop off an unexpected
        // if we can
        let mut written = written.lock().unwrap();
        let _removed = written.pop_front();
        // trace!("ERROR: {:02x?}", removed);
    }

    // Parse the key-value pairs
    if let Some(value) = kv.get("value") {
        recv_exec_result_value(current_pc_arc, value);
    } else if let Some(register_names) = kv.get("register-names") {
        recv_exec_result_register_names(register_names, register_names_arc);
    } else if let Some(changed_registers) = kv.get("changed-registers") {
        recv_exec_result_changed_registers(changed_registers, register_changed_arc);
    } else if let Some(register_values) = kv.get("register-values") {
        let mut next_write = next_write.lock().unwrap();
        let mut written = written.lock().unwrap();
        recv_exec_results_register_values(
            register_values,
            thirty_two_bit,
            registers_arc,
            register_names_arc,
            memory_map_arc,
            filepath_arc,
            &mut next_write,
            &mut written,
        );
    } else if let Some(memory) = kv.get("memory") {
        let mut next_write = next_write.lock().unwrap();
        let mut written = written.lock().unwrap();
        recv_exec_result_memory(
            stack_arc,
            thirty_two_bit,
            endian_arc,
            registers_arc,
            hexdump_arc,
            memory,
            &mut written,
            &mut next_write,
            memory_map_arc,
            filepath_arc,
        );
    } else if let Some(asm) = kv.get("asm_insns") {
        let mut written = written.lock().unwrap();
        recv_exec_result_asm_insns(asm, asm_arc, registers_arc, stack_arc, &mut written);
    }
}
