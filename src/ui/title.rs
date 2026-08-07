use ratatui::layout::Constraint::Length;
use ratatui::layout::{Alignment, Layout};
use ratatui::prelude::Stylize;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::{Frame, layout::Rect, style::Style};

use super::{GRAY, GRAY_FG, GREEN};

use crate::State;

pub fn draw_title_area(state: &mut State, f: &mut Frame, title_area: Rect) {
    let vertical_title = Layout::vertical([Length(1), Length(1)]);
    let [first, second] = vertical_title.areas(title_area);
    f.render_widget(
        Block::new().borders(Borders::TOP).border_style(Style::default().fg(GRAY)).title_top(
            Line::from(vec![
                Span::raw(" "),
                env!("CARGO_PKG_NAME").bold(),
                Span::styled(
                    concat!(" v", env!("CARGO_PKG_VERSION"), " "),
                    Style::default().fg(GRAY_FG),
                ),
            ])
            .centered(),
        ),
        first,
    );
    let mode = &state.mode;
    // Use previous_mode's index when in an overlay to maintain selection
    let selected_index = if matches!(mode, crate::Mode::QuitConfirmation | crate::Mode::Help) {
        state.previous_mode.ui_index()
    } else {
        mode.ui_index()
    };
    let titles = [
        "F1 Main",
        "F2 Registers",
        "F3 Stack",
        "F4 Instructions",
        "F5 Output",
        "F6 Mapping",
        "F7 Hexdump",
        "F8 Symbols",
        "F9 Source",
    ];

    // Record clickable regions for mouse tab selection, mirroring ratatui's
    // Tabs layout: [pad_left(1)][title][pad_right(1)]([divider(1)])... starting
    // at the left of `second` (the block has no borders, so inner == area)
    state.tab_regions.clear();
    let mut x = second.left();
    for (i, title) in titles.iter().enumerate() {
        if x >= second.right() {
            break;
        }
        x += 1; // left padding
        let start = x;
        let title_width = title.chars().count() as u16;
        x = (x + title_width).min(second.right());
        state.tab_regions.push((second.top(), start, x));
        x += 1; // right padding
        if i + 1 < titles.len() {
            x += 1; // divider
        }
    }

    let tab = Tabs::new(titles.to_vec())
        .block(Block::new().title_alignment(Alignment::Center))
        .style(Style::default())
        .highlight_style(Style::default().fg(GREEN).add_modifier(Modifier::BOLD))
        .select(selected_index)
        .divider("|".fg(GRAY_FG));

    f.render_widget(tab, second);
}
