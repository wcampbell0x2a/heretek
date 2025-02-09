use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::Layout;
use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, style::Style, Frame};

use super::{BLUE, GRAY_FG, GREEN, ORANGE};

use crate::{App, InputMode};

pub fn draw_input(title_area: Rect, app: &App, f: &mut Frame, input: Rect) {
    // Input
    let width = title_area.width - 3;
    // keep 2 for borders and 1 for cursor

    let scroll = app.input.visual_scroll(width as usize);
    let stream_lock = app.stream_output_prompt.lock().unwrap();
    let prompt_len = stream_lock.len();

    let async_result = app.async_result.lock().unwrap();
    let status = app.status.lock().unwrap();

    let txt_input = Paragraph::new(format!("{}{}", stream_lock, app.input.value()))
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(GREEN),
        })
        .scroll((0, scroll as u16))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(vec!["|".fg(GRAY_FG), format!("{status}").fg(BLUE), "|".fg(GRAY_FG)])
                .title(vec![
                    "|".fg(GRAY_FG),
                    format!("{async_result}").fg(ORANGE),
                    "|".fg(GRAY_FG),
                ]),
        );

    f.render_widget(txt_input, input);
    match app.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor_position((
                // Put cursor past the end of the input text
                input.x
                    + ((app.input.visual_cursor()).max(scroll) - scroll) as u16
                    + 1
                    + prompt_len as u16,
                // Move one line down, from the border to the input line
                input.y + 1,
            ))
        }
    }
}
