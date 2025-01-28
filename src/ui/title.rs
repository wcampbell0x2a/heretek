use ratatui::layout::Constraint::Length;
use ratatui::layout::{Alignment, Layout};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, style::Style, Frame};

use super::{GRAY_FG, HEAP_COLOR, STACK_COLOR, TEXT_COLOR};

use crate::{App, InputMode};

pub fn draw_title_area(app: &App, f: &mut Frame, title_area: Rect) {
    let vertical_title = Layout::vertical([Length(1), Length(1)]);
    let [first, second] = vertical_title.areas(title_area);
    f.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .title(vec![
                "|".fg(GRAY_FG),
                env!("CARGO_PKG_NAME").bold(),
                "-".fg(GRAY_FG),
                "v".into(),
                env!("CARGO_PKG_VERSION").into(),
                "|".fg(GRAY_FG),
            ])
            .title_alignment(Alignment::Center),
        first,
    );
    // Title Area
    match app.input_mode {
        InputMode::Normal => {
            let mut status = app.status.lock().unwrap();
            *status = "Press q to exit, i to enter input".to_owned();
        }
        InputMode::Editing => {
            let mut status = app.status.lock().unwrap();
            *status = "Press Esc to stop editing, Enter to send input".to_owned();
        }
    };

    let msg = vec![
        Span::styled("F1", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" main | "),
        Span::styled("F2", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" registers | "),
        Span::styled("F3", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" stack | "),
        Span::styled("F4", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" instructions | "),
        Span::styled("F5", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" output | "),
        Span::styled("F6", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" mapping | "),
        Span::styled("F7", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" hexdump | "),
        Span::styled("Heap", Style::default().fg(HEAP_COLOR).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled("Stack", Style::default().fg(STACK_COLOR).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled("Code", Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD)),
    ];
    let text = Text::from(Line::from(msg));
    let help_message = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(help_message, second);
}
