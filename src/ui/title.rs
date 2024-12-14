use ratatui::layout::Constraint::Length;
use ratatui::layout::{Alignment, Layout};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, style::Style, Frame};

use super::{HEAP_COLOR, STACK_COLOR, TEXT_COLOR};

use crate::{App, InputMode};

pub fn draw_title_area(app: &App, f: &mut Frame, title_area: Rect) {
    let vertical_title = Layout::vertical([Length(1), Length(1)]);
    let [first, second] = vertical_title.areas(title_area);
    f.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .title(vec![
                "|".fg(Color::Rgb(100, 100, 100)),
                env!("CARGO_PKG_NAME").bold(),
                "-".fg(Color::Rgb(100, 100, 100)),
                "v".into(),
                env!("CARGO_PKG_VERSION").into(),
                "|".fg(Color::Rgb(100, 100, 100)),
            ])
            .title_alignment(Alignment::Center),
        first,
    );
    // Title Area
    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to enter input | "),
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
                Span::raw(" mappings | "),
                Span::styled("Heap", Style::default().fg(HEAP_COLOR).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(
                    "Stack",
                    Style::default().fg(STACK_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled("Code", Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD)),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send input | "),
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
                Span::styled("Heap", Style::default().fg(HEAP_COLOR).add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(
                    "Stack",
                    Style::default().fg(STACK_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled("Code", Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD)),
            ],
            Style::default(),
        ),
    };
    let text = Text::from(Line::from(msg)).style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, second);
}
