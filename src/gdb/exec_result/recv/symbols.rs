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
