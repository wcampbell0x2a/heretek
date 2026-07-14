use std::path::PathBuf;

use super::{PURPLE, RED, add_deref_to_span, apply_val_color, effective_mode, pane_block};

use ansi_to_tui::IntoText;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation};
use ratatui::{Frame, layout::Rect, style::Style};

use crate::register::RegisterStorage;
use crate::{Mode, PtrSize, State};

const ANSI_BYTES: &[u8] = include_bytes!("../../assets/heretek.txt");

/// Registers
pub fn draw_registers(state: &mut State, f: &mut Frame, register: Rect) {
    let active = matches!(effective_mode(state), Mode::All | Mode::OnlyRegister);
    let block = pane_block("Registers", None, "", active);

    let mut lines = vec![];
    let mut longest_register_name = 0;
    let mut longest_extra_val = 0;

    // show heretek ansi
    if state.current_pc == 0 && state.registers.is_empty() {
        let text = ANSI_BYTES.into_text().unwrap();
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, register);
        return;
    }

    // find longest register name
    // TODO: cache this
    for RegisterStorage { name, register, deref: _ } in &state.registers {
        if let Some(reg) = register {
            if !reg.is_set() {
                continue;
            }
            if let Some(reg_value) = &reg.value
                && u64::from_str_radix(&reg_value[2..], 16).is_ok()
                && longest_register_name < name.len()
            {
                longest_register_name = name.len();
            }
        }
    }
    let width: usize = if state.ptr_size == PtrSize::Size32 { 11 } else { 19 };

    let empty = PathBuf::from("");
    let binding = state.filepath.as_ref().unwrap_or(&empty).clone();
    let filepath = binding.to_string_lossy();
    let registers = state.registers.clone();
    for (i, RegisterStorage { name, register, deref }) in registers.iter().enumerate() {
        if let Some(reg) = register {
            if !reg.is_set() {
                continue;
            }
            if let Some(reg_value) = &reg.value
                && let Ok(val) = u64::from_str_radix(&reg_value[2..], 16)
            {
                let changed = state.register_changed.contains(&(i as u16));
                let mut reg_name = Span::from(format!("  {name:longest_register_name$}"))
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

                let hex_string = reg.value.as_ref().unwrap().clone();
                let hex_width = hex_string.len();
                let padding_width = width.saturating_sub(hex_width);
                let mut span =
                    Span::from(format!("→ {hex_string}{:padding$}", "", padding = padding_width));
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

    let len = lines.len();
    // account for the top border
    let visible = (register.height as usize).saturating_sub(1);
    state.registers_scroll.set_max_scroll(len.saturating_sub(visible));
    let skip = state.registers_scroll.scroll;

    // TODO: remove collect, juts skip before
    let lines: Vec<Line> = lines.into_iter().skip(skip).take(visible).collect();

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, register);

    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        register,
        &mut state.registers_scroll.state,
    );
}
