use crate::{PtrSize, State, Written};

/// MIResponse::ExecResult, key: "value"
pub fn recv_exec_result_value(state: &mut State, value: &String) {
    if let Some(Written::SizeOfVoidStar) = state.written.front() {
        match value.as_str() {
            "8" => {
                state.ptr_size = PtrSize::Size64;
                log::trace!("Setting to 64 bit mode");
            }
            "4" => {
                state.ptr_size = PtrSize::Size32;
                log::trace!("Setting to 32 bit mode");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Args;
    use rstest::rstest;

    fn create_test_state() -> State {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Auto,
            cmds: None,
            log_path: None,
        };
        State::new(args)
    }

    #[rstest]
    #[case("8", PtrSize::Size64)]
    #[case("4", PtrSize::Size32)]
    fn test_value_sizeof_voidstar(#[case] size_str: &str, #[case] expected_size: PtrSize) {
        let mut state = create_test_state();
        state.written.push_back(Written::SizeOfVoidStar);

        recv_exec_result_value(&mut state, &size_str.to_string());

        assert_eq!(state.ptr_size, expected_size);
        assert!(state.written.is_empty());
    }

    #[test]
    fn test_value_sizeof_voidstar_unknown() {
        let mut state = create_test_state();
        state.written.push_back(Written::SizeOfVoidStar);
        let initial_ptr_size = state.ptr_size;

        recv_exec_result_value(&mut state, &"16".to_string());

        assert_eq!(state.ptr_size, initial_ptr_size);
        assert!(state.written.is_empty());
    }

    #[rstest]
    #[case("0x0000555555555140", 0x0000555555555140)]
    #[case("0x401000 <main>", 0x401000)]
    #[case("0xdeadbeef", 0xdeadbeef)]
    fn test_value_pc_address(#[case] input: &str, #[case] expected_pc: u64) {
        let mut state = create_test_state();

        recv_exec_result_value(&mut state, &input.to_string());

        assert_eq!(state.current_pc, expected_pc);
    }
}
