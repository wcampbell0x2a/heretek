use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation};
use ratatui::Frame;

use super::BLUE;

use crate::App;

pub fn draw_output(app: &mut App, f: &mut Frame, output: Rect, full: bool) {
    let output_lock = app.output.lock().unwrap();

    let len = output_lock.len();
    let max = output.height;
    let skip = if full {
        if len <= max as usize {
            0
        } else {
            app.output_scroll
        }
    } else {
        if len <= max as usize {
            0
        } else {
            len - max as usize + 2
        }
    };
    app.output_scroll_state = app.output_scroll_state.content_length(len);

    let outputs: Vec<ListItem> = output_lock
        .iter()
        .skip(skip)
        .take(max as usize)
        .map(|m| {
            let m = m.replace('\t', "    ");
            let content = vec![Line::from(Span::raw(format!("{}", m)))];
            ListItem::new(content)
        })
        .collect();
    let help = if full { "(up(k), down(j), 50 up(K), 50 down(J))" } else { "" };
    let output_block = List::new(outputs)
        .block(Block::default().borders(Borders::ALL).title(format!("Output {help}").fg(BLUE)));
    f.render_widget(output_block, output);

    // only show scrollbar on full page
    if full {
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            output,
            &mut app.output_scroll_state,
        );
    }
}
