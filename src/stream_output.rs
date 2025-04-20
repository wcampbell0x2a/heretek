/// MIResponse::StreamOutput
pub fn stream_output(
    t: &str,
    s: &str,
    endian_arc: &Arc<Mutex<Option<Endian>>>,
    filepath_arc: &Arc<Mutex<Option<PathBuf>>>,
    current_map: &mut (Option<Mapping>, String),
    output_arc: &Arc<Mutex<Vec<String>>>,
    stream_output_prompt_arc: &Arc<Mutex<String>>,
) {
    if s.starts_with("The target endianness") {
        let mut endian = endian_arc.lock().unwrap();
        *endian = if s.contains("little") {
            Some(deku::ctx::Endian::Little)
        } else {
            Some(deku::ctx::Endian::Big)
        };
        debug!("endian: {endian:?}");

        // don't include this is output
        return;
    }

    // When using attach, assume the first symbols found are the text field
    // StreamOutput("~", "Reading symbols from /home/wcampbell/a.out...\n")
    let mut filepath_lock = filepath_arc.lock().unwrap();
    if filepath_lock.is_none() {
        let symbols = "Reading symbols from ";
        if s.starts_with(symbols) {
            let filepath = &s[symbols.len()..];
            let filepath = filepath.trim_end();
            if let Some(filepath) = filepath.strip_suffix("...") {
                info!("new filepath: {filepath}");
                *filepath_lock = Some(PathBuf::from(filepath));
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

    let split: Vec<String> =
        s.split('\n').map(String::from).map(|a| a.trim_end().to_string()).collect();
    for s in split {
        if !s.is_empty() {
            // debug!("{s}");
            output_arc.lock().unwrap().push(s);
        }
    }

    // console-stream-output
    if t == "~" && !s.contains('\n') {
        let mut stream_lock = stream_output_prompt_arc.lock().unwrap();
        *stream_lock = s.to_string();
    }
}
