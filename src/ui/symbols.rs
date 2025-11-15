use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Scrollbar, ScrollbarOrientation, Table};
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
        format!("Symbols - Filtered: \"{}\" (/ to reset)", state.symbols_search_input.value())
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

    let block = Block::default().borders(Borders::ALL).title(title.fg(ORANGE));
    let table = Table::new(rows, widths).block(block);
    f.render_widget(table, area);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut state.symbols_scroll.state,
    );
}

fn draw_symbol_asm(state: &mut State, f: &mut Frame, area: Rect) {
    let title = if state.symbols.is_empty() {
        "Disassembly (no symbols loaded)".to_string()
    } else if let Some(sym) = state.symbols.get(state.symbols_selected) {
        format!("Disassembly: {} {SCROLL_CONTROL_TEXT}, Back(Esc)", sym.name)
    } else {
        "Disassembly".to_string()
    };

    let mut rows = vec![Row::new(["Address", "Instruction"]).style(Style::new().fg(BLUE))];

    for asm in state.symbol_asm.iter() {
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
    let block = Block::default().borders(Borders::ALL).title(title.fg(GREEN));
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
    let block = Block::default().borders(Borders::ALL).title("Search (fuzzy)".fg(ORANGE));

    let width = area.width.saturating_sub(2) as usize;
    let scroll = state.symbols_search_input.visual_scroll(width);
    let paragraph = Paragraph::new(search_text).block(block).scroll((0, scroll as u16));

    f.render_widget(paragraph, area);

    // Set cursor position
    let cursor_pos = state.symbols_search_input.visual_cursor();
    f.set_cursor_position((area.x + 1 + (cursor_pos.saturating_sub(scroll)) as u16, area.y + 1));
}
