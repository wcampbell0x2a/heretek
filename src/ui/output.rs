use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Scrollbar, ScrollbarOrientation};

use super::{BLUE, pane_block};

use crate::State;

pub fn draw_output(state: &mut State, f: &mut Frame, output: Rect, full: bool) {
    let len = state.output.len();
    // account for the top border
    let visible = (output.height as usize).saturating_sub(1);
    state.output_scroll.set_max_scroll(len.saturating_sub(visible));

    // auto-scroll to bottom when new output is added (tail -f behavior)
    if full && len > state.output_prev_len {
        state.output_scroll.end();
        state.output_prev_len = len;
    }

    let skip = if full { state.output_scroll.scroll } else { len.saturating_sub(visible) };

    let outputs: Vec<ListItem> = state
        .output
        .iter()
        .skip(skip)
        .take(visible)
        .map(|m| {
            let m = m.replace('\t', "    ");
            // inferior stdout/stderr
            let span = if m.starts_with("p> ") {
                Span::styled(m.clone(), Style::default().fg(BLUE))
            } else {
                Span::raw(m.clone())
            };
            ListItem::new(vec![Line::from(span)])
        })
        .collect();
    let output_block = List::new(outputs).block(pane_block("Output", None, "", full));
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
