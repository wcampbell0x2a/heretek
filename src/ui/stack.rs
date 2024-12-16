use ratatui::layout::Constraint;
use ratatui::prelude::Constraint::Fill;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Cell, Table};
use ratatui::{layout::Rect, style::Style, widgets::Row, Frame};

use super::{apply_val_color, ORANGE, PURPLE};

use crate::App;

pub fn draw_stack(app: &App, f: &mut Frame, stack: Rect) {
    // Stack
    let mut rows = vec![];
    if let Ok(stack) = app.stack.lock() {
        let mut entries: Vec<_> = stack.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (addr, values) in entries.iter() {
            // TODO: increase scope
            let filepath_lock = app.filepath.lock().unwrap();
            let binding = filepath_lock.as_ref().unwrap();
            let filepath = binding.to_string_lossy();

            let addr = Cell::from(format!("  0x{:02x}", addr)).style(Style::new().fg(PURPLE));
            let mut cells = vec![addr];
            for v in values {
                let mut cell = Cell::from(format!("➛ 0x{:02x}", v));
                let (is_stack, is_heap, is_text) = app.classify_val(*v, &filepath);
                apply_val_color(&mut cell, is_stack, is_heap, is_text);
                cells.push(cell);
            }
            let row = Row::new(cells);
            rows.push(row);
        }
    }

    let widths = [
        Constraint::Length(16),
        Fill(1),
        Fill(1),
        Fill(1),
        Fill(1),
        Fill(1),
        Fill(1),
        Fill(1),
        Fill(1),
    ];
    let table = Table::new(rows, widths)
        .block(Block::default().borders(Borders::TOP).title("Stack".fg(ORANGE)));

    f.render_widget(table, stack);
}
