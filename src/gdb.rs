use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use deku::ctx::Endian;
use log::{debug, info, trace};

use crate::mi::{
    data_disassemble, data_read_memory_bytes, data_read_sp_bytes, join_registers,
    parse_asm_insns_values, parse_key_value_pairs, parse_memory_mappings, parse_mi_response,
    parse_register_names_values, parse_register_values, read_pc_value, Asm, MIResponse,
    MemoryMapping, Register, MEMORY_MAP_START_STR_NEW, MEMORY_MAP_START_STR_OLD,
};
use crate::Written;

pub fn gdb_interact(
    gdb_stdout: BufReader<Box<dyn Read + Send>>,
    thirty_two_bit_arc: Arc<Mutex<bool>>,
    endian_arc: Arc<Mutex<Option<deku::ctx::Endian>>>,
    filepath_arc: Arc<Mutex<Option<PathBuf>>>,
    register_changed_arc: Arc<Mutex<Vec<u8>>>,
    register_names_arc: Arc<Mutex<Vec<String>>>,
    registers_arc: Arc<Mutex<Vec<(String, Option<Register>, Vec<u64>)>>>,
    current_pc_arc: Arc<Mutex<u64>>,
    stack_arc: Arc<Mutex<HashMap<u64, Vec<u64>>>>,
    asm_arc: Arc<Mutex<Vec<Asm>>>,
    gdb_stdin_arc: Arc<Mutex<dyn Write + Send>>,
    output_arc: Arc<Mutex<Vec<String>>>,
    stream_output_prompt_arc: Arc<Mutex<String>>,
    memory_map_arc: Arc<Mutex<Option<Vec<MemoryMapping>>>>,
) {
    let mut current_map = (false, String::new());
    let mut next_write = vec![String::new()];
    let mut written = VecDeque::new();

    for line in gdb_stdout.lines() {
        if let Ok(line) = line {
            let response = parse_mi_response(&line);
            // TODO: I really hate the flow of this function, the reading and writing should be split into some
            // sort of state machine instead of just writing stuff and hoping the next state makes us read the right thing...
            debug!("response {:?}", response);
            match &response {
                MIResponse::AsyncRecord(reason, v) => {
                    if reason == "stopped" {
                        // debug!("{v:?}");
                        // TODO: we could cache this, per file opened
                        if let Some(arch) = v.get("arch") {
                            // debug!("{arch}");
                        }
                        // Get endian
                        next_write.push(r#"-interpreter-exec console "show endian""#.to_string());
                        // TODO: we could cache this, per file opened
                        next_write.push("-data-list-register-names".to_string());
                        // When a breakpoint is hit, query for register values
                        next_write.push("-data-list-register-values x".to_string());
                        // get a list of changed registers
                        next_write.push("-data-list-changed-registers".to_string());
                        // get the memory mapping
                        next_write
                            .push(r#"-interpreter-exec console "info proc mappings""#.to_string());
                    }
                }
                MIResponse::ExecResult(status, kv) => {
                    if status == "running" {
                        // TODO: this causes a bunch of re-drawing, but
                        // I'm sure in the future we could make sure we are leaving our own
                        // state or something?

                        // reset the stack
                        let mut stack = stack_arc.lock().unwrap();
                        stack.clear();

                        // reset the asm
                        let mut asm = asm_arc.lock().unwrap();
                        asm.clear();

                        // reset the regs
                        let mut regs = registers_arc.lock().unwrap();
                        regs.clear();
                    }
                    if status == "done" {
                        // Check if we were looking for a mapping
                        // TODO: This should be an enum or something?
                        if current_map.0 {
                            let m = parse_memory_mappings(&current_map.1);
                            let mut memory_map = memory_map_arc.lock().unwrap();
                            *memory_map = Some(m);
                            current_map = (false, String::new());
                        }
                    }
                    if status == "error" {
                        // assume this is from us, pop off an unexpected
                        // if we can
                        let removed = written.pop_front();
                        // trace!("ERROR: {:02x?}", removed);
                    }

                    if let Some(value) = kv.get("value") {
                        recv_exec_result_value(&current_pc_arc, value);
                    } else if let Some(register_names) = kv.get("register-names") {
                        recv_exec_result_register_names(register_names, &register_names_arc);
                    } else if let Some(changed_registers) = kv.get("changed-registers") {
                        recv_exec_result_changed_values(changed_registers, &register_changed_arc);
                    } else if let Some(register_values) = kv.get("register-values") {
                        recv_exec_results_register_value(
                            register_values,
                            &thirty_two_bit_arc,
                            &endian_arc,
                            &registers_arc,
                            &register_names_arc,
                            &mut next_write,
                            &mut written,
                        );
                    } else if let Some(memory) = kv.get("memory") {
                        recv_exec_result_memory(
                            &stack_arc,
                            &thirty_two_bit_arc,
                            &endian_arc,
                            &registers_arc,
                            memory,
                            &mut written,
                            &mut next_write,
                        );
                    } else if let Some(asm) = kv.get("asm_insns") {
                        recv_exec_result_asm_insns(asm, &asm_arc);
                    }
                }
                MIResponse::StreamOutput(t, s) => {
                    if s.starts_with("The target endianness") {
                        let mut endian = endian_arc.lock().unwrap();
                        *endian = if s.contains("little") {
                            Some(deku::ctx::Endian::Little)
                        } else {
                            Some(deku::ctx::Endian::Big)
                        };
                        debug!("endian: {endian:?}");

                        // don't include this is output
                        continue;
                    }

                    // When using attach, assume the first symbols found are the text field
                    // StreamOutput("~", "Reading symbols from /home/wcampbell/a.out...\n")
                    let mut filepath_lock = filepath_arc.lock().unwrap();
                    if filepath_lock.is_none() {
                        let symbols = "Reading symbols from ";
                        if s.starts_with(symbols) {
                            let filepath = &s[symbols.len()..];
                            let filepath = filepath.trim_end();
                            if let Some(filepath) = filepath.strip_suffix("...") {
                                info!("new filepath: {filepath}");
                                *filepath_lock = Some(PathBuf::from(filepath));
                            }
                        }
                    }

                    // when we find the start of a memory map, we sent this
                    // and it's quite noisy to the regular output so don't
                    // include
                    // TODO: We should only be checking for these when we expect them
                    if s.starts_with("process") || s.starts_with("Mapped address spaces:") {
                        // HACK: completely skip the following, as they are a side
                        // effect of not having a GDB MI way of getting a memory map
                        continue;
                    }
                    let split: Vec<&str> = s.split_whitespace().collect();
                    if split == MEMORY_MAP_START_STR_NEW || split == MEMORY_MAP_START_STR_OLD {
                        current_map.0 = true;
                    }
                    if current_map.0 {
                        current_map.1.push_str(&s);
                        continue;
                    }

                    let split: Vec<String> =
                        s.split('\n').map(String::from).map(|a| a.trim_end().to_string()).collect();
                    for s in split {
                        if !s.is_empty() {
                            // debug!("{s}");
                            output_arc.lock().unwrap().push(s);
                        }
                    }

                    // console-stream-output
                    if t == "~" {
                        if !s.contains('\n') {
                            let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
                            *stream_lock = s.to_string();
                        }
                    }
                }
                MIResponse::Unknown(s) => {
                    let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
                    *stream_lock = s.to_string();
                }
                _ => (),
            }
            if !next_write.is_empty() {
                for w in &next_write {
                    write_mi(&gdb_stdin_arc, w);
                }
                next_write.clear();
            }
        }
    }
}

fn recv_exec_result_asm_insns(asm: &String, asm_arc: &Arc<Mutex<Vec<Asm>>>) {
    let new_asms = parse_asm_insns_values(asm);
    let mut asm = asm_arc.lock().unwrap();
    *asm = new_asms.clone();
}

fn recv_exec_result_memory(
    stack_arc: &Arc<Mutex<HashMap<u64, Vec<u64>>>>,
    thirty_two_bit_arc: &Arc<Mutex<bool>>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    registers_arc: &Arc<Mutex<Vec<(String, Option<Register>, Vec<u64>)>>>,
    memory: &String,
    written: &mut VecDeque<Written>,
    next_write: &mut Vec<String>,
) {
    if written.is_empty() {
        return;
    }
    let last_written = written.pop_front().unwrap();

    match last_written {
        Written::RegisterValue((base_reg, n)) => {
            let thirty = thirty_two_bit_arc.lock().unwrap();
            let mut regs = registers_arc.lock().unwrap();

            let (data, _) = read_memory(memory);
            for (_, b, extra) in regs.iter_mut() {
                if let Some(b) = b {
                    if b.number == base_reg {
                        let (val, len) = if *thirty {
                            let mut val = u32::from_str_radix(&data["contents"], 16).unwrap();
                            debug!("val: {:02x?}", val);
                            let endian = endian_arc.lock().unwrap();
                            if endian.unwrap() == Endian::Big {
                                val = val.to_le();
                            } else {
                                val = val.to_be();
                            }

                            (val as u64, 4)
                        } else {
                            let mut val = u64::from_str_radix(&data["contents"], 16).unwrap();
                            debug!("val: {:02x?}", val);
                            let endian = endian_arc.lock().unwrap();
                            if endian.unwrap() == Endian::Big {
                                val = val.to_le();
                            } else {
                                val = val.to_be();
                            }

                            (val, 8)
                        };
                        if extra.iter().last() == Some(&(val)) {
                            trace!("loop detected!");
                            return;
                        }
                        extra.push(val as u64);
                        debug!("extra val: {:02x?}", val);

                        if val != 0 {
                            // TODO: endian
                            debug!("1: trying to read: {:02x}", val);
                            let num = format!("0x{:02x}", val);
                            next_write.push(data_read_memory_bytes(&num, 0, len));
                            written.push_back(Written::RegisterValue((b.number.clone(), val)));
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
                thirty_two_bit_arc,
                endian_arc,
                begin,
                &mut stack,
                next_write,
                written,
            );
        }
        Written::Stack(None) => {
            let mut stack = stack_arc.lock().unwrap();
            let (data, begin) = read_memory(memory);
            debug!("stack: {:02x?}", data);

            update_stack(
                data,
                thirty_two_bit_arc,
                endian_arc,
                begin,
                &mut stack,
                next_write,
                written,
            );
        }
    }
}

fn update_stack(
    data: HashMap<String, String>,
    thirty_two_bit_arc: &Arc<Mutex<bool>>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    begin: String,
    stack: &mut std::sync::MutexGuard<HashMap<u64, Vec<u64>>>,
    next_write: &mut Vec<String>,
    written: &mut VecDeque<Written>,
) {
    // TODO: this is insane and should be cached
    let thirty = thirty_two_bit_arc.lock().unwrap();
    let (val, len) = if *thirty {
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
    if let Some(row) = stack.get(&key) {
        if row.iter().last() == Some(&val) {
            trace!("loop detected!");
            return;
        }
    }
    stack.entry(key).and_modify(|v| v.push(val)).or_insert(vec![val]);

    debug!("stack: {:02x?}", stack);

    if val != 0 {
        // TODO: endian?
        debug!("2: trying to read: {}", data["contents"]);
        let num = format!("0x{:02x}", val);
        next_write.push(data_read_memory_bytes(&num, 0, len));
        written.push_back(Written::Stack(Some(begin)));
    }
}

fn read_memory(memory: &String) -> (HashMap<String, String>, String) {
    let mem_str = memory.strip_prefix(r#"[{"#).unwrap();
    let mem_str = mem_str.strip_suffix(r#"}]"#).unwrap();
    let data = parse_key_value_pairs(mem_str);
    let begin = data["begin"].to_string();
    let begin = begin.strip_prefix("0x").unwrap().to_string();
    (data, begin)
}

fn recv_exec_results_register_value(
    register_values: &String,
    thirty_two_bit_arc: &Arc<Mutex<bool>>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    registers_arc: &Arc<Mutex<Vec<(String, Option<Register>, Vec<u64>)>>>,
    register_names_arc: &Arc<Mutex<Vec<String>>>,
    next_write: &mut Vec<String>,
    written: &mut VecDeque<Written>,
) {
    let thirty = thirty_two_bit_arc.lock().unwrap();
    // parse the response and save it
    let registers = parse_register_values(register_values);
    let mut regs = registers_arc.lock().unwrap();
    let regs_names = register_names_arc.lock().unwrap();
    for r in &registers {
        if let Some(r) = r {
            if r.is_set() {
                if let Some(val) = &r.value {
                    if *thirty {
                        // TODO: this should be able to expect
                        if let Ok(mut val_u32) = u32::from_str_radix(&val[2..], 16) {
                            // NOTE: This is already in the right endian
                            // avoid trying to read null :^)
                            if val_u32 != 0 {
                                // TODO: we shouldn't do this for known CODE locations
                                next_write.push(data_read_memory_bytes(
                                    &format!("0x{:02x?}", val_u32),
                                    0,
                                    4,
                                ));
                                written.push_back(Written::RegisterValue((
                                    r.number.clone(),
                                    val_u32 as u64,
                                )));
                            }
                        }
                    } else {
                        // TODO: this should be able to expect
                        if let Ok(mut val_u64) = u64::from_str_radix(&val[2..], 16) {
                            // NOTE: This is already in the right endian
                            // avoid trying to read null :^)
                            if val_u64 != 0 {
                                // TODO: we shouldn't do this for known CODE locations
                                next_write.push(data_read_memory_bytes(
                                    &format!("0x{:02x?}", val_u64),
                                    0,
                                    8,
                                ));
                                written
                                    .push_back(Written::RegisterValue((r.number.clone(), val_u64)));
                            }
                        }
                    }
                }
            }
        }
    }
    let registers = join_registers(&regs_names, &registers);
    let registers: Vec<(String, Option<Register>, Vec<u64>)> =
        registers.iter().map(|(a, b)| (a.clone(), b.clone(), vec![])).collect();
    *regs = registers.clone();

    // assuming we have a valid $pc, get the bytes
    let val = read_pc_value();
    next_write.push(val);

    if *thirty {
        // assuming we have a valid $sp, get the bytes
        next_write.push(data_read_sp_bytes(0, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(4, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(8, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(12, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(16, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(20, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(24, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(28, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(32, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(36, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(40, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(44, 4));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(48, 4));
        written.push_back(Written::Stack(None));
    } else {
        // assuming we have a valid $sp, get the bytes
        next_write.push(data_read_sp_bytes(0, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(8, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(16, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(24, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(32, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(40, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(48, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(56, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(62, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(70, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(78, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(86, 8));
        written.push_back(Written::Stack(None));
        next_write.push(data_read_sp_bytes(94, 8));
        written.push_back(Written::Stack(None));
    }

    // update current asm at pc
    let instruction_length = 8;
    next_write.push(data_disassemble(instruction_length * 5, instruction_length * 15));
}

fn recv_exec_result_changed_values(
    changed_registers: &String,
    register_changed_arc: &Arc<Mutex<Vec<u8>>>,
) {
    let changed_registers = parse_register_names_values(changed_registers);
    // debug!("cr: {:?}", changed_registers);
    let result: Vec<u8> =
        changed_registers.iter().map(|s| s.parse::<u8>().expect("Invalid number")).collect();
    let mut reg_changed = register_changed_arc.lock().unwrap();
    *reg_changed = result;
}

fn recv_exec_result_register_names(
    register_names: &String,
    register_names_arc: &Arc<Mutex<Vec<String>>>,
) {
    let register_names = parse_register_names_values(register_names);
    let mut regs_names = register_names_arc.lock().unwrap();
    *regs_names = register_names;
}

fn recv_exec_result_value(current_pc_arc: &Arc<Mutex<u64>>, value: &String) {
    // This works b/c we only use this for PC, but will most likely
    // be wrong sometime
    let mut cur_pc_lock = current_pc_arc.lock().unwrap();
    let pc: Vec<&str> = value.split_whitespace().collect();
    let pc = pc[0].strip_prefix("0x").unwrap();
    *cur_pc_lock = u64::from_str_radix(pc, 16).unwrap();
}

pub fn write_mi(gdb_stdin_arc: &Arc<Mutex<dyn Write + Send>>, w: &str) {
    let mut stdin = gdb_stdin_arc.lock().unwrap();
    debug!("writing {}", w);
    writeln!(stdin, "{}", w).expect("Failed to send command");
}