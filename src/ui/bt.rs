use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, layout::Rect, style::Style};

use super::{ORANGE, PURPLE, pane_block};

use crate::State;

pub fn draw_bt(state: &mut State, f: &mut Frame, bt_rect: Rect) {
    let block = pane_block("Backtrace", None, "", false);
    let mut lines = vec![];
    for b in &state.bt {
        let loc_span = Span::from(format!("  {:08x}", b.location,)).style(Style::new().fg(PURPLE));

        let func_span = Span::from(b.function.clone().unwrap_or(String::new()).clone())
            .style(Style::new().fg(ORANGE));
        let spans = vec![loc_span, Span::from(" → "), func_span];
        let line = Line::from(spans);
        lines.push(line);
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, bt_rect);
}
