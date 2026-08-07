use std::collections::HashMap;

use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, layout::Rect, style::Style};

use super::{ORANGE, PURPLE, add_deref_to_span, effective_mode, pane_block};

use crate::register::RegisterStorage;
use crate::{Mode, PtrSize, State};

/// Build a map of stack address -> register names for any register whose value
/// points directly at an address present in the stack view
fn addr_to_regs(
    registers: &[RegisterStorage],
    stacks: &std::collections::BTreeMap<u64, crate::deref::Deref>,
) -> HashMap<u64, Vec<String>> {
    let mut map: HashMap<u64, Vec<String>> = HashMap::new();
    for reg in registers {
        if let Some(ref register) = reg.register
            && let Some(ref val_str) = register.value
            && let Some(hex) = val_str.strip_prefix("0x")
            && let Ok(val) = u64::from_str_radix(hex, 16)
            && stacks.contains_key(&val)
        {
            map.entry(val).or_default().push(reg.name.clone());
        }
    }
    map
}

pub fn draw_stack(state: &mut State, f: &mut Frame, stack: Rect) {
    let active = matches!(effective_mode(state), Mode::OnlyStack);
    let block = pane_block("Stack", None, "", active);
    let mut lines = vec![];
    let mut longest_cells = 0;
    let width: usize = if state.ptr_size == PtrSize::Size32 { 11 } else { 19 };

    let stacks = state.stack.clone();

    // Build map of address -> register names
    let addr_to_regs = addr_to_regs(&state.registers, &stacks);

    for (addr, values) in &stacks {
        let filepath = state.filepath.clone().unwrap_or_default();
        let filepath = filepath.to_string_lossy();

        let hex_string = format!("0x{addr:02x}");
        let hex_width = hex_string.len();
        let padding_width = (width - 4).saturating_sub(hex_width);
        let span = Span::from(format!("  {hex_string}{:padding$}", "", padding = padding_width))
            .style(Style::new().fg(PURPLE));
        let mut spans = vec![span];
        if let Some(reg_names) = addr_to_regs.get(addr) {
            let annotation = format!(" ({})", reg_names.join(", "));
            spans.push(Span::from(annotation).style(Style::new().fg(ORANGE)));
        }
        add_deref_to_span(values, &mut spans, state, &filepath, &mut longest_cells, width);
        let line = Line::from(spans);
        lines.push(line);
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, stack);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::deref::Deref;
    use crate::mi::Register;

    fn reg(name: &str, value: Option<&str>) -> RegisterStorage {
        let register = value.map(|v| Register {
            number: "0".to_string(),
            value: Some(v.to_string()),
            v2_int128: None,
            v8_int32: None,
            v4_int64: None,
            v8_float: None,
            v16_int8: None,
            v4_int32: None,
            error: None,
        });
        RegisterStorage::new(name.to_string(), register, Deref::new())
    }

    #[test]
    fn test_addr_to_regs_matches_stack_addresses() {
        let mut stacks: BTreeMap<u64, Deref> = BTreeMap::new();
        stacks.insert(0x7fffffffb690, Deref::new());
        stacks.insert(0x7fffffffb6a0, Deref::new());

        let registers = vec![
            reg("rsp", Some("0x7fffffffb690")), // on the stack
            reg("rax", Some("0x7fffffffb6a0")), // on the stack
            reg("rbp", Some("0x7fffffffb6a0")), // same address as rax
            reg("rip", Some("0x401000")),       // not on the stack
            reg("rbx", None),                   // unset
        ];

        let map = addr_to_regs(&registers, &stacks);

        assert_eq!(map.get(&0x7fffffffb690), Some(&vec!["rsp".to_string()]));
        assert_eq!(map.get(&0x7fffffffb6a0), Some(&vec!["rax".to_string(), "rbp".to_string()]));
        assert!(!map.contains_key(&0x401000));
    }
}
