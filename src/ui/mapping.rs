use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Table};
use ratatui::{layout::Rect, style::Style, widgets::Row, Frame};

use super::{BLUE, ORANGE};

use crate::App;

pub fn draw_mapping(app: &App, f: &mut Frame, mapping_rect: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Memory Mapping".fg(ORANGE));

    let mut rows = vec![];
    rows.push(
        Row::new(["Start Address", "End Address", "Size", "Offset", "Permissions", "Path"])
            .style(Style::new().fg(BLUE)),
    );
    let memory_map = app.memory_map.lock().unwrap();
    if let Some(memory_map) = memory_map.as_ref() {
        for m in memory_map {
            let row = Row::new([
                format!("0x{:08x}", m.start_address),
                format!("0x{:08x}", m.end_address),
                format!("0x{:08x}", m.size),
                format!("0x{:08x}", m.offset),
                m.permissions.clone().unwrap_or("".to_string()),
                m.path.clone(),
            ]);
            rows.push(row);
        }
    }

    let widths = [
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths).block(block);
    f.render_widget(table, mapping_rect);
}
