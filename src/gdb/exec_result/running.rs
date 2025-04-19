use crate::State;

pub fn exec_result_running(state: &mut State) {
    // TODO: this causes a bunch of re-drawing, but
    // I'm sure in the future we could make sure we are leaving our own
    // state or something?

    // reset the stack
    state.stack.clear();

    // reset the asm
    state.asm.clear();

    // reset the regs
    state.registers.clear();

    // reset the hexdump
    state.hexdump = None;

    // reset status
    state.async_result = "Status: running".to_string();

    // reset written
    // TODO: research this. This prevents the "hold down enter and confuse this program".
    // but may have other problems arise.
    state.written.clear();
    state.next_write.clear();
}
