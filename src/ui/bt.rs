use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, style::Style, Frame};

use super::{ORANGE, PURPLE};

use crate::App;

pub fn draw_bt(app: &App, f: &mut Frame, bt_rect: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Backtrace".fg(ORANGE));
    let mut lines = vec![];
    let bt = app.bt.lock().unwrap();
    if !bt.is_empty() {
        for b in bt.iter() {
            let loc_span =
                Span::from(format!("  {:08x}", b.location,)).style(Style::new().fg(PURPLE));

            let func_span = Span::from(format!("{}", b.function.clone().unwrap_or("".to_string())))
                .style(Style::new().fg(ORANGE));
            let spans = vec![loc_span, Span::from(" → "), func_span];
            let line = Line::from(spans);
            lines.push(line);
        }
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, bt_rect);
}
