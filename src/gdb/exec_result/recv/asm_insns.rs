use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::deref::Deref;
use crate::mi::{parse_asm_insns_values, Asm};
use crate::register::RegisterStorage;
use crate::Written;

/// `MIResponse::ExecResult`, key: "asm_insns"
pub fn recv_exec_result_asm_insns(
    asm: &String,
    asm_arc: &Arc<Mutex<Vec<Asm>>>,
    registers_arc: &Arc<Mutex<Vec<RegisterStorage>>>,
    stack_arc: &Arc<Mutex<BTreeMap<u64, Deref>>>,
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
        for RegisterStorage { name: _, register, deref } in regs.iter_mut() {
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
        let mut stack = stack_arc.lock().unwrap();
        let key = u64::from_str_radix(&deref, 16).unwrap();
        if let Some(deref) = stack.get_mut(&key) {
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
