use std::path::PathBuf;

use super::{add_deref_to_span, apply_val_color, ORANGE, PURPLE, RED};

use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation};
use ratatui::{layout::Rect, style::Style, Frame};

use crate::register::RegisterStorage;
use crate::State;

/// Registers
pub fn draw_registers(state: &mut State, f: &mut Frame, register: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Registers".fg(ORANGE));

    let mut lines = vec![];
    let mut longest_register_name = 0;
    let mut longest_extra_val = 0;

    if state.registers.is_empty() {
        f.render_widget(block, register);
        return;
    }

    // find longest register name
    // TODO: cache this
    for RegisterStorage { name, register, deref: _ } in state.registers.iter() {
        if let Some(reg) = register {
            if !reg.is_set() {
                continue;
            }
            if let Some(reg_value) = &reg.value {
                if u64::from_str_radix(&reg_value[2..], 16).is_ok()
                    && longest_register_name < name.len()
                {
                    longest_register_name = name.len();
                }
            }
        }
    }
    let width: usize = if state.thirty_two_bit { 11 } else { 19 };

    let empty = PathBuf::from("");
    let binding = state.filepath.as_ref().unwrap_or(&empty).clone();
    let filepath = binding.to_string_lossy();
    let registers = state.registers.clone();
    for (i, RegisterStorage { name, register, deref }) in registers.iter().enumerate() {
        if let Some(reg) = register {
            if !reg.is_set() {
                continue;
            }
            if let Some(reg_value) = &reg.value {
                if let Ok(val) = u64::from_str_radix(&reg_value[2..], 16) {
                    let changed = state.register_changed.contains(&(i as u16));
                    let mut reg_name =
                        Span::from(format!("  {name:width$}", width = longest_register_name))
                            .style(Style::new().fg(PURPLE));
                    let (is_stack, is_heap, is_text) = state.classify_val(val, &filepath);

                    let mut extra_derefs = Vec::new();
                    add_deref_to_span(
                        deref,
                        &mut extra_derefs,
                        state,
                        &filepath,
                        &mut longest_extra_val,
                        width,
                    );

                    let hex_string = reg.value.as_ref().unwrap().to_string();
                    let hex_width = hex_string.len();
                    let padding_width = width.saturating_sub(hex_width);
                    let mut span = Span::from(format!(
                        "→ {}{:padding$}",
                        hex_string,
                        "",
                        padding = padding_width
                    ));
                    apply_val_color(&mut span, is_stack, is_heap, is_text);

                    // Apply color to reg name
                    if changed {
                        reg_name = reg_name.style(Style::new().fg(RED));
                    }
                    let mut line = Line::from(vec![reg_name, span]);
                    line.spans.append(&mut extra_derefs);
                    lines.push(line);
                }
            }
        }
    }

    let take = lines.len();
    let max = register.height;
    let skip = if take <= max as usize { 0 } else { state.registers_scroll.scroll };
    state.registers_scroll.state = state.registers_scroll.state.content_length(take);

    // TODO: remove collect, juts skip before
    let lines: Vec<Line> = lines.into_iter().skip(skip).take(take).collect();

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, register);

    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        register,
        &mut state.registers_scroll.state,
    );
}
