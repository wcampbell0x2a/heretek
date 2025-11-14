use ratatui::layout::Constraint::Length;
use ratatui::layout::{Alignment, Layout};
use ratatui::prelude::Stylize;
use ratatui::style::Modifier;
use ratatui::text::Span;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::{Frame, layout::Rect, style::Style};

use super::{ASM_COLOR, GRAY_FG, GREEN, HEAP_COLOR, STACK_COLOR, STRING_COLOR, TEXT_COLOR};

use crate::{InputMode, State};

pub fn draw_title_area(state: &mut State, f: &mut Frame, title_area: Rect) {
    let vertical_title = Layout::vertical([Length(1), Length(1)]);
    let [first, second] = vertical_title.areas(title_area);
    f.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .title(
                Title::from(vec![
                    "|".fg(GRAY_FG),
                    env!("CARGO_PKG_NAME").bold(),
                    "-".fg(GRAY_FG),
                    "v".into(),
                    env!("CARGO_PKG_VERSION").into(),
                    "|".fg(GRAY_FG),
                ])
                .alignment(Alignment::Center),
            )
            .title(
                Title::from(vec![
                    Span::raw(" | "),
                    Span::styled(
                        "Heap",
                        Style::default().fg(HEAP_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        "Stack",
                        Style::default().fg(STACK_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        "Code",
                        Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        "String",
                        Style::default().fg(STRING_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        "Asm",
                        Style::default().fg(ASM_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                ])
                .alignment(Alignment::Right),
            ),
        first,
    );
    // Title Area
    state.status = match state.input_mode {
        InputMode::Normal => "Press q to exit, i to enter input".to_owned(),
        InputMode::Editing => "Press Esc to stop editing, Enter to send input".to_owned(),
    };

    let mode = &state.mode;
    let tab = Tabs::new(vec![
        "F1 Main",
        "F2 Registers",
        "F3 Stack",
        "F4 Instructions",
        "F5 Output",
        "F6 Mapping",
        "F7 Hexdump",
        "F8 Symbols",
    ])
    .block(Block::new().title_alignment(Alignment::Center))
    .style(Style::default())
    .highlight_style(Style::default().fg(GREEN).add_modifier(Modifier::BOLD))
    .select(mode.ui_index())
    .divider("|");

    f.render_widget(tab, second);
}
