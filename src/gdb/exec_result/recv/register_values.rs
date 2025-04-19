use log::trace;

use crate::deref::Deref;
use crate::gdb::dump_sp_bytes;
use crate::mi::{
    data_disassemble, data_disassemble_pc, data_read_memory_bytes, join_registers,
    parse_register_values, read_pc_value, INSTRUCTION_LEN,
};
use crate::register::RegisterStorage;
use crate::ui::SAVED_STACK;
use crate::{State, Written};

/// `MIResponse::ExecResult`, key: "register-values"
///
/// This is the first time we see the register-values, so this is the actual
/// value for them (not any deref values)
pub fn recv_exec_results_register_values(register_values: &String, state: &mut State) {
    // parse the response and save it
    let registers_local = parse_register_values(register_values);
    for r in registers_local.iter().flatten() {
        if r.is_set() {
            if let Some(val) = &r.value {
                if state.thirty_two_bit {
                    // TODO: this should be able to expect
                    if let Ok(val_u32) = u32::from_str_radix(&val[2..], 16) {
                        // NOTE: This is already in the right endian
                        // avoid trying to read null :^)
                        if val_u32 != 0 {
                            // If this is a code location, go ahead and try
                            // to request the asm at that spot
                            let mut asked_for_code = false;
                            if let Some(memory_map) = state.memory_map.as_ref() {
                                for b in memory_map {
                                    let is_path = b.is_path(
                                        state.filepath.as_ref().unwrap().to_str().unwrap(),
                                    );
                                    if b.contains(u64::from(val_u32)) && (is_path || b.is_exec()) {
                                        state.next_write.push(data_disassemble(
                                            val_u32 as usize,
                                            INSTRUCTION_LEN,
                                        ));
                                        state.written.push_back(Written::SymbolAtAddrRegister((
                                            r.number.clone(),
                                            u64::from(val_u32),
                                        )));
                                        asked_for_code = true;
                                    }
                                }
                            }
                            if !asked_for_code {
                                // just a value
                                state.next_write.push(data_read_memory_bytes(val_u32 as u64, 0, 4));
                                state.written.push_back(Written::RegisterValue((
                                    r.number.clone(),
                                    val_u32 as u64,
                                )));
                            }
                        }
                    }
                } else {
                    // TODO: this should be able to expect
                    if let Ok(val_u64) = u64::from_str_radix(&val[2..], 16) {
                        // NOTE: This is already in the right endian
                        // avoid trying to read null :^)
                        if val_u64 != 0 {
                            // If this is a code location, go ahead and try
                            // to request the asm at that spot
                            let mut asked_for_code = false;
                            if let Some(memory_map) = state.memory_map.as_ref() {
                                for b in memory_map {
                                    let is_path = b.is_path(
                                        state.filepath.as_ref().unwrap().to_str().unwrap(),
                                    );
                                    if b.contains(val_u64) && (is_path || b.is_exec()) {
                                        state.next_write.push(data_disassemble(
                                            val_u64 as usize,
                                            INSTRUCTION_LEN,
                                        ));
                                        state.written.push_back(Written::SymbolAtAddrRegister((
                                            r.number.clone(),
                                            val_u64,
                                        )));
                                        asked_for_code = true;
                                    }
                                }
                            }
                            if !asked_for_code {
                                // just a value
                                state.next_write.push(data_read_memory_bytes(val_u64, 0, 8));
                                state
                                    .written
                                    .push_back(Written::RegisterValue((r.number.clone(), val_u64)));
                            }
                        }
                    }
                }
            }
        }
    }
    let registers_new = join_registers(&state.register_names, &registers_local);
    let registers_new: Vec<RegisterStorage> = registers_new
        .iter()
        .map(|(a, b)| RegisterStorage::new(a.clone(), b.clone(), Deref::new()))
        .collect();
    state.registers = registers_new.clone();

    // assuming we have a valid $pc, get the bytes
    trace!("requesting pc bytes");
    let val = read_pc_value();
    state.next_write.push(val);

    // assuming we have a valid Stack ($sp), get the bytes
    trace!("requesting stack");
    if state.thirty_two_bit {
        dump_sp_bytes(state, 4, u64::from(SAVED_STACK));
    } else {
        dump_sp_bytes(state, 8, u64::from(SAVED_STACK));
    }

    // update current asm at pc
    trace!("updating pc asm");
    let instruction_length = 8;
    state.next_write.push(data_disassemble_pc(instruction_length * 5, instruction_length * 15));
    state.written.push_back(Written::AsmAtPc);
}
