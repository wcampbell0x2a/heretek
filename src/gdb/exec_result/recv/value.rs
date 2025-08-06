use crate::{PtrSize, State, Written};

/// MIResponse::ExecResult, key: "value"
pub fn recv_exec_result_value(state: &mut State, value: &String) {
    if let Some(Written::SizeOfVoidStar) = state.written.front() {
        match value.as_str() {
            "8" => {
                state.ptr_size = PtrSize::Size64;
                log::trace!("Setting to 32 bit mode");
            }
            "4" => {
                state.ptr_size = PtrSize::Size32;
                log::trace!("Setting to 64 bit mode");
            }
            _ => (),
        };
        let _ = state.written.pop_front().unwrap();
    } else {
        // program is stopped, get the current pc
        let pc: Vec<&str> = value.split_whitespace().collect();
        if let Some(pc) = pc[0].strip_prefix("0x") {
            state.current_pc = u64::from_str_radix(pc, 16).unwrap();
        }
    }
}
