use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::App;

use super::{BLUE, DARK_GRAY, GREEN, ORANGE, SCROLL_CONTROL_TEXT, YELLOW};

fn to_hexdump_str(buffer: &[u8]) -> Vec<Line> {
    let mut lines = Vec::new();
    for (offset, chunk) in buffer.chunks(16).enumerate() {
        let mut hex_spans = Vec::new();
        // bytes
        for byte in chunk.iter() {
            let color = color(*byte);
            hex_spans.push(Span::styled(format!("{:02x} ", byte), Style::default().fg(color)));
        }

        // ascii
        hex_spans.push(Span::raw("| "));
        for byte in chunk.iter() {
            let ascii_char = if byte.is_ascii_graphic() { *byte as char } else { '.' };
            let color = color(*byte);
            hex_spans.push(Span::styled(ascii_char.to_string(), Style::default().fg(color)));
        }

        let line = Line::from_iter(
            vec![Span::raw(format!("{:08x}: ", offset * 16)), Span::raw("")]
                .into_iter()
                .chain(hex_spans),
        );

        lines.push(line);
    }

    lines
}

fn color(byte: u8) -> Color {
    if byte == 0x00 {
        DARK_GRAY
    } else if byte.is_ascii_graphic() {
        BLUE
    } else if byte.is_ascii_whitespace() {
        GREEN
    } else if byte.is_ascii() {
        ORANGE
    } else {
        YELLOW
    }
}

fn popup_area(area: Rect, percent_x: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(3)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

fn block(pos: &str) -> Block {
    let block = Block::default().borders(Borders::ALL).title(
        format!("Hexdump{pos} {SCROLL_CONTROL_TEXT}, Save(S), HEAP(H), STACK(T))").fg(ORANGE),
    );
    block
}

pub fn draw_hexdump(app: &mut App, f: &mut Frame, hexdump: Rect, show_popup: bool) {
    let hexdump_lock = app.hexdump.lock().unwrap();
    let mut pos = "".to_string();

    if let Some(r) = hexdump_lock.as_ref() {
        pos = format!("(0x{:02x?})", r.0);
        let data = &r.1;

        let lines = to_hexdump_str(data);
        let len = lines.len();

        let max = hexdump.height;
        let skip = if len <= max as usize { 0 } else { app.hexdump_scroll };
        let lines: Vec<Line> = lines.into_iter().skip(skip).collect();
        app.hexdump_scroll_state = app.hexdump_scroll_state.content_length(len);
        let paragraph =
            Paragraph::new(lines).block(block(&pos)).style(Style::default().fg(Color::White));

        f.render_widget(paragraph, hexdump);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            hexdump,
            &mut app.hexdump_scroll_state,
        );
        if show_popup {
            let area = popup_area(hexdump, 60);
            let txt_input = Paragraph::new(app.hexdump_popup.value().to_string())
                .style(Style::default())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Save to".fg(YELLOW))
                        .border_style(Style::default().fg(ORANGE)),
                );
            f.render_widget(Clear, area);
            f.render_widget(txt_input, area);
        }
    } else {
        f.render_widget(Paragraph::new("").block(block(&pos)), hexdump);
    }
}
