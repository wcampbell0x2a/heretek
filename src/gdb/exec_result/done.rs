use std::collections::HashMap;
use std::path::PathBuf;

use crate::mi::{
    match_inner_items, parse_key_value_pairs, parse_memory_mappings_new, parse_memory_mappings_old,
    Mapping,
};
use crate::{Bt, State};

pub fn exec_result_done(
    state: &mut State,
    kv: &HashMap<String, String>,
    current_map: &mut (Option<Mapping>, String),
) {
    // at this point, current_map was written in completion from StreamOutput
    // NOTE: We might be able to reduce the amount of time this is called
    exec_result_done_memory_map(state, current_map);

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
                    bt.location = u64::from_str_radix(val, 16).unwrap()
                } else if key == "func" {
                    bt.function = Some(val);
                }
            }
            state.bt.push(bt);
        }
    } else if kv.contains_key("matches") {
        state.completions.clear();
        let matches = &kv["matches"];
        let m_str = matches.strip_prefix(r#"["#).unwrap();
        let m_str = m_str.strip_suffix(r#"]"#).unwrap();
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
                state.memory_map.as_ref().unwrap()[0].path.clone().unwrap_or("".to_owned()),
            ));
        }
    }
}
