use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use deku::ctx::Endian;
use log::{debug, error, info, trace};

use crate::deref::Deref;
use crate::mi::{
    data_disassemble, data_disassemble_pc, data_read_memory_bytes, data_read_sp_bytes,
    join_registers, parse_asm_insns_values, parse_key_value_pairs, parse_memory_mappings_new,
    parse_memory_mappings_old, parse_mi_response, parse_register_names_values,
    parse_register_values, read_pc_value, Asm, MIResponse, Mapping, MemoryMapping, Register,
    INSTRUCTION_LEN, MEMORY_MAP_START_STR_NEW, MEMORY_MAP_START_STR_OLD,
};
use crate::Written;

pub fn gdb_interact(
    gdb_stdout: BufReader<Box<dyn Read + Send>>,
    next_write: Arc<Mutex<Vec<String>>>,
    written: Arc<Mutex<VecDeque<Written>>>,
    thirty_two_bit: Arc<AtomicBool>,
    endian_arc: Arc<Mutex<Option<deku::ctx::Endian>>>,
    filepath_arc: Arc<Mutex<Option<PathBuf>>>,
    register_changed_arc: Arc<Mutex<Vec<u8>>>,
    register_names_arc: Arc<Mutex<Vec<String>>>,
    registers_arc: Arc<Mutex<Vec<(String, Option<Register>, Deref)>>>,
    current_pc_arc: Arc<Mutex<u64>>,
    stack_arc: Arc<Mutex<HashMap<u64, Deref>>>,
    asm_arc: Arc<Mutex<Vec<Asm>>>,
    output_arc: Arc<Mutex<Vec<String>>>,
    stream_output_prompt_arc: Arc<Mutex<String>>,
    memory_map_arc: Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    hexdump_arc: Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    async_result_arc: Arc<Mutex<String>>,
) {
    let mut current_map = (None, String::new());

    for line in gdb_stdout.lines() {
        if let Ok(line) = line {
            let response = parse_mi_response(&line);
            trace!("response {:?}", response);
            match &response {
                MIResponse::AsyncRecord(reason, v) => {
                    if reason == "stopped" {
                        async_record_stopped(&async_result_arc, v, &next_write);
                    }
                }
                MIResponse::ExecResult(status, kv) => {
                    // Parse the status
                    if status == "running" {
                        exec_result_running(
                            &stack_arc,
                            &asm_arc,
                            &registers_arc,
                            &hexdump_arc,
                            &async_result_arc,
                        );
                    } else if status == "done" {
                        exec_result_done(&mut current_map, &memory_map_arc, &filepath_arc);
                    } else if status == "error" {
                        // assume this is from us, pop off an unexpected
                        // if we can
                        let mut written = written.lock().unwrap();
                        let _removed = written.pop_front();
                        // trace!("ERROR: {:02x?}", removed);
                    }

                    // Parse the key-value pairs
                    if let Some(value) = kv.get("value") {
                        recv_exec_result_value(&current_pc_arc, value);
                    } else if let Some(register_names) = kv.get("register-names") {
                        recv_exec_result_register_names(register_names, &register_names_arc);
                    } else if let Some(changed_registers) = kv.get("changed-registers") {
                        recv_exec_result_changed_registers(
                            changed_registers,
                            &register_changed_arc,
                        );
                    } else if let Some(register_values) = kv.get("register-values") {
                        let mut next_write = next_write.lock().unwrap();
                        let mut written = written.lock().unwrap();
                        recv_exec_results_register_values(
                            register_values,
                            &thirty_two_bit,
                            &registers_arc,
                            &register_names_arc,
                            &memory_map_arc,
                            &filepath_arc,
                            &mut next_write,
                            &mut written,
                        );
                    } else if let Some(memory) = kv.get("memory") {
                        let mut next_write = next_write.lock().unwrap();
                        let mut written = written.lock().unwrap();
                        recv_exec_result_memory(
                            &stack_arc,
                            &thirty_two_bit,
                            &endian_arc,
                            &registers_arc,
                            &hexdump_arc,
                            memory,
                            &mut written,
                            &mut next_write,
                            &memory_map_arc,
                            &filepath_arc,
                        );
                    } else if let Some(asm) = kv.get("asm_insns") {
                        let mut written = written.lock().unwrap();
                        recv_exec_result_asm_insns(
                            asm,
                            &asm_arc,
                            &registers_arc,
                            &stack_arc,
                            &mut written,
                        );
                    }
                }
                MIResponse::StreamOutput(t, s) => {
                    stream_output(
                        t,
                        s,
                        &endian_arc,
                        &filepath_arc,
                        &mut current_map,
                        &output_arc,
                        &stream_output_prompt_arc,
                    );
                }
                MIResponse::Unknown(s) => {
                    let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
                    *stream_lock = s.to_string();
                }
                _ => (),
            }
        }
    }
}

fn exec_result_done(
    current_map: &mut (Option<Mapping>, String),
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
) {
    // Check if we were looking for a mapping
    // TODO: This should be an enum or something?
    if let Some(mapping_ver) = &current_map.0 {
        let m = match mapping_ver {
            Mapping::Old => parse_memory_mappings_old(&current_map.1),
            Mapping::New => parse_memory_mappings_new(&current_map.1),
        };
        let mut memory_map = memory_map_arc.lock().unwrap();
        *memory_map = Some(m);
        *current_map = (None, String::new());

        // If we haven't resolved a filepath yet, assume the 1st
        // filepath in the mapping is the main text file
        let mut filepath_lock = filepath_arc.lock().unwrap();
        if filepath_lock.is_none() {
            *filepath_lock = Some(PathBuf::from(
                memory_map.as_ref().unwrap()[0].path.clone().unwrap_or("".to_owned()),
            ));
        }
    }
}

fn exec_result_running(
    stack_arc: &Arc<Mutex<HashMap<u64, Deref>>>,
    asm_arc: &Arc<Mutex<Vec<Asm>>>,
    registers_arc: &Arc<Mutex<Vec<(String, Option<Register>, Deref)>>>,
    hexdump_arc: &Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    async_result_arc: &Arc<Mutex<String>>,
) {
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

    // reset the hexdump
    let mut data_read = hexdump_arc.lock().unwrap();
    *data_read = None;

    // reset status
    let mut async_result = async_result_arc.lock().unwrap();
    *async_result = String::new();
}

fn async_record_stopped(
    async_result_arc: &Arc<Mutex<String>>,
    v: &HashMap<String, String>,
    next_write: &Arc<Mutex<Vec<String>>>,
) {
    // in the case of a breakpoint, save the output
    // Either it's a breakpoint event, step, signal
    let mut async_result = async_result_arc.lock().unwrap();
    async_result.push_str(&format!("Status("));
    if v.get("bkptno").is_some() {
        if let Some(val) = v.get("bkptno") {
            async_result.push_str(&format!("bkptno={val}, "));
        }
    } else if v.get("signal-name").is_some() {
        if let Some(val) = v.get("signal-name") {
            async_result.push_str(&format!("signal-name={val}"));
        }
        if let Some(val) = v.get("signal-meaning") {
            async_result.push_str(&format!(", signal-meaning={val}, "));
        }
    }
    if let Some(val) = v.get("reason") {
        async_result.push_str(&format!("reason={val}"));
    }
    if let Some(val) = v.get("stopped-threads") {
        async_result.push_str(&format!(", stopped-threads={val}"));
    }
    if let Some(val) = v.get("thread-id") {
        async_result.push_str(&format!(", thread-id={val}"));
    }
    async_result.push_str(")");

    let mut next_write = next_write.lock().unwrap();
    // get the memory mapping. We do this first b/c most of the deref logic needs
    // these locations
    next_write.push(r#"-interpreter-exec console "info proc mappings""#.to_string());
    // Get endian
    next_write.push(r#"-interpreter-exec console "show endian""#.to_string());
    // TODO: we could cache this, per file opened
    next_write.push("-data-list-register-names".to_string());
    // When a breakpoint is hit, query for register values
    next_write.push("-data-list-register-values x".to_string());
    // get a list of changed registers
    next_write.push("-data-list-changed-registers".to_string());
}

fn stream_output(
    t: &str,
    s: &str,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
    current_map: &mut (Option<Mapping>, String),
    output_arc: &Arc<Mutex<Vec<String>>>,
    stream_output_prompt_arc: &Arc<Mutex<String>>,
) {
    if s.starts_with("The target endianness") {
        let mut endian = endian_arc.lock().unwrap();
        *endian = if s.contains("little") {
            Some(deku::ctx::Endian::Little)
        } else {
            Some(deku::ctx::Endian::Big)
        };
        debug!("endian: {endian:?}");

        // don't include this is output
        return;
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
        return;
    }
    let split: Vec<&str> = s.split_whitespace().collect();
    if split == MEMORY_MAP_START_STR_NEW {
        current_map.0 = Some(Mapping::New);
    } else if split == MEMORY_MAP_START_STR_OLD {
        current_map.0 = Some(Mapping::Old);
    }
    if current_map.0.is_some() {
        current_map.1.push_str(&s);
        return;
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

/// MIResponse::ExecResult, key: "asm_insns"
fn recv_exec_result_asm_insns(
    asm: &String,
    asm_arc: &Arc<Mutex<Vec<Asm>>>,
    registers_arc: &Arc<Mutex<Vec<(String, Option<Register>, Deref)>>>,
    stack_arc: &Arc<Mutex<HashMap<u64, Deref>>>,
    written: &mut VecDeque<Written>,
) {
    if written.is_empty() {
        return;
    }
    let last_written = written.pop_front().unwrap();
    // TODO: change to match
    if let Written::AsmAtPc = last_written {
        let new_asms = parse_asm_insns_values(asm);
        let mut asm = asm_arc.lock().unwrap();
        *asm = new_asms.clone();
    }
    if let Written::SymbolAtAddrRegister((base_reg, _n)) = &last_written {
        let mut regs = registers_arc.lock().unwrap();
        for (_, b, deref) in regs.iter_mut() {
            if let Some(b) = b {
                if b.number == *base_reg {
                    let new_asms = parse_asm_insns_values(asm);
                    if new_asms.len() > 0 {
                        if let Some(func_name) = &new_asms[0].func_name {
                            deref.final_assembly = func_name.to_owned();
                        } else {
                            deref.final_assembly = new_asms[0].inst.to_owned();
                        }
                    }
                }
            }
        }
    }
    if let Written::SymbolAtAddrStack(deref) = last_written {
        let mut stack = stack_arc.lock().unwrap();
        let key = u64::from_str_radix(&deref, 16).unwrap();
        if let Some(deref) = stack.get_mut(&key) {
            let new_asms = parse_asm_insns_values(asm);
            if new_asms.len() > 0 {
                // Try and show func_name, otherwise asm
                if let Some(func_name) = &new_asms[0].func_name {
                    deref.final_assembly = func_name.to_owned();
                } else {
                    deref.final_assembly = new_asms[0].inst.to_owned();
                }
            }
        }
    }
}

/// MIResponse::ExecResult, key: "memory"
fn recv_exec_result_memory(
    stack_arc: &Arc<Mutex<HashMap<u64, Deref>>>,
    thirty_two_bit: &Arc<AtomicBool>,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    registers_arc: &Arc<Mutex<Vec<(String, Option<Register>, Deref)>>>,
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
        Written::RegisterValue((base_reg, _n)) => {
            debug!("new register val for {base_reg}");
            let thirty = thirty_two_bit.load(Ordering::Relaxed);
            let mut regs = registers_arc.lock().unwrap();

            let (data, _) = read_memory(memory);
            for (_, b, deref) in regs.iter_mut() {
                if let Some(b) = b {
                    if b.number == base_reg {
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
                        if deref.try_push(val as u64) {
                            // If this is a code location, go ahead and try
                            // to request the asm at that spot
                            let filepath_lock = filepath_arc.lock().unwrap();
                            let memory_map = memory_map_arc.lock().unwrap();
                            for r in memory_map.as_ref().unwrap() {
                                if r.contains(val)
                                    && r.is_path(filepath_lock.as_ref().unwrap().to_str().unwrap())
                                {
                                    // send a search for a symbol!
                                    // TODO: 32-bit?
                                    next_write
                                        .push(data_disassemble(val as usize, INSTRUCTION_LEN));
                                    written.push_back(Written::SymbolAtAddrRegister((
                                        b.number.clone(),
                                        val,
                                    )));
                                    break;
                                }
                            }

                            if !(val == 0) {
                                // TODO: endian
                                debug!("register deref: trying to read: {:02x}", val);
                                next_write.push(data_read_memory_bytes(val, 0, len));
                                written.push_back(Written::RegisterValue((b.number.clone(), val)));
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
    stack: &mut std::sync::MutexGuard<HashMap<u64, Deref>>,
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
            if r.contains(val) && r.is_path(filepath_lock.as_ref().unwrap().to_str().unwrap()) {
                // send a search for a symbol!
                next_write.push(data_disassemble(val as usize, INSTRUCTION_LEN));
                written.push_back(Written::SymbolAtAddrStack(begin.clone()));
                return;
            }
        }
        // TODO: endian?
        debug!("stack deref: trying to read: {}", data["contents"]);
        next_write.push(data_read_memory_bytes(val, 0, len));
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

/// MIResponse::ExecResult, key: "register-values"
fn recv_exec_results_register_values(
    register_values: &String,
    thirty_two_bit: &Arc<AtomicBool>,
    registers_arc: &Arc<Mutex<Vec<(String, Option<Register>, Deref)>>>,
    register_names_arc: &Arc<Mutex<Vec<String>>>,
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
    next_write: &mut Vec<String>,
    written: &mut VecDeque<Written>,
) {
    let thirty = thirty_two_bit.load(Ordering::Relaxed);

    // parse the response and save it
    let registers = parse_register_values(register_values);
    let mut regs = registers_arc.lock().unwrap();
    let regs_names = register_names_arc.lock().unwrap();
    for r in registers.iter().flatten() {
        if r.is_set() {
            if let Some(val) = &r.value {
                if thirty {
                    // TODO: this should be able to expect
                    if let Ok(val_u32) = u32::from_str_radix(&val[2..], 16) {
                        // NOTE: This is already in the right endian
                        // avoid trying to read null :^)
                        if val_u32 != 0 {
                            let filepath_lock = filepath_arc.lock().unwrap();
                            let memory_map = memory_map_arc.lock().unwrap();

                            // If this is a code location, go ahead and try
                            // to request the asm at that spot
                            let mut asked_for_code = false;
                            if let Some(memory_map) = memory_map.as_ref() {
                                for b in memory_map {
                                    if b.contains(u64::from(val_u32))
                                        && b.is_path(
                                            filepath_lock.as_ref().unwrap().to_str().unwrap(),
                                        )
                                    {
                                        next_write.push(data_disassemble(
                                            val_u32 as usize,
                                            INSTRUCTION_LEN,
                                        ));
                                        written.push_back(Written::SymbolAtAddrRegister((
                                            r.number.clone(),
                                            u64::from(val_u32),
                                        )));
                                        asked_for_code = true;
                                    }
                                }
                            }
                            if !asked_for_code {
                                next_write.push(data_read_memory_bytes(val_u32 as u64, 0, 4));
                                written.push_back(Written::RegisterValue((
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
                            let filepath_lock = filepath_arc.lock().unwrap();
                            let memory_map = memory_map_arc.lock().unwrap();

                            // If this is a code location, go ahead and try
                            // to request the asm at that spot
                            let mut asked_for_code = false;
                            if let Some(memory_map) = memory_map.as_ref() {
                                for b in memory_map {
                                    if b.contains(val_u64)
                                        && b.is_path(
                                            filepath_lock.as_ref().unwrap().to_str().unwrap(),
                                        )
                                    {
                                        next_write.push(data_disassemble(
                                            val_u64 as usize,
                                            INSTRUCTION_LEN,
                                        ));
                                        written.push_back(Written::SymbolAtAddrRegister((
                                            r.number.clone(),
                                            val_u64,
                                        )));
                                        asked_for_code = true;
                                    }
                                }
                            }
                            if !asked_for_code {
                                next_write.push(data_read_memory_bytes(val_u64, 0, 8));
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
    let registers: Vec<(String, Option<Register>, Deref)> =
        registers.iter().map(|(a, b)| (a.clone(), b.clone(), Deref::new())).collect();
    *regs = registers.clone();

    // assuming we have a valid $pc, get the bytes
    let val = read_pc_value();
    next_write.push(val);

    // assuming we have a valid $sp, get the bytes
    if thirty {
        dump_sp_bytes(next_write, written, 4, 14);
    } else {
        dump_sp_bytes(next_write, written, 8, 14);
    }

    // update current asm at pc
    let instruction_length = 8;
    next_write.push(data_disassemble_pc(instruction_length * 5, instruction_length * 15));
    written.push_back(Written::AsmAtPc);
}

fn dump_sp_bytes(
    next_write: &mut Vec<String>,
    written: &mut VecDeque<Written>,
    size: u64,
    amt: u64,
) {
    let mut curr_offset = 0;
    for _ in 0..amt {
        next_write.push(data_read_sp_bytes(curr_offset, size));
        written.push_back(Written::Stack(None));
        curr_offset += size;
    }
}

/// MIResponse::ExecResult, key: "changed-registers"
fn recv_exec_result_changed_registers(
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

/// MIResponse::ExecResult, key: "register-names"
fn recv_exec_result_register_names(
    register_names: &String,
    register_names_arc: &Arc<Mutex<Vec<String>>>,
) {
    let register_names = parse_register_names_values(register_names);
    let mut regs_names = register_names_arc.lock().unwrap();
    *regs_names = register_names;
}

/// MIResponse::ExecResult, key: "value"
fn recv_exec_result_value(current_pc_arc: &Arc<Mutex<u64>>, value: &String) {
    // This works b/c we only use this for PC, but will most likely
    // be wrong sometime
    let mut cur_pc_lock = current_pc_arc.lock().unwrap();
    debug!("value: {value}");
    let pc: Vec<&str> = value.split_whitespace().collect();
    let pc = pc[0].strip_prefix("0x").unwrap();
    *cur_pc_lock = u64::from_str_radix(pc, 16).unwrap();
}

/// Unlock GDB stdin and write
pub fn write_mi(gdb_stdin_arc: &Arc<Mutex<dyn Write + Send>>, w: &str) {
    let mut stdin = gdb_stdin_arc.lock().unwrap();
    debug!("writing {}", w);
    writeln!(stdin, "{}", w).expect("Failed to send command");
}
