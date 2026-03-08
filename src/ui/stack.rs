use std::collections::HashMap;

use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, layout::Rect, style::Style};

use super::{ORANGE, PURPLE, add_deref_to_span};

use crate::{PtrSize, State};

pub fn draw_stack(state: &mut State, f: &mut Frame, stack: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Stack".fg(ORANGE));
    let mut lines = vec![];
    let mut longest_cells = 0;
    let width: usize = if state.ptr_size == PtrSize::Size32 { 11 } else { 19 };

    let stacks = state.stack.clone();

    // Build map of address -> register names
    let mut addr_to_regs: HashMap<u64, Vec<String>> = HashMap::new();
    for reg in &state.registers {
        if let Some(ref register) = reg.register
            && let Some(ref val_str) = register.value
            && let Some(hex) = val_str.strip_prefix("0x")
            && let Ok(val) = u64::from_str_radix(hex, 16)
            && stacks.contains_key(&val)
        {
            addr_to_regs.entry(val).or_default().push(reg.name.clone());
        }
    }

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
