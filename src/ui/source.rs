use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

use arborium::AnsiHighlighter;

use super::{GREEN, ORANGE};

use crate::State;

pub fn draw_source(state: &mut State, f: &mut Frame, area: Rect) {
    let language = state.source_language.clone().unwrap_or_else(|| "c".to_string());

    let title =
        if let (Some(file), Some(line)) = (&state.current_source_file, state.current_source_line) {
            let filename =
                std::path::Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
            Line::from(format!("Source ({filename}:{line}) ({language})").fg(ORANGE))
        } else {
            return;
        };

    if state.source_lines.is_empty() || state.current_source_line.is_none() {
        let block = Block::bordered().title(title);
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

    let theme = arborium::theme::builtin::ayu_dark();
    let mut highlighter = AnsiHighlighter::new(theme);

    let lines_to_display: Vec<String> = state
        .source_lines
        .clone()
        .into_iter()
        .skip(start_line)
        .take(end_line - start_line)
        .collect();

    let joined_lines = lines_to_display.join("\n");

    let ansi_text = highlighter
        .highlight(&language, &joined_lines)
        .unwrap_or_else(|_| joined_lines.to_string());

    // Remove strikethrough ANSI codes as they're not useful for syntax highlighting
    let ansi_text = ansi_text.replace("\x1b[9m", "");

    let parsed_lines: Vec<Line> = match ansi_to_tui::IntoText::into_text(&ansi_text) {
        Ok(text) => text.lines,
        Err(_) => lines_to_display.iter().map(|s| Line::raw(s.to_string())).collect(),
    };

    let rows: Vec<Row> = lines_to_display
        .iter()
        .enumerate()
        .map(|(i, line_content)| {
            let line_num = start_line + i + 1;
            let is_current = line_num == current_line;
            let marker = if is_current {
                Cell::from(">").style(Style::default().fg(GREEN))
            } else {
                Cell::from(" ")
            };

            let line_num_cell = Cell::from(format!("{:>4}", line_num)).style(if is_current {
                Style::default().fg(GREEN)
            } else {
                Style::default()
            });

            // Use the pre-highlighted line if available
            let mut line =
                parsed_lines.get(i).cloned().unwrap_or_else(|| Line::raw(line_content.to_string()));

            // Add padding at the start
            line.spans.insert(0, " ".into());

            let content_cell = Cell::from(line);

            Row::new(vec![marker, line_num_cell, content_cell])
        })
        .collect();

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
