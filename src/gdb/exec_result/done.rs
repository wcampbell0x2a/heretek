use std::collections::HashMap;
use std::path::PathBuf;

use crate::mi::{
    Mapping, match_inner_items, parse_key_value_pairs, parse_memory_mappings_new,
    parse_memory_mappings_old,
};
use crate::{Bt, State};

use super::recv::symbols::recv_exec_result_symbols;

pub fn exec_result_done(
    state: &mut State,
    kv: &HashMap<String, String>,
    current_map: &mut (Option<Mapping>, String),
    current_symbols: &mut String,
) {
    // at this point, current_map was written in completion from StreamOutput
    // NOTE: We might be able to reduce the amount of time this is called
    exec_result_done_memory_map(state, current_map);
    exec_result_done_symbols(state, current_symbols);

    // result from -stack-list-frames
    // ^done,stack=[frame={level="0",addr="0x0000555555804a50",func="main",arch="i386:x86-64"},frame={level="1",addr="0x00007ffff7ca1488",func="??",from="/usr/lib/libc.so.6",arch="i386:x86-64"},frame={level="2",addr="0x00007ffff7ca154c",func="__libc_start_main",from="/usr/lib/libc.so.6",arch="i386:x86-64"},frame={level="3",addr="0x00005555557bdcc5",func="_start",arch="i386:x86-64"}]
    if kv.contains_key("stack") {
        state.bt.clear();
        for capture in match_inner_items(kv.get("stack").unwrap()) {
            let cap_str = &capture[0];
            let cap_str = &cap_str[1..cap_str.len() - 1].to_string();
            let key_values = parse_key_value_pairs(cap_str);
            let mut bt = Bt::default();
            for (key, val) in key_values {
                if key == "addr" {
                    let val = val.strip_prefix("0x").unwrap();
                    bt.location = u64::from_str_radix(val, 16).unwrap();
                } else if key == "func" {
                    bt.function = Some(val);
                }
            }
            state.bt.push(bt);
        }
    } else if kv.contains_key("matches") {
        state.completions.clear();
        let matches = &kv["matches"];
        let m_str = matches.strip_prefix(r"[").unwrap();
        let m_str = m_str.strip_suffix(r"]").unwrap();
        let data = parse_key_value_pairs(m_str);

        for (k, _) in data {
            let k: String = k.chars().filter(|&c| c != '\"').collect();
            state.completions.push(k);
        }
    }
}

fn exec_result_done_memory_map(state: &mut State, current_map: &mut (Option<Mapping>, String)) {
    // Check if we were looking for a mapping
    // TODO: This should be an enum or something?
    if let Some(mapping_ver) = &current_map.0 {
        let m = match mapping_ver {
            Mapping::Old => parse_memory_mappings_old(&current_map.1),
            Mapping::New => parse_memory_mappings_new(&current_map.1),
        };
        state.memory_map = Some(m);
        *current_map = (None, String::new());

        // If we haven't resolved a filepath yet, assume the 1st
        // filepath in the mapping is the main text file
        if state.filepath.is_none() {
            state.filepath = Some(PathBuf::from(
                state.memory_map.as_ref().unwrap()[0].path.clone().unwrap_or_default(),
            ));
        }
    }
}

fn exec_result_done_symbols(state: &mut State, current_symbols: &mut String) {
    if !current_symbols.is_empty() {
        recv_exec_result_symbols(state, current_symbols);
        current_symbols.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, PtrSize, Written};

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
    fn test_exec_result_done_with_stack() {
        let mut state = create_test_state();
        let mut kv = HashMap::new();
        kv.insert(
            "stack".to_string(),
            r#"[frame={level="0",addr="0x0000555555804a50",func="main",arch="i386:x86-64"},frame={level="1",addr="0x00007ffff7ca1488",func="??",from="/usr/lib/libc.so.6",arch="i386:x86-64"}]"#.to_string(),
        );
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        exec_result_done(&mut state, &kv, &mut current_map, &mut current_symbols);

        assert_eq!(state.bt.len(), 2);
        assert_eq!(state.bt[0].location, 0x0000555555804a50);
        assert_eq!(state.bt[0].function, Some("main".to_string()));
        assert_eq!(state.bt[1].location, 0x00007ffff7ca1488);
        assert_eq!(state.bt[1].function, Some("??".to_string()));
    }

    #[test]
    fn test_exec_result_done_with_matches() {
        let mut state = create_test_state();
        let mut kv = HashMap::new();
        kv.insert("matches".to_string(), r#"["break","bt","backtrace"]"#.to_string());
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        exec_result_done(&mut state, &kv, &mut current_map, &mut current_symbols);

        assert_eq!(state.completions.len(), 3);
        assert!(state.completions.contains(&"break".to_string()));
        assert!(state.completions.contains(&"bt".to_string()));
        assert!(state.completions.contains(&"backtrace".to_string()));
    }

    #[test]
    fn test_exec_result_done_memory_map_old() {
        let mut state = create_test_state();
        let kv = HashMap::new();
        let mut current_map = (
            Some(Mapping::Old),
            "Start Addr   End Addr       Size     Offset objfile\n0x400000    0x401000    0x1000        0x0 /path/to/binary\n".to_string(),
        );
        let mut current_symbols = String::new();

        exec_result_done(&mut state, &kv, &mut current_map, &mut current_symbols);

        assert!(state.memory_map.is_some());
        assert_eq!(state.filepath, Some(PathBuf::from("/path/to/binary")));
        assert_eq!(current_map.0, None);
        assert_eq!(current_map.1, "");
    }

    #[test]
    fn test_exec_result_done_memory_map_new() {
        let mut state = create_test_state();
        let kv = HashMap::new();
        let mut current_map = (
            Some(Mapping::New),
            "Start Addr   End Addr       Size     Offset Perms  objfile\n0x400000    0x401000    0x1000        0x0  r-xp   /path/to/binary\n".to_string(),
        );
        let mut current_symbols = String::new();

        exec_result_done(&mut state, &kv, &mut current_map, &mut current_symbols);

        assert!(state.memory_map.is_some());
        assert_eq!(state.filepath, Some(PathBuf::from("/path/to/binary")));
        assert_eq!(current_map.0, None);
        assert_eq!(current_map.1, "");
    }

    #[test]
    fn test_exec_result_done_symbols() {
        let mut state = create_test_state();
        state.written.push_back(Written::SymbolList);
        let kv = HashMap::new();
        let mut current_map = (None, String::new());
        let mut current_symbols = "0x00401000 main\n0x00402000 foo".to_string();

        exec_result_done(&mut state, &kv, &mut current_map, &mut current_symbols);

        assert_eq!(state.symbols.len(), 2);
        assert_eq!(current_symbols, "");
    }

    #[test]
    fn test_exec_result_done_symbols_empty() {
        let mut state = create_test_state();
        let kv = HashMap::new();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        exec_result_done(&mut state, &kv, &mut current_map, &mut current_symbols);

        assert_eq!(state.symbols.len(), 0);
        assert_eq!(current_symbols, "");
    }
}
