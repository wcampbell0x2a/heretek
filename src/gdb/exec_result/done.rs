use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::mi::{
    match_inner_items, parse_key_value_pairs, parse_memory_mappings_new, parse_memory_mappings_old,
    Mapping, MemoryMapping,
};
use crate::Bt;

pub fn exec_result_done(
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    bt: &Arc<Mutex<Vec<Bt>>>,
    completions: &Arc<Mutex<Vec<String>>>,
    current_map: &mut (Option<Mapping>, String),
    kv: &HashMap<String, String>,
) {
    // at this point, current_map was written in completion from StreamOutput
    // NOTE: We might be able to reduce the amount of time this is called
    exec_result_done_memory_map(current_map, memory_map_arc, filepath_arc);

    // result from -stack-list-frames
    if kv.contains_key("stack") {
        let mut bts = bt.lock().unwrap();
        bts.clear();
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
            bts.push(bt);
        }
    } else if kv.contains_key("matches") {
        let mut completions = completions.lock().unwrap();
        completions.clear();
        let matches = &kv["matches"];
        let m_str = matches.strip_prefix(r#"["#).unwrap();
        let m_str = m_str.strip_suffix(r#"]"#).unwrap();
        let data = parse_key_value_pairs(m_str);

        for (k, _) in data {
            let k: String = k.chars().filter(|&c| c != '\"').collect();
            completions.push(k);
        }
    }
}

fn exec_result_done_memory_map(
    current_map: &mut (Option<Mapping>, String),
    memory_map_arc: &Arc<Mutex<Option<Vec<MemoryMapping>>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
) {
    // Check if we were looking for a mapping
    // TODO: This should be an enum or something?
    if let Some(mapping_ver) = &current_map.0 {
        let m = match mapping_ver {
            Mapping::Old => parse_memory_mappings_old(&current_map.1),
            Mapping::New => parse_memory_mappings_new(&current_map.1),
        };
        let mut memory_map = memory_map_arc.lock().unwrap();
        *memory_map = Some(m);
        *current_map = (None, String::new());

        // If we haven't resolved a filepath yet, assume the 1st
        // filepath in the mapping is the main text file
        let mut filepath_lock = filepath_arc.lock().unwrap();
        if filepath_lock.is_none() {
            *filepath_lock = Some(PathBuf::from(
                memory_map.as_ref().unwrap()[0].path.clone().unwrap_or("".to_owned()),
            ));
        }
    }
}
