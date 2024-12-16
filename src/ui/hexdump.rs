use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::App;

use super::ORANGE;

fn to_hexdump_str(data: &[u8]) -> String {
    data.chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let address = format!("{:08x}:", i * 16);
            let hex_values =
                chunk.iter().map(|byte| format!("{:02x}", byte)).collect::<Vec<_>>().join(" ");
            let ascii_values = chunk
                .iter()
                .map(|&byte| {
                    if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
                        byte as char
                    } else {
                        '.'
                    }
                })
                .collect::<String>();
            format!("{:<10} {:48} |{}|", address, hex_values, ascii_values)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn draw_hexdump(app: &mut App, f: &mut Frame, hexdump: Rect) {
    let last_read = app.hexdump.lock().unwrap();
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Hexdump (up(k), down(j), 50 up(K), 50 down(J))".fg(ORANGE));
    if let Some(r) = last_read.as_ref() {
        let data = &r.1;

        let data = to_hexdump_str(data);
        let lines = data.lines();
        let len = lines.count();

        let max = hexdump.height;
        let skip = if len <= max as usize { 0 } else { app.hexdump_scroll };
        app.hexdump_scroll_state = app.hexdump_scroll_state.content_length(len);

        let lines: Vec<&str> = data.lines().skip(skip).collect();
        let paragraph =
            Paragraph::new(lines.join("\n")).block(block).style(Style::default().fg(Color::White));
        f.render_widget(paragraph, hexdump);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            hexdump,
            &mut app.hexdump_scroll_state,
        );
    } else {
        f.render_widget(Paragraph::new("").block(block), hexdump);
    }
}
