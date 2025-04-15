use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::deref::Deref;
use crate::mi::Asm;
use crate::register::RegisterStorage;
use crate::Written;

pub fn exec_result_running(
    stack_arc: &Arc<Mutex<BTreeMap<u64, Deref>>>,
    asm_arc: &Arc<Mutex<Vec<Asm>>>,
    registers_arc: &Arc<Mutex<Vec<RegisterStorage>>>,
    hexdump_arc: &Arc<Mutex<Option<(u64, Vec<u8>)>>>,
    async_result_arc: &Arc<Mutex<String>>,
    written: &mut VecDeque<Written>,
    next_write: &mut Vec<String>,
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
    *async_result = "Status: running".to_string();

    // reset written
    // TODO: research this. This prevents the "hold down enter and confuse this program".
    // but may have other problems arise.
    written.clear();
    next_write.clear();
}
