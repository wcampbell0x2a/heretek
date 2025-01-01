use std::sync::atomic::Ordering;

use ratatui::prelude::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, style::Style, Frame};

use super::{add_deref_to_span, ORANGE, PURPLE};

use crate::App;

pub fn draw_stack(app: &App, f: &mut Frame, stack: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Stack".fg(ORANGE));
    let mut lines = vec![];
    let mut longest_cells = 0;
    let width: usize = if app.thirty_two_bit.load(Ordering::Relaxed) { 11 } else { 19 };

    if let Ok(stack) = app.stack.lock() {
        let mut entries: Vec<_> = stack.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (addr, values) in entries.iter() {
            let filepath_lock = app.filepath.lock().unwrap();
            let binding = filepath_lock.as_ref().unwrap();
            let filepath = binding.to_string_lossy();

            let hex_string = format!("0x{:02x}", addr);
            let hex_width = hex_string.len();
            let padding_width = (width - 4).saturating_sub(hex_width);
            let span =
                Span::from(format!("  {}{:padding$}", hex_string, "", padding = padding_width))
                    .style(Style::new().fg(PURPLE));
            let mut spans = vec![span];
            add_deref_to_span(values, &mut spans, app, &filepath, &mut longest_cells, width);
            let line = Line::from(spans);
            lines.push(line);
        }
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, stack);
}
