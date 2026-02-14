use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Scrollbar, ScrollbarOrientation, Table};
use ratatui::{
    Frame,
    layout::{Layout, Rect},
    style::Style,
    widgets::Row,
};

use super::{BLUE, GREEN, ORANGE, SCROLL_CONTROL_TEXT};
use crate::State;

pub fn draw_symbols(state: &mut State, f: &mut Frame, area: Rect) {
    if state.symbols_viewing_asm {
        // Split into left (symbol list) and right (assembly)
        let horizontal =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);
        let [left_panel, right_panel] = horizontal.areas(area);

        // Draw symbol list on left
        draw_symbol_list(state, f, left_panel, true);

        // Draw assembly on right
        draw_symbol_asm(state, f, right_panel);
    } else {
        // Show only symbol list full width, with optional search bar at bottom when actively searching
        if state.symbols_search_active {
            let vertical = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]);
            let [list_area, search_area] = vertical.areas(area);
            draw_symbol_list(state, f, list_area, false);
            draw_search_input(state, f, search_area);
        } else {
            draw_symbol_list(state, f, area, false);
        }
    }
}

fn draw_symbol_list(state: &mut State, f: &mut Frame, area: Rect, viewing_asm: bool) {
    let title = if viewing_asm {
        "Symbols".to_string()
    } else if state.symbols_search_active {
        "Symbols - Search (Enter/Esc to finish)".to_string()
    } else if !state.symbols_search_input.value().is_empty() {
        format!(
            "Symbols - Filtered: \"{}\" {SCROLL_CONTROL_TEXT} ('/' to reset), Search(/), Refresh(r), Disasm(Enter)",
            state.symbols_search_input.value()
        )
    } else {
        format!("Symbols {SCROLL_CONTROL_TEXT}, Search(/), Refresh(r), Disasm(Enter)")
    };

    let mut rows = vec![Row::new(["Address", "Name"]).style(Style::new().fg(BLUE))];

    // Use filtered symbols when searching
    let filtered_symbols = state.get_filtered_symbols();

    for (list_index, (_original_index, sym)) in filtered_symbols.iter().enumerate() {
        let mut row = Row::new([format!("0x{:016x}", sym.address), sym.name.clone()]);

        if list_index == state.symbols_selected {
            row = row.style(Style::new().fg(ORANGE).bold());
        }
        rows.push(row);
    }

    // Handle scrolling
    let len = rows.len();
    let max = area.height.saturating_sub(2); // Account for border
    let skip = if len <= max as usize { 0 } else { state.symbols_scroll.scroll };

    // Store viewport height for use in key handlers
    state.symbols_viewport_height = max;
    state.symbols_scroll.state = state.symbols_scroll.state.content_length(len);
    let rows: Vec<Row> = rows.into_iter().skip(skip).take(max as usize).collect();

    let widths = [Constraint::Length(18), Constraint::Fill(1)];

    let block = Block::bordered().title(title.fg(ORANGE));
    let table = Table::new(rows, widths).block(block);
    f.render_widget(table, area);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut state.symbols_scroll.state,
    );
}

fn draw_symbol_asm(state: &mut State, f: &mut Frame, area: Rect) {
    let title = if state.symbol_asm_name.is_empty() {
        format!("Disassembly {SCROLL_CONTROL_TEXT}, Back(Esc)")
    } else {
        format!("Disassembly: {} {SCROLL_CONTROL_TEXT}, Back(Esc)", state.symbol_asm_name)
    };

    let mut rows = vec![Row::new(["Address", "Instruction"]).style(Style::new().fg(BLUE))];

    for asm in &state.symbol_asm {
        let row = Row::new([format!("0x{:016x}", asm.address), asm.inst.clone()]);
        rows.push(row);
    }

    // Handle scrolling
    let len = rows.len();
    let max = area.height.saturating_sub(2); // Account for border
    let skip = if len <= max as usize { 0 } else { state.symbol_asm_scroll.scroll };

    state.symbol_asm_scroll.state = state.symbol_asm_scroll.state.content_length(len);
    let rows: Vec<Row> = rows.into_iter().skip(skip).take(max as usize).collect();

    let widths = [Constraint::Length(18), Constraint::Fill(1)];
    let block = Block::bordered().title(title.fg(GREEN));
    let table = Table::new(rows, widths).block(block);
    f.render_widget(table, area);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut state.symbol_asm_scroll.state,
    );
}

fn draw_search_input(state: &State, f: &mut Frame, area: Rect) {
    use ratatui::widgets::Paragraph;

    let search_text = state.symbols_search_input.value();
    let block = Block::bordered().title("Search (fuzzy)".fg(ORANGE));

    let width = area.width.saturating_sub(2) as usize;
    let scroll = state.symbols_search_input.visual_scroll(width);
    let paragraph = Paragraph::new(search_text).block(block).scroll((0, scroll as u16));

    f.render_widget(paragraph, area);

    // Set cursor position
    let cursor_pos = state.symbols_search_input.visual_cursor();
    f.set_cursor_position((area.x + 1 + (cursor_pos.saturating_sub(scroll)) as u16, area.y + 1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, PtrSize, Symbol};
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

    fn create_test_symbols() -> Vec<Symbol> {
        vec![
            Symbol { address: 0x401000, name: "main".to_string(), needs_address_resolution: false },
            Symbol { address: 0x401100, name: "foo".to_string(), needs_address_resolution: false },
            Symbol { address: 0x401200, name: "bar".to_string(), needs_address_resolution: false },
        ]
    }

    #[test]
    fn test_draw_symbols_empty() {
        let mut state = create_test_state();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_symbols_with_data() {
        let mut state = create_test_state();
        state.symbols = create_test_symbols();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_symbols_viewing_asm() {
        let mut state = create_test_state();
        state.symbols = create_test_symbols();
        state.symbols_viewing_asm = true;
        state.symbol_asm = vec![crate::mi::Asm {
            address: 0x401000,
            inst: "push rbp".to_string(),
            offset: 0,
            func_name: Some("main".to_string()),
        }];

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_symbols_search_active() {
        let mut state = create_test_state();
        state.symbols = create_test_symbols();
        state.symbols_search_active = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_symbols_with_selection() {
        let mut state = create_test_state();
        state.symbols = create_test_symbols();
        state.symbols_selected = 1;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();

        assert_eq!(state.symbols_selected, 1);
    }

    #[test]
    fn test_draw_symbols_with_scroll() {
        let mut state = create_test_state();
        // Create many symbols to require scrolling
        state.symbols = (0..100)
            .map(|i| Symbol {
                address: 0x400000 + (i * 0x10),
                name: format!("func_{i}"),
                needs_address_resolution: false,
            })
            .collect();
        state.symbols_scroll.scroll = 10;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();

        assert_eq!(state.symbols_scroll.scroll, 10);
    }

    #[test]
    fn test_draw_symbols_asm_scroll() {
        let mut state = create_test_state();
        state.symbols = create_test_symbols();
        state.symbols_viewing_asm = true;
        // Create many asm instructions
        state.symbol_asm = (0..100)
            .map(|i| crate::mi::Asm {
                address: 0x401000 + i,
                inst: format!("instruction_{i}"),
                offset: i,
                func_name: Some("main".to_string()),
            })
            .collect();
        state.symbol_asm_scroll.scroll = 5;

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();

        assert_eq!(state.symbol_asm_scroll.scroll, 5);
    }

    #[test]
    fn test_draw_symbols_with_filter() {
        let mut state = create_test_state();
        state.symbols = create_test_symbols();
        state.symbols_search_input = tui_input::Input::new("ma".to_string());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_symbols(&mut state, f, area);
            })
            .unwrap();
    }
}
