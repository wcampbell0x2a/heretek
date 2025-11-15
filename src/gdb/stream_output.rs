use std::path::PathBuf;

use log::{debug, error, info};

use crate::State;
use crate::mi::{
    MEMORY_MAP_BEGIN, MEMORY_MAP_START_STR_NEW, MEMORY_MAP_START_STR_NEW_2,
    MEMORY_MAP_START_STR_OLD, Mapping,
};

/// `MIResponse::StreamOutput`
pub fn stream_output(
    t: &str,
    s: &str,
    state: &mut State,
    current_map: &mut (Option<Mapping>, String),
    current_symbols: &mut String,
) {
    if s.starts_with("The target endianness") {
        state.endian = if s.contains("little") {
            Some(deku::ctx::Endian::Little)
        } else {
            Some(deku::ctx::Endian::Big)
        };
        debug!("endian: {:?}", state.endian);

        // don't include this is output
        return;
    }

    // When using attach, assume the first symbols found are the text field
    // StreamOutput("~", "Reading symbols from /home/wcampbell/a.out...\n")
    if state.filepath.is_none() {
        let symbols = "Reading symbols from ";
        if s.starts_with(symbols) {
            let filepath = &s[symbols.len()..];
            let filepath = filepath.trim_end();
            if let Some(filepath) = filepath.strip_suffix("...") {
                info!("new filepath: {filepath}");
                state.filepath = Some(PathBuf::from(filepath));
            }
        }
    }

    // when we find the start of a memory map, we sent this
    // and it's quite noisy to the regular output so don't
    // include
    // TODO: We should only be checking for these when we expect them
    if s.starts_with("process") || s.starts_with("Mapped address spaces:") {
        // HACK: completely skip the following, as they are a side
        // effect of not having a GDB MI way of getting a memory map
        return;
    }

    if s.contains("warning: unable to open /proc file '/proc/1/maps'") {
        log::trace!("proc file could not be read, abort memory map reading");
        return;
    }

    let split: Vec<&str> = s.split_whitespace().collect();
    if split == MEMORY_MAP_START_STR_NEW {
        current_map.0 = Some(Mapping::New);
    } else if split == MEMORY_MAP_START_STR_NEW_2 {
        current_map.0 = Some(Mapping::New);
    } else if split == MEMORY_MAP_START_STR_OLD {
        current_map.0 = Some(Mapping::Old);
    } else if split.starts_with(&MEMORY_MAP_BEGIN) {
        error!("Expected memory mapping, was not expected mapping");
    }
    if current_map.0.is_some() {
        current_map.1.push_str(s);
        return;
    }

    use crate::Written;
    if let Some(Written::SymbolList) = state.written.front() {
        current_symbols.push_str(s);
        return;
    }

    let split: Vec<String> =
        s.split('\n').map(String::from).map(|a| a.trim_end().to_string()).collect();
    for s in split {
        if !s.is_empty() {
            state.output.push(s);
        }
    }

    // console-stream-output
    if t == "~" && !s.contains('\n') {
        state.stream_output_prompt = s.to_string();
    }
}
