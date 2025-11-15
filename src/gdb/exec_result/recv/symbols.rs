use crate::mi::parse_symbol_list;
use crate::{State, Written};

pub fn recv_exec_result_symbols(state: &mut State, accumulated_output: &str) {
    if state.written.is_empty() {
        return;
    }

    let last_written = state.written.front();
    if let Some(Written::SymbolList) = last_written {
        state.symbols = parse_symbol_list(accumulated_output);
        state.written.pop_front();

        state.symbols_selected = 0;
        state.symbols_scroll.reset();
        state.symbol_asm.clear();
        state.symbol_asm_scroll.reset();
        state.symbols_viewing_asm = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, PtrSize};

    #[test]
    fn test_recv_exec_result_symbols_empty_written() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);

        let output = "0x00401000 main\n0x00402000 foo";
        recv_exec_result_symbols(&mut state, output);

        assert_eq!(state.symbols.len(), 0);
    }

    #[test]
    fn test_recv_exec_result_symbols_with_symbol_list() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        state.written.push_back(Written::SymbolList);

        let output = "0x00401000 main\n0x00402000 foo";
        recv_exec_result_symbols(&mut state, output);

        assert_eq!(state.symbols.len(), 2);
        assert_eq!(state.symbols_selected, 0);
        assert_eq!(state.symbol_asm.len(), 0);
        assert_eq!(state.symbols_viewing_asm, false);
        assert_eq!(state.written.len(), 0);
    }

    #[test]
    fn test_recv_exec_result_symbols_wrong_written_type() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        state.written.push_back(Written::Memory);

        let output = "0x00401000 main\n0x00402000 foo";
        recv_exec_result_symbols(&mut state, output);

        assert_eq!(state.symbols.len(), 0);
        assert_eq!(state.written.len(), 1);
    }
}
