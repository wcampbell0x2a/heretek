use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use deku::ctx::Endian;
use log::{debug, error};

use crate::deref::Deref;
use crate::gdb::read_memory;
use crate::mi::{data_disassemble, data_read_memory_bytes, MemoryMapping, INSTRUCTION_LEN};
use crate::register::RegisterStorage;
use crate::Written;

/// `MIResponse::ExecResult`, key: "memory"
pub fn recv_exec_result_memory(
    stack_arc: &Arc<Mutex<BTreeMap<u64, Deref>>>,
    thirty_two_bit: &Arc<AtomicBool>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    registers_arc: &Arc<Mutex<Vec<RegisterStorage>>>,
    hexdump_arc: &Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    memory: &String,
    written: &mut VecDeque<Written>,
    next_write: &mut Vec<String>,
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
) {
    if written.is_empty() {
        return;
    }
    let last_written = written.pop_front().unwrap();

    match last_written {
        Written::RegisterValue((base_reg, begin)) => {
            debug!("new register val for {base_reg}");
            let thirty = thirty_two_bit.load(Ordering::Relaxed);
            let mut regs = registers_arc.lock().unwrap();

            let (data, _) = read_memory(memory);
            for RegisterStorage { name: _, register, deref } in regs.iter_mut() {
                if let Some(reg) = register {
                    if reg.number == base_reg {
                        let (val, len) = if thirty {
                            let mut val = u32::from_str_radix(&data["contents"], 16).unwrap();
                            let endian = endian_arc.lock().unwrap();
                            if endian.unwrap() == Endian::Big {
                                val = val.to_le();
                            } else {
                                val = val.to_be();
                            }

                            (val as u64, 4)
                        } else {
                            let mut val = u64::from_str_radix(&data["contents"], 16).unwrap();
                            let endian = endian_arc.lock().unwrap();
                            if endian.unwrap() == Endian::Big {
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
                            let filepath_lock = filepath_arc.lock().unwrap();
                            let memory_map = memory_map_arc.lock().unwrap();
                            for r in memory_map.as_ref().unwrap() {
                                let is_path =
                                    r.is_path(filepath_lock.as_ref().unwrap().to_str().unwrap());
                                if r.contains(val) && (is_path || r.is_exec()) {
                                    // send a search for a symbol!
                                    // TODO: 32-bit?
                                    next_write
                                        .push(data_disassemble(val as usize, INSTRUCTION_LEN));
                                    written.push_back(Written::SymbolAtAddrRegister((
                                        reg.number.clone(),
                                        val,
                                    )));
                                    is_code = true;
                                    break;
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
                                    next_write.push(data_read_memory_bytes(addr + len, 0, len));
                                    written.push_back(Written::RegisterValue((
                                        reg.number.clone(),
                                        val,
                                    )));
                                    return;
                                }
                            }

                            if !is_code && val != 0 {
                                // TODO: endian
                                debug!("register deref: trying to read: {:02x}", val);
                                next_write.push(data_read_memory_bytes(val, 0, len));
                                written
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
            let mut stack = stack_arc.lock().unwrap();
            let (data, _) = read_memory(memory);
            debug!("stack: {:02x?}", data);

            update_stack(
                data,
                thirty_two_bit,
                endian_arc,
                begin,
                &mut stack,
                next_write,
                written,
                memory_map_arc,
                filepath_arc,
            );
        }
        Written::Stack(None) => {
            let mut stack = stack_arc.lock().unwrap();
            let (data, begin) = read_memory(memory);
            debug!("stack: {:02x?}", data);

            update_stack(
                data,
                thirty_two_bit,
                endian_arc,
                begin,
                &mut stack,
                next_write,
                written,
                memory_map_arc,
                filepath_arc,
            );
        }
        Written::Memory => {
            let (data, begin) = read_memory(memory);
            debug!("memory: ({:02x?}, {:02x?}", begin, data);
            let hex = hex::decode(&data["contents"]).unwrap();
            let mut hexdump_lock = hexdump_arc.lock().unwrap();
            *hexdump_lock = Some((u64::from_str_radix(&begin, 16).unwrap(), hex));
        }
        _ => {
            error!("unexpected Written: {last_written:?}");
        }
    }
}
fn update_stack(
    data: HashMap<String, String>,
    thirty_two_bit: &Arc<AtomicBool>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    begin: String,
    stack: &mut std::sync::MutexGuard<BTreeMap<u64, Deref>>,
    next_write: &mut Vec<String>,
    written: &mut VecDeque<Written>,
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
) {
    // TODO: this is insane and should be cached
    let thirty = thirty_two_bit.load(Ordering::Relaxed);
    let (val, len) = if thirty {
        let mut val = u32::from_str_radix(&data["contents"], 16).unwrap();
        let endian = endian_arc.lock().unwrap();
        if endian.unwrap() == Endian::Big {
            val = val.to_le();
        } else {
            val = val.to_be();
        }

        (val as u64, 4)
    } else {
        let mut val = u64::from_str_radix(&data["contents"], 16).unwrap();
        let endian = endian_arc.lock().unwrap();
        if endian.unwrap() == Endian::Big {
            val = val.to_le();
        } else {
            val = val.to_be();
        }

        (val, 8)
    };

    // Begin is always correct endian
    let key = u64::from_str_radix(&begin, 16).unwrap();
    let deref = stack.entry(key).or_insert(Deref::new());
    let inserted = deref.try_push(val);

    if inserted && val != 0 {
        // If this is a code location, go ahead and try
        // to request the asm at that spot
        let filepath_lock = filepath_arc.lock().unwrap();
        let memory_map = memory_map_arc.lock().unwrap();
        for r in memory_map.as_ref().unwrap() {
            let is_path = r.is_path(filepath_lock.as_ref().unwrap().to_str().unwrap());
            if r.contains(val) && (is_path || r.is_exec()) {
                // send a search for a symbol!
                debug!("stack deref: trying to read as asm: {val:02x}");
                next_write.push(data_disassemble(val as usize, INSTRUCTION_LEN));
                written.push_back(Written::SymbolAtAddrStack(begin.clone()));
                return;
            }
        }

        // all string? Request the next
        if val > 0xff {
            let bytes = val.to_le_bytes();
            if bytes
                .iter()
                .all(|a| a.is_ascii_alphabetic() || a.is_ascii_graphic() || a.is_ascii_whitespace())
            {
                let addr = data["begin"].strip_prefix("0x").unwrap().to_string();
                let addr = u64::from_str_radix(&addr, 16).unwrap();
                next_write.push(data_read_memory_bytes(addr + len, 0, len));
                written.push_back(Written::Stack(Some(begin)));
                return;
            }
        }

        // regular value to request
        debug!("stack deref: trying to read as data: {val:02x}");
        next_write.push(data_read_memory_bytes(val, 0, len));
        written.push_back(Written::Stack(Some(begin)));
    }
}
