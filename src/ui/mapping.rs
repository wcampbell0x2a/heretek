use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Scrollbar, ScrollbarOrientation, Table};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Row};

use super::{BLUE, ORANGE, SCROLL_CONTROL_TEXT};

use crate::State;

pub fn draw_mapping(state: &mut State, f: &mut Frame, mapping_rect: Rect) {
    let title = format!("Memory Mapping {SCROLL_CONTROL_TEXT}, Hexdump(H)");

    let mut rows = vec![];
    rows.push(
        Row::new(["Start Address", "End Address", "Size", "Offset", "Permissions", "Path"])
            .style(Style::new().fg(BLUE)),
    );
    let memory_map = state.memory_map.clone();
    if let Some(memory_map) = memory_map.as_ref() {
        for (index, m) in memory_map.iter().enumerate() {
            let mut row = Row::new([
                format!("0x{:08x}", m.start_address),
                format!("0x{:08x}", m.end_address),
                format!("0x{:08x}", m.size),
                format!("0x{:08x}", m.offset),
                m.permissions.clone().unwrap_or("".to_string()),
                m.path.clone().unwrap_or("".to_string()),
            ]);
            // Highlight the selected row
            if index == state.memory_map_selected {
                row = row.style(Style::new().fg(ORANGE).bold());
            }
            rows.push(row);
        }
    }
    let len = rows.len();
    let max = mapping_rect.height;
    let skip = if len <= max as usize { 0 } else { state.memory_map_scroll.scroll };

    // Store viewport height for use in key handlers
    state.memory_map_viewport_height = max;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mi::MemoryMapping;
    use crate::{Args, PtrSize};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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

    fn create_test_mapping() -> MemoryMapping {
        MemoryMapping {
            start_address: 0x400000,
            end_address: 0x401000,
            size: 0x1000,
            offset: 0x0,
            permissions: Some("r-xp".to_string()),
            path: Some("/bin/test".to_string()),
        }
    }

    #[test]
    fn test_draw_mapping_empty() {
        let mut state = create_test_state();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_mapping(&mut state, f, area);
            })
            .unwrap();

        // Verify state was updated
        assert_eq!(state.memory_map_viewport_height, 24);
    }

    #[test]
    fn test_draw_mapping_with_data() {
        let mut state = create_test_state();
        state.memory_map = Some(vec![create_test_mapping()]);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_mapping(&mut state, f, area);
            })
            .unwrap();

        // Verify state was updated
        assert_eq!(state.memory_map_viewport_height, 24);
    }

    #[test]
    fn test_draw_mapping_multiple_entries() {
        let mut state = create_test_state();
        state.memory_map = Some(vec![
            create_test_mapping(),
            MemoryMapping {
                start_address: 0x500000,
                end_address: 0x501000,
                size: 0x1000,
                offset: 0x0,
                permissions: Some("rw-p".to_string()),
                path: Some("/lib/test.so".to_string()),
            },
            MemoryMapping {
                start_address: 0x600000,
                end_address: 0x602000,
                size: 0x2000,
                offset: 0x1000,
                permissions: None,
                path: None,
            },
        ]);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_mapping(&mut state, f, area);
            })
            .unwrap();

        // Verify state was updated
        assert_eq!(state.memory_map_viewport_height, 24);
    }

    #[test]
    fn test_draw_mapping_with_selection() {
        let mut state = create_test_state();
        state.memory_map = Some(vec![create_test_mapping(), create_test_mapping()]);
        state.memory_map_selected = 1;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_mapping(&mut state, f, area);
            })
            .unwrap();

        // Verify selection was applied
        assert_eq!(state.memory_map_selected, 1);
    }

    #[test]
    fn test_draw_mapping_with_scroll() {
        let mut state = create_test_state();
        // Create many mappings to require scrolling
        let mappings: Vec<MemoryMapping> = (0..50)
            .map(|i| MemoryMapping {
                start_address: 0x400000 + (i * 0x1000),
                end_address: 0x401000 + (i * 0x1000),
                size: 0x1000,
                offset: 0x0,
                permissions: Some("r-xp".to_string()),
                path: Some(format!("/path/to/lib{}.so", i)),
            })
            .collect();
        state.memory_map = Some(mappings);
        state.memory_map_scroll.scroll = 10;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_mapping(&mut state, f, area);
            })
            .unwrap();

        // Verify scroll was applied
        assert_eq!(state.memory_map_scroll.scroll, 10);
    }
}
