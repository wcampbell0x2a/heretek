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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, PtrSize};
    use deku::ctx::Endian;
    use rstest::rstest;
    use std::path::PathBuf;

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

    #[rstest]
    #[case("The target endianness is set automatically (currently little endian)", Endian::Little)]
    #[case("The target endianness is set automatically (currently big endian)", Endian::Big)]
    fn test_stream_output_endian(#[case] input: &str, #[case] expected_endian: Endian) {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output("~", input, &mut state, &mut current_map, &mut current_symbols);

        assert_eq!(state.endian, Some(expected_endian));
        assert_eq!(state.output.len(), 0);
    }

    #[rstest]
    #[case("Reading symbols from /usr/bin/test...\n", "/usr/bin/test")]
    #[case("Reading symbols from /home/user/my project/a.out...\n", "/home/user/my project/a.out")]
    fn test_stream_output_reading_symbols(#[case] input: &str, #[case] expected_path: &str) {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output("~", input, &mut state, &mut current_map, &mut current_symbols);

        assert_eq!(state.filepath, Some(PathBuf::from(expected_path)));
    }

    #[rstest]
    #[case("process 1234\n")]
    #[case("Mapped address spaces:\n")]
    #[case("warning: unable to open /proc file '/proc/1/maps'\n")]
    fn test_stream_output_skip_lines(#[case] input: &str) {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output("~", input, &mut state, &mut current_map, &mut current_symbols);

        assert_eq!(state.output.len(), 0);
    }

    #[rstest]
    #[case(
        "Start Addr         End Addr           Size               Offset             Perms objfile",
        Mapping::New
    )]
    #[case(
        "Start Addr         End Addr           Size               Offset             Perms File",
        Mapping::New
    )]
    #[case(
        "Start Addr         End Addr           Size               Offset             objfile",
        Mapping::Old
    )]
    fn test_stream_output_memory_map_format(
        #[case] header: &str,
        #[case] expected_mapping: Mapping,
    ) {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output("~", header, &mut state, &mut current_map, &mut current_symbols);

        assert_eq!(current_map.0, Some(expected_mapping));
        assert!(current_map.1.contains(header));
        assert_eq!(state.output.len(), 0);
    }

    #[test]
    fn test_stream_output_symbol_list_capture() {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        state.written.push_back(crate::Written::SymbolList);

        stream_output(
            "~",
            "0x0000000000001234  main\n",
            &mut state,
            &mut current_map,
            &mut current_symbols,
        );

        assert!(current_symbols.contains("main"));
        assert_eq!(state.output.len(), 0); // captured in current_symbols
    }

    #[test]
    fn test_stream_output_normal_output() {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output(
            "~",
            "Some normal output\n",
            &mut state,
            &mut current_map,
            &mut current_symbols,
        );

        assert_eq!(state.output.len(), 1);
        assert_eq!(state.output[0], "Some normal output");
    }

    #[test]
    fn test_stream_output_multiline() {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output(
            "~",
            "Line 1\nLine 2\nLine 3\n",
            &mut state,
            &mut current_map,
            &mut current_symbols,
        );

        assert_eq!(state.output.len(), 3);
        assert_eq!(state.output[0], "Line 1");
        assert_eq!(state.output[1], "Line 2");
        assert_eq!(state.output[2], "Line 3");
    }

    #[test]
    fn test_stream_output_console_prompt() {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output("~", "(gdb)", &mut state, &mut current_map, &mut current_symbols);

        assert_eq!(state.stream_output_prompt, "(gdb)");
    }

    #[test]
    fn test_stream_output_empty_lines_removed() {
        let mut state = create_test_state();
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output("~", "\n\n\n", &mut state, &mut current_map, &mut current_symbols);

        assert_eq!(state.output.len(), 0); // empty lines should not be added
    }

    #[test]
    fn test_stream_output_does_not_overwrite_filepath() {
        let mut state = create_test_state();
        state.filepath = Some(PathBuf::from("/original/path"));
        let mut current_map = (None, String::new());
        let mut current_symbols = String::new();

        stream_output(
            "~",
            "Reading symbols from /new/path...\n",
            &mut state,
            &mut current_map,
            &mut current_symbols,
        );

        assert_eq!(state.filepath, Some(PathBuf::from("/original/path"))); // should not change
    }
}
