use std::path::PathBuf;

use super::{ORANGE, PURPLE, RED};

use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Cell, Table};
use ratatui::{layout::Rect, style::Style, widgets::Row, Frame};

use crate::App;

/// Registers
pub fn draw_registers(app: &App, f: &mut Frame, register: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Registers".fg(ORANGE));

    let mut rows = vec![];

    if let Ok(regs) = app.registers.lock() {
        if regs.is_empty() {
            f.render_widget(block, register);
            return;
        }

        let reg_changed_lock = app.register_changed.lock().unwrap();
        let filepath_lock = app.filepath.lock().unwrap();
        let empty = PathBuf::from("");
        let binding = filepath_lock.as_ref().unwrap_or(&empty);
        let filepath = binding.to_string_lossy();
        for (i, (name, register, vals)) in regs.iter().enumerate() {
            if let Some(reg) = register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value {
                    if let Ok(val) = u64::from_str_radix(&reg_value[2..], 16) {
                        let changed = reg_changed_lock.contains(&(i as u8));
                        let mut reg_name =
                            Cell::from(format!("  {name}")).style(Style::new().fg(PURPLE));
                        let (is_stack, is_heap, is_text) = app.classify_val(val, &filepath);

                        let mut extra_vals = Vec::new();
                        if !is_text && val != 0 && !vals.is_empty() {
                            for v in vals {
                                let mut cell = Cell::from(format!("➛ 0x{:02x}", v));
                                let (is_stack, is_heap, is_text) = app.classify_val(*v, &filepath);
                                super::apply_val_color(&mut cell, is_stack, is_heap, is_text);
                                extra_vals.push(cell);
                            }
                        }

                        let mut cell = Cell::from(format!("➛ {}", reg.value.clone().unwrap()));
                        super::apply_val_color(&mut cell, is_stack, is_heap, is_text);

                        // Apply color to reg name
                        if changed {
                            reg_name = reg_name.style(Style::new().fg(RED));
                        }
                        let mut row = vec![reg_name, cell];
                        row.append(&mut extra_vals);
                        rows.push(Row::new(row));
                    }
                }
            }
        }
    }

    let widths = [
        Constraint::Length(5),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
    ];
    let table = Table::new(rows, widths).block(block);
    f.render_widget(table, register);
}
