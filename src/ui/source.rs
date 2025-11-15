use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

use super::{GREEN, ORANGE};

use crate::State;

pub fn draw_source(state: &mut State, f: &mut Frame, area: Rect) {
    let title =
        if let (Some(file), Some(line)) = (&state.current_source_file, state.current_source_line) {
            let filename =
                std::path::Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
            Title::from(format!("Source ({filename}:{line})").fg(ORANGE))
        } else {
            return;
        };

    if state.source_lines.is_empty() || state.current_source_line.is_none() {
        let block = Block::default().borders(Borders::ALL).title(title);
        f.render_widget(block, area);
        return;
    }

    let current_line = state.current_source_line.unwrap() as usize;
    let total_lines = state.source_lines.len();

    // Calculate which lines to show (center the current line in the view)
    // Account for borders and title
    let lines_to_show = (area.height as usize).saturating_sub(3);
    let start_line = if current_line > lines_to_show / 2 {
        (current_line.saturating_sub(lines_to_show / 2)).saturating_sub(1)
    } else {
        0
    };
    let end_line = (start_line + lines_to_show).min(total_lines);

    let mut rows = vec![];
    for (idx, line_content) in
        state.source_lines.iter().enumerate().skip(start_line).take(end_line - start_line)
    {
        let line_num = idx + 1;
        let line_num_cell = Cell::from(format!("{line_num:4}"));

        let content_cell = if line_num == current_line {
            Cell::from(format!(" {line_content}")).style(Style::default().fg(GREEN).bold())
        } else {
            Cell::from(format!(" {line_content}")).white()
        };

        let marker_cell = if line_num == current_line {
            Cell::from(">").style(Style::default().fg(GREEN).bold())
        } else {
            Cell::from(" ")
        };

        rows.push(Row::new(vec![marker_cell, line_num_cell, content_cell]));
    }

    let widths = [Constraint::Length(1), Constraint::Length(4), Constraint::Fill(1)];

    let table = Table::new(rows, widths).block(Block::default().borders(Borders::TOP).title(title));

    let mut table_state = TableState::default();
    if current_line > start_line {
        table_state = table_state.with_selected(current_line - start_line - 1);
    }

    f.render_stateful_widget(table, area, &mut table_state);
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_draw_source_no_file() {
        let mut state = create_test_state();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();

        // Function should return early when no source file is set
    }

    #[test]
    fn test_draw_source_with_file_no_lines() {
        let mut state = create_test_state();
        state.current_source_file = Some("test.c".to_string());
        state.current_source_line = Some(10);
        // Empty source_lines

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_source_with_file_and_lines() {
        let mut state = create_test_state();
        state.current_source_file = Some("test.c".to_string());
        state.current_source_line = Some(5);
        state.source_lines = vec![
            "int main() {".to_string(),
            "    int x = 0;".to_string(),
            "    int y = 1;".to_string(),
            "    int z = 2;".to_string(),
            "    return x + y + z;".to_string(),
            "}".to_string(),
        ];

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_source_many_lines_centered() {
        let mut state = create_test_state();
        state.current_source_file = Some("test.c".to_string());
        state.current_source_line = Some(50);
        // Create 100 lines
        state.source_lines = (1..=100).map(|i| format!("line {i}")).collect();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_source_first_line() {
        let mut state = create_test_state();
        state.current_source_file = Some("test.c".to_string());
        state.current_source_line = Some(1);
        state.source_lines = vec!["first line".to_string(), "second line".to_string()];

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_source_last_line() {
        let mut state = create_test_state();
        state.current_source_file = Some("/path/to/long/directory/test.c".to_string());
        state.current_source_line = Some(10);
        state.source_lines = (1..=10).map(|i| format!("line {i}")).collect();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_source_no_line_number() {
        let mut state = create_test_state();
        state.current_source_file = Some("test.c".to_string());
        state.current_source_line = None;
        state.source_lines = vec!["line 1".to_string()];

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_source(&mut state, f, area);
            })
            .unwrap();
    }
}
