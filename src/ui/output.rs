use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Scrollbar, ScrollbarOrientation};

use super::{BLUE, SCROLL_CONTROL_TEXT};

use crate::State;

pub fn draw_output(state: &mut State, f: &mut Frame, output: Rect, full: bool) {
    let len = state.output.len();
    let max = output.height;
    let skip = if full {
        if len <= max as usize { 0 } else { state.output_scroll.scroll }
    } else if len <= max as usize {
        0
    } else {
        len - max as usize + 2
    };

    state.output_scroll.state = state.output_scroll.state.content_length(len);

    let outputs: Vec<ListItem> = state
        .output
        .iter()
        .skip(skip)
        .take(max as usize)
        .map(|m| {
            let m = m.replace('\t', "    ");
            let content = vec![Line::from(Span::raw(m.clone()))];
            ListItem::new(content)
        })
        .collect();
    let help = if full { SCROLL_CONTROL_TEXT } else { "" };
    let output_block =
        List::new(outputs).block(Block::bordered().title(format!("Output {help}").fg(BLUE)));
    f.render_widget(output_block, output);

    // only show scrollbar on full page
    if full {
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            output,
            &mut state.output_scroll.state,
        );
    }
}
