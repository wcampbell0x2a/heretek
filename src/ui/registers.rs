use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::{add_deref_to_span, apply_val_color, ORANGE, PURPLE, RED};

use log::debug;
use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, style::Style, Frame};

use crate::register::RegisterStorage;
use crate::App;

/// Registers
pub fn draw_registers(app: &App, f: &mut Frame, register: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Registers".fg(ORANGE));

    let mut lines = vec![];
    let mut longest_register_name = 0;
    let mut longest_extra_val = 0;

    if let Ok(regs) = app.registers.lock() {
        if regs.is_empty() {
            f.render_widget(block, register);
            return;
        }

        // find longest register name
        // TODO: cache this
        let reg_changed_lock = app.register_changed.lock().unwrap();
        let filepath_lock = app.filepath.lock().unwrap();
        for RegisterStorage { name, register, deref: _ } in regs.iter() {
            if let Some(reg) = register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value {
                    if let Ok(_) = u64::from_str_radix(&reg_value[2..], 16) {
                        if longest_register_name < name.len() {
                            longest_register_name = name.len();
                        }
                    }
                }
            }
        }
        let width: usize = if app.thirty_two_bit.load(Ordering::Relaxed) { 11 } else { 19 };

        let empty = PathBuf::from("");
        let binding = filepath_lock.as_ref().unwrap_or(&empty);
        let filepath = binding.to_string_lossy();
        for (i, RegisterStorage { name, register, deref }) in regs.iter().enumerate() {
            if let Some(reg) = register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value {
                    if let Ok(val) = u64::from_str_radix(&reg_value[2..], 16) {
                        let changed = reg_changed_lock.contains(&(i as u8));
                        let mut reg_name =
                            Span::from(format!("  {name:width$}", width = longest_register_name))
                                .style(Style::new().fg(PURPLE));
                        let (is_stack, is_heap, is_text) = app.classify_val(val, &filepath);

                        let mut extra_derefs = Vec::new();
                        add_deref_to_span(
                            deref,
                            &mut extra_derefs,
                            app,
                            &filepath,
                            &mut longest_extra_val,
                            width,
                        );

                        let hex_string = format!("{}", reg.value.as_ref().unwrap());
                        let hex_width = hex_string.len();
                        let padding_width = width.saturating_sub(hex_width);
                        let mut span = Span::from(format!(
                            "➛ {}{:padding$}",
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
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, register);
}
