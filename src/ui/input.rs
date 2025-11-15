use ratatui::prelude::Stylize;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, layout::Rect, style::Style};

use super::{BLUE, GRAY_FG, GREEN, ORANGE};
use crate::{InputMode, State};

pub fn draw_input(title_area: Rect, state: &mut State, f: &mut Frame, input: Rect) {
    // Input
    let width = title_area.width - 3;
    // keep 2 for borders and 1 for cursor

    let scroll = state.input.visual_scroll(width as usize);
    let prompt_len = state.stream_output_prompt.len();

    let txt_input =
        Paragraph::new(format!("{}{}", state.stream_output_prompt, state.input.value()))
            .style(match state.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default().fg(GREEN),
            })
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(vec!["|".fg(GRAY_FG), state.status.clone().fg(BLUE), "|".fg(GRAY_FG)])
                    .title(vec![
                        "|".fg(GRAY_FG),
                        state.async_result.clone().fg(ORANGE),
                        "|".fg(GRAY_FG),
                    ]),
            );

    f.render_widget(txt_input, input);
    match state.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor_position((
                // Put cursor past the end of the input text
                input.x
                    + ((state.input.visual_cursor()).max(scroll) - scroll) as u16
                    + 1
                    + prompt_len as u16,
                // Move one line down, from the border to the input line
                input.y + 1,
            ));
        }
    }
}
