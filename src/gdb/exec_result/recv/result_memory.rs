use std::collections::HashMap;

use deku::ctx::Endian;
use log::{debug, error};

use crate::deref::Deref;
use crate::gdb::read_memory;
use crate::mi::{INSTRUCTION_LEN, data_disassemble, data_read_memory_bytes};
use crate::register::RegisterStorage;
use crate::{PtrSize, State, Written};

/// `MIResponse::ExecResult`, key: "memory"
pub fn recv_exec_result_memory(state: &mut State, memory: &String) {
    if state.written.is_empty() {
        return;
    }
    let last_written = state.written.pop_front().unwrap();

    match last_written {
        Written::RegisterValue((base_reg, _begin)) => {
            debug!("new register val for {base_reg}");
            let thirty = state.ptr_size == PtrSize::Size32;

            let (data, _) = read_memory(memory);
            for RegisterStorage { name: _, register, deref } in state.registers.iter_mut() {
                if let Some(reg) = register {
                    if reg.number == base_reg {
                        let (val, len) = if thirty {
                            let mut val = u32::from_str_radix(&data["contents"], 16).unwrap();
                            if state.endian.unwrap() == Endian::Big {
                                val = val.to_le();
                            } else {
                                val = val.to_be();
                            }

                            (val as u64, 4)
                        } else {
                            let mut val = u64::from_str_radix(&data["contents"], 16).unwrap();
                            if state.endian.unwrap() == Endian::Big {
                                val = val.to_le();
                            } else {
                                val = val.to_be();
                            }

                            (val, 8)
                        };
                        if deref.try_push(val) {
                            // If this is a code location, go ahead and try
                            // to request the asm at that spot
                            let mut is_code = false;
                            if let Some(mm) = &state.memory_map {
                                for r in mm {
                                    let is_path = r.is_path(
                                        state.filepath.as_ref().unwrap().to_str().unwrap(),
                                    );
                                    if r.contains(val) && (is_path || r.is_exec()) {
                                        // send a search for a symbol!
                                        // TODO: 32-bit?
                                        state
                                            .next_write
                                            .push(data_disassemble(val as usize, INSTRUCTION_LEN));
                                        state.written.push_back(Written::SymbolAtAddrRegister((
                                            reg.number.clone(),
                                            val,
                                        )));
                                        is_code = true;
                                        break;
                                    }
                                }
                            }

                            // all string? Request the next
                            if val > 0xff {
                                let bytes = val.to_le_bytes();
                                if bytes.iter().all(|a| {
                                    a.is_ascii_alphabetic()
                                        || a.is_ascii_graphic()
                                        || a.is_ascii_whitespace()
                                }) {
                                    let addr =
                                        data["begin"].strip_prefix("0x").unwrap().to_string();
                                    let addr = u64::from_str_radix(&addr, 16).unwrap();
                                    state.next_write.push(data_read_memory_bytes(
                                        addr + len,
                                        0,
                                        len,
                                    ));
                                    state.written.push_back(Written::RegisterValue((
                                        reg.number.clone(),
                                        val,
                                    )));
                                    return;
                                }
                            }

                            if !is_code && val != 0 {
                                // TODO: endian
                                debug!("register deref: trying to read: {:02x}", val);
                                state.next_write.push(data_read_memory_bytes(val, 0, len));
                                state
                                    .written
                                    .push_back(Written::RegisterValue((reg.number.clone(), val)));
                            }
                        }
                        break;
                    }
                }
            }
        }
        // We got here from a recusrive stack call (not the first one)
        // we use the begin here as the base key, instead of the base
        // addr we read
        Written::Stack(Some(begin)) => {
            let (data, _) = read_memory(memory);
            debug!("stack: {:02x?}", data);

            update_stack(data, state, begin);
        }
        Written::Stack(None) => {
            let (data, begin) = read_memory(memory);
            debug!("stack: {:02x?}", data);

            update_stack(data, state, begin);
        }
        Written::Memory => {
            let (data, begin) = read_memory(memory);
            debug!("memory: ({:02x?}, {:02x?}", begin, data);
            let hex = hex::decode(&data["contents"]).unwrap();
            state.hexdump = Some((u64::from_str_radix(&begin, 16).unwrap(), hex));
        }
        _ => {
            error!("unexpected Written: {last_written:?}");
        }
    }
}
fn update_stack(data: HashMap<String, String>, state: &mut State, begin: String) {
    // TODO: this is insane and should be cached
    let (val, len) = if state.ptr_size == PtrSize::Size32 {
        let mut val = u32::from_str_radix(&data["contents"], 16).unwrap();
        if state.endian.unwrap() == Endian::Big {
            val = val.to_le();
        } else {
            val = val.to_be();
        }

        (val as u64, 4)
    } else {
        let mut val = u64::from_str_radix(&data["contents"], 16).unwrap();
        if state.endian.unwrap() == Endian::Big {
            val = val.to_le();
        } else {
            val = val.to_be();
        }

        (val, 8)
    };

    // Begin is always correct endian
    let key = u64::from_str_radix(&begin, 16).unwrap();
    let deref = state.stack.entry(key).or_insert(Deref::new());
    let inserted = deref.try_push(val);

    if inserted && val != 0 {
        // If this is a code location, go ahead and try
        // to request the asm at that spot
        if let Some(mm) = &state.memory_map {
            for r in mm {
                let is_path = r.is_path(state.filepath.as_ref().unwrap().to_str().unwrap());
                if r.contains(val) && (is_path || r.is_exec()) {
                    // send a search for a symbol!
                    debug!("stack deref: trying to read as asm: {val:02x}");
                    state.next_write.push(data_disassemble(val as usize, INSTRUCTION_LEN));
                    state.written.push_back(Written::SymbolAtAddrStack(begin.clone()));
                    return;
                }
            }
        }

        if state.config.deref_show_string {
            //all string? Request the next
            if val > 0xff {
                let bytes = val.to_le_bytes();
                if bytes.iter().all(|a| {
                    a.is_ascii_alphabetic() || a.is_ascii_graphic() || a.is_ascii_whitespace()
                }) {
                    let addr = data["begin"].strip_prefix("0x").unwrap().to_string();
                    let addr = u64::from_str_radix(&addr, 16).unwrap();
                    state.next_write.push(data_read_memory_bytes(addr + len, 0, len));
                    state.written.push_back(Written::Stack(Some(begin)));
                    return;
                }
            }
        }

        // regular value to request
        debug!("stack deref: trying to read as data: {val:02x}");
        state.next_write.push(data_read_memory_bytes(val, 0, len));
        state.written.push_back(Written::Stack(Some(begin)));
    }
}
