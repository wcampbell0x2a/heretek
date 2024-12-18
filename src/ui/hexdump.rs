use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::App;

use super::{ORANGE, SCROLL_CONTROL_TEXT, YELLOW};

fn to_hexdump_str(data: &[u8]) -> String {
    data.chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let address = format!("{:08x}:", i * 16);
            let hex_values =
                chunk.iter().map(|byte| format!("{:02x}", byte)).collect::<Vec<_>>().join(" ");
            let ascii_values = chunk
                .iter()
                .map(|&byte| if byte.is_ascii_graphic() { byte as char } else { '.' })
                .collect::<String>();
            format!("{:<10} {:48} |{}|", address, hex_values, ascii_values)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn popup_area(area: Rect, percent_x: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(3)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn draw_hexdump(app: &mut App, f: &mut Frame, hexdump: Rect, show_popup: bool) {
    let hexdump_lock = app.hexdump.lock().unwrap();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Hexdump {SCROLL_CONTROL_TEXT}, Save(S))").fg(ORANGE));
    if let Some(r) = hexdump_lock.as_ref() {
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
        if show_popup {
            let area = popup_area(hexdump, 60);
            let txt_input = Paragraph::new(format!("{}", app.hexdump_popup.value()))
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
        f.render_widget(Paragraph::new("").block(block), hexdump);
    }
}
