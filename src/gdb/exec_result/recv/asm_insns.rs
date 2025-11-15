use crate::mi::parse_asm_insns_values;
use crate::register::RegisterStorage;
use crate::{State, Written};

/// `MIResponse::ExecResult`, key: "asm_insns"
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
        for RegisterStorage { name: _, register, deref } in state.registers.iter_mut() {
            if let Some(reg) = register {
                if reg.number == *base_reg {
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
                            deref.final_assembly = new_asms[0].inst.to_owned();
                        }
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
                    deref.final_assembly = new_asms[0].inst.to_owned();
                }
            }
        }
    }
}
