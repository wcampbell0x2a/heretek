use log::trace;

use crate::deref::Deref;
use crate::gdb::dump_sp_bytes;
use crate::mi::{
    INSTRUCTION_LEN, data_disassemble, data_disassemble_pc, data_read_memory_bytes, join_registers,
    parse_register_values, read_pc_value,
};
use crate::register::RegisterStorage;
use crate::ui::SAVED_STACK;
use crate::{PtrSize, State, Written};

/// `MIResponse::ExecResult`, key: "register-values"
///
/// This is the first time we see the register-values, so this is the actual
/// value for them (not any deref values)
pub fn recv_exec_results_register_values(register_values: &String, state: &mut State) {
    // parse the response and save it
    let registers_local = parse_register_values(register_values);
    for r in registers_local.iter().flatten() {
        if r.is_set()
            && let Some(val) = &r.value
        {
            if state.ptr_size == PtrSize::Size32 {
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
                                let is_path =
                                    b.is_path(state.filepath.as_ref().unwrap().to_str().unwrap());
                                if b.contains(u64::from(val_u32)) && (is_path || b.is_exec()) {
                                    state
                                        .next_write
                                        .push(data_disassemble(val_u32 as usize, INSTRUCTION_LEN));
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
                            state.next_write.push(data_read_memory_bytes(u64::from(val_u32), 0, 4));
                            state.written.push_back(Written::RegisterValue((
                                r.number.clone(),
                                u64::from(val_u32),
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
                                let is_path =
                                    b.is_path(state.filepath.as_ref().unwrap().to_str().unwrap());
                                if b.contains(val_u64) && (is_path || b.is_exec()) {
                                    state
                                        .next_write
                                        .push(data_disassemble(val_u64 as usize, INSTRUCTION_LEN));
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
    if state.ptr_size == PtrSize::Size32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, mi::MemoryMapping};
    use rstest::rstest;
    use std::path::PathBuf;

    fn create_test_state(ptr_size: PtrSize) -> State {
        let args = Args { gdb_path: None, remote: None, ptr_size, cmds: None, log_path: None };
        let mut state = State::new(args);
        state.register_names = vec!["rax".to_string(), "rbx".to_string()];
        state
    }

    fn create_memory_map(filepath: &str) -> Vec<MemoryMapping> {
        vec![
            MemoryMapping {
                start_address: 0x400000,
                end_address: 0x500000,
                size: 0x100000,
                offset: 0,
                permissions: Some("r-xp".to_string()),
                path: Some(filepath.to_string()),
            },
            MemoryMapping {
                start_address: 0x600000,
                end_address: 0x700000,
                size: 0x100000,
                offset: 0,
                permissions: Some("rw-p".to_string()),
                path: Some("[heap]".to_string()),
            },
        ]
    }

    #[rstest]
    #[case(PtrSize::Size64, "0x450000", true)]
    #[case(PtrSize::Size64, "0x650000", false)]
    #[case(PtrSize::Size32, "0x450000", true)]
    #[case(PtrSize::Size32, "0x650000", false)]
    fn test_register_values_code_vs_data(
        #[case] ptr_size: PtrSize,
        #[case] addr: &str,
        #[case] is_code: bool,
    ) {
        let mut state = create_test_state(ptr_size);
        state.filepath = Some(PathBuf::from("/usr/bin/test"));
        state.memory_map = Some(create_memory_map("/usr/bin/test"));

        let register_values = format!(r#"[{{number="0",value="{addr}"}}]"#);

        recv_exec_results_register_values(&register_values, &mut state);

        if is_code {
            assert!(state.next_write.iter().any(|w| w.contains("data-disassemble")));
            assert!(state.written.iter().any(|w| matches!(w, Written::SymbolAtAddrRegister(_))));
        } else {
            assert!(state.next_write.iter().any(|w| w.contains("data-read-memory-bytes")));
            assert!(state.written.iter().any(|w| matches!(w, Written::RegisterValue(_))));
        }
    }

    #[rstest]
    #[case(PtrSize::Size32, 4)]
    #[case(PtrSize::Size64, 8)]
    fn test_register_values_stack_size(#[case] ptr_size: PtrSize, #[case] expected_size: u8) {
        let mut state = create_test_state(ptr_size);
        let register_values = r#"[{number="0",value="0x1000"}]"#.to_string();

        recv_exec_results_register_values(&register_values, &mut state);

        let stack_writes: Vec<_> = state.next_write.iter().filter(|w| w.contains("$sp")).collect();

        assert!(!stack_writes.is_empty());
        assert!(stack_writes.iter().any(|w| w.contains(&format!(" {expected_size}"))));
    }

    #[test]
    fn test_register_values_null_pointer() {
        let mut state = create_test_state(PtrSize::Size64);
        state.filepath = Some(PathBuf::from("/usr/bin/test"));

        let register_values = r#"[{number="0",value="0x0"}]"#.to_string();

        recv_exec_results_register_values(&register_values, &mut state);

        let has_register_memory_request =
            state.written.iter().any(|w| matches!(w, Written::RegisterValue((_, 0))));
        assert!(!has_register_memory_request);
        assert!(state.next_write.iter().any(|w| w.contains("$pc")));
    }

    #[test]
    fn test_register_values_no_memory_map() {
        let mut state = create_test_state(PtrSize::Size64);
        state.filepath = Some(PathBuf::from("/usr/bin/test"));
        state.memory_map = None;

        let register_values = r#"[{number="0",value="0x450000"}]"#.to_string();

        recv_exec_results_register_values(&register_values, &mut state);

        assert!(!state.registers.is_empty());
    }

    #[rstest]
    #[case(r#"[{number="0",value="<unavailable>"}]"#)]
    #[case(r#"[{number="0",error="not available"}]"#)]
    fn test_register_values_unavailable(#[case] register_values: &str) {
        let mut state = create_test_state(PtrSize::Size64);

        recv_exec_results_register_values(&register_values.to_string(), &mut state);

        let has_register_memory_request = state
            .written
            .iter()
            .any(|w| matches!(w, Written::RegisterValue(_) | Written::SymbolAtAddrRegister(_)));
        assert!(!has_register_memory_request);
        assert!(!state.next_write.is_empty());
        assert!(!state.registers.is_empty());
    }

    #[test]
    fn test_register_values_multiple_registers() {
        let mut state = create_test_state(PtrSize::Size64);
        state.filepath = Some(PathBuf::from("/usr/bin/test"));
        state.memory_map = Some(create_memory_map("/usr/bin/test"));

        let register_values =
            r#"[{number="0",value="0x450000"},{number="1",value="0x460000"}]"#.to_string();

        recv_exec_results_register_values(&register_values, &mut state);

        assert_eq!(state.registers.len(), 2);
        assert_eq!(state.registers[0].name, "rax");
        assert_eq!(state.registers[1].name, "rbx");
    }

    #[test]
    fn test_register_values_requests_pc_and_stack() {
        let mut state = create_test_state(PtrSize::Size64);
        let register_values = r#"[{number="0",value="0x1000"}]"#.to_string();

        recv_exec_results_register_values(&register_values, &mut state);

        assert!(
            state
                .next_write
                .iter()
                .any(|w| w.contains("data-evaluate-expression") && w.contains("$pc"))
        );
        assert!(state.next_write.iter().any(|w| w.contains("$sp")));
        assert!(state.written.iter().any(|w| matches!(w, Written::AsmAtPc)));
    }
}
