use crate::mi::parse_asm_insns_values;
use crate::register::RegisterStorage;
use crate::{State, Written};

/// `MIResponse::ExecResult`, key: "`asm_insns`"
pub fn recv_exec_result_asm_insns(state: &mut State, asm: &String) {
    if state.written.is_empty() {
        return;
    }
    let last_written = state.written.pop_front().unwrap();
    // TODO: change to match
    if let Written::AsmAtPc = last_written {
        state.asm = parse_asm_insns_values(asm).clone();
    }
    if let Written::SymbolDisassembly(_name) = &last_written {
        state.symbol_asm = parse_asm_insns_values(asm).clone();
    }
    if let Written::SymbolAtAddrRegister((base_reg, _n)) = &last_written {
        for RegisterStorage { name: _, register, deref } in &mut state.registers {
            if let Some(reg) = register
                && reg.number == *base_reg
            {
                let new_asms = parse_asm_insns_values(asm);
                if !new_asms.is_empty() {
                    if let Some(func_name) = &new_asms[0].func_name {
                        deref.final_assembly = format!(
                            "{}+{} ({})",
                            func_name.to_owned(),
                            new_asms[0].offset,
                            new_asms[0].inst
                        );
                    } else {
                        deref.final_assembly = new_asms[0].inst.clone();
                    }
                }
            }
        }
    }
    if let Written::SymbolAtAddrStack(deref) = last_written {
        let key = u64::from_str_radix(&deref, 16).unwrap();
        if let Some(deref) = state.stack.get_mut(&key) {
            let new_asms = parse_asm_insns_values(asm);
            if !new_asms.is_empty() {
                // Try and show func_name, otherwise asm
                if let Some(func_name) = &new_asms[0].func_name {
                    deref.final_assembly = format!(
                        "{}+{} ({})",
                        func_name.to_owned(),
                        new_asms[0].offset,
                        new_asms[0].inst
                    );
                } else {
                    deref.final_assembly = new_asms[0].inst.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mi::Register;
    use crate::{Args, PtrSize, deref::Deref};
    use rstest::rstest;

    fn create_test_state() -> State {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        State::new(args)
    }

    #[test]
    fn test_asm_insns_empty_written_queue() {
        let mut state = create_test_state();
        let asm = r#"[{address="0x401000",inst="mov rax, rbx"}]"#.to_string();

        recv_exec_result_asm_insns(&mut state, &asm);

        assert!(state.asm.is_empty());
    }

    #[test]
    fn test_asm_insns_at_pc() {
        let mut state = create_test_state();
        state.written.push_back(Written::AsmAtPc);

        let asm = r#"[{address="0x401000",func-name="main",offset="0",inst="push rbp"},{address="0x401001",func-name="main",offset="1",inst="mov rbp,rsp"}]"#.to_string();

        recv_exec_result_asm_insns(&mut state, &asm);

        assert_eq!(state.asm.len(), 2);
        assert_eq!(state.asm[0].address, 0x401000);
        assert_eq!(state.asm[0].inst, "push rbp");
        assert!(state.written.is_empty());
    }

    #[test]
    fn test_asm_insns_symbol_disassembly() {
        let mut state = create_test_state();
        state.written.push_back(Written::SymbolDisassembly("main".to_string()));

        let asm =
            r#"[{address="0x401000",func-name="main",offset="0",inst="push rbp"}]"#.to_string();

        recv_exec_result_asm_insns(&mut state, &asm);

        assert_eq!(state.symbol_asm.len(), 1);
        assert_eq!(state.symbol_asm[0].address, 0x401000);
        assert!(state.written.is_empty());
    }

    #[rstest]
    #[case(
        "0",
        0x401000,
        r#"[{address="0x401000",func-name="printf",offset="8",inst="sub rsp,0xd0"}]"#,
        "printf+8 (sub rsp,0xd0)"
    )]
    #[case("1", 0x500000, r#"[{address="0x500000",inst="nop"}]"#, "nop")]
    fn test_asm_insns_symbol_at_addr_register(
        #[case] reg_num: &str,
        #[case] addr: u64,
        #[case] asm_input: &str,
        #[case] expected: &str,
    ) {
        let mut state = create_test_state();

        let reg = Register {
            number: reg_num.to_string(),
            value: Some(format!("0x{addr:x}")),
            v2_int128: None,
            v8_int32: None,
            v4_int64: None,
            v8_float: None,
            v16_int8: None,
            v4_int32: None,
            error: None,
        };
        let reg_storage = RegisterStorage::new("rax".to_string(), Some(reg), Deref::new());
        state.registers.push(reg_storage);

        state.written.push_back(Written::SymbolAtAddrRegister((reg_num.to_string(), addr)));

        recv_exec_result_asm_insns(&mut state, &asm_input.to_string());

        assert_eq!(state.registers[0].deref.final_assembly, expected);
    }

    #[rstest]
    #[case(
        0x7fffffffe000,
        r#"[{address="0x401200",func-name="exit",offset="0",inst="mov edi,eax"}]"#,
        "exit+0 (mov edi,eax)"
    )]
    #[case(0x7fffffffe100, r#"[{address="0x600000",inst="ret"}]"#, "ret")]
    fn test_asm_insns_symbol_at_addr_stack(
        #[case] stack_addr: u64,
        #[case] asm_input: &str,
        #[case] expected: &str,
    ) {
        let mut state = create_test_state();

        state.stack.insert(stack_addr, Deref::new());
        state.written.push_back(Written::SymbolAtAddrStack(format!("{stack_addr:x}")));

        recv_exec_result_asm_insns(&mut state, &asm_input.to_string());

        let deref = state.stack.get(&stack_addr).unwrap();
        assert_eq!(deref.final_assembly, expected);
    }

    #[test]
    fn test_asm_insns_empty_asm_list() {
        let mut state = create_test_state();

        let reg = Register {
            number: "2".to_string(),
            value: Some("0x401000".to_string()),
            v2_int128: None,
            v8_int32: None,
            v4_int64: None,
            v8_float: None,
            v16_int8: None,
            v4_int32: None,
            error: None,
        };
        let reg_storage = RegisterStorage::new("rcx".to_string(), Some(reg), Deref::new());
        state.registers.push(reg_storage);

        state.written.push_back(Written::SymbolAtAddrRegister(("2".to_string(), 0x401000)));

        let asm = r"[]".to_string();

        recv_exec_result_asm_insns(&mut state, &asm);

        assert_eq!(state.registers[0].deref.final_assembly, "");
    }
}
