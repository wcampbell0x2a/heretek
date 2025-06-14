use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Scrollbar, ScrollbarOrientation, Table};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Row};

use super::{BLUE, ORANGE, SCROLL_CONTROL_TEXT};

use crate::State;

pub fn draw_mapping(state: &mut State, f: &mut Frame, mapping_rect: Rect) {
    let title = format!("Memory Mapping {SCROLL_CONTROL_TEXT}");

    let mut rows = vec![];
    rows.push(
        Row::new(["Start Address", "End Address", "Size", "Offset", "Permissions", "Path"])
            .style(Style::new().fg(BLUE)),
    );
    let memory_map = state.memory_map.clone();
    if let Some(memory_map) = memory_map.as_ref() {
        for m in memory_map {
            let row = Row::new([
                format!("0x{:08x}", m.start_address),
                format!("0x{:08x}", m.end_address),
                format!("0x{:08x}", m.size),
                format!("0x{:08x}", m.offset),
                m.permissions.clone().unwrap_or("".to_string()),
                m.path.clone().unwrap_or("".to_string()),
            ]);
            rows.push(row);
        }
    }
    let len = rows.len();
    let max = mapping_rect.height;
    let skip = if len <= max as usize { 0 } else { state.memory_map_scroll.scroll };

    state.memory_map_scroll.state = state.memory_map_scroll.state.content_length(len);
    let rows: Vec<Row> = rows.into_iter().skip(skip).take(max as usize).collect();

    let widths = [
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Fill(1),
    ];
    let block = Block::default().borders(Borders::ALL).title(title.fg(ORANGE));
    let table = Table::new(rows, widths).block(block);
    f.render_widget(table, mapping_rect);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        mapping_rect,
        &mut state.memory_map_scroll.state,
    );
}
