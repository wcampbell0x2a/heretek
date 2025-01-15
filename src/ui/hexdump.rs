use std::sync::atomic::Ordering;

use deku::ctx::Endian;
use log::{debug, trace};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::App;

use super::{BLUE, DARK_GRAY, GREEN, ORANGE, SCROLL_CONTROL_TEXT, YELLOW};

pub const HEXDUMP_WIDTH: usize = 16;

/// Convert bytes in hexdump, `skip` that many lines, `take` that many lines
fn to_hexdump_str<'a>(
    app: &mut App,
    pos: u64,
    buffer: &[u8],
    skip: usize,
    take: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    for (offset, chunk) in buffer.chunks(16).skip(skip).take(take).enumerate() {
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

        // check if value has a register reference
        let thirty = app.thirty_two_bit.load(Ordering::Relaxed);

        let mut ref_spans = Vec::new();
        let endian = app.endian.lock().unwrap();
        let registers = app.registers.lock().unwrap();

        ref_spans.push(Span::raw("| "));

        // NOTE: This is disabled, since it's mostly useless?
        //deref_bytes_to_registers(&endian, chunk, thirty, &mut ref_spans, &registers);

        let windows = if thirty { 4 } else { 8 };
        for r in registers.iter() {
            if let Some(reg) = &r.register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value {
                    if let Ok(val) = u64::from_str_radix(&reg_value[2..], 16) {
                        for n in 0..=windows {
                            if val as usize == pos as usize + ((offset + skip) * HEXDUMP_WIDTH + n)
                            {
                                ref_spans.push(Span::raw(format!(
                                    "← ${}(0x{:02x}) ",
                                    r.name.clone(),
                                    val
                                )));
                            }
                        }
                    }
                }
            }
        }

        let line = Line::from_iter(
            vec![Span::raw(format!("{:08x}: ", (skip + offset) * HEXDUMP_WIDTH)), Span::raw("")]
                .into_iter()
                .chain(hex_spans)
                .chain(ref_spans),
        );

        lines.push(line);
    }

    lines
}

fn deref_bytes_to_registers(
    endian: &Option<Endian>,
    chunk: &[u8],
    thirty: bool,
    ref_spans: &mut Vec<Span<'_>>,
    registers: &Vec<crate::register::RegisterStorage>,
) {
    let windows = if thirty { 4 } else { 8 };
    for w in chunk.windows(windows) {
        let bytes_val = if thirty {
            let val = if endian.unwrap() == Endian::Big {
                // TODO: try_into()
                u32::from_be_bytes([w[0], w[1], w[2], w[3]])
            } else {
                u32::from_le_bytes([w[0], w[1], w[2], w[3]])
            };

            val as u64
        } else {
            if endian.unwrap() == Endian::Big {
                u64::from_be_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]])
            } else {
                u64::from_le_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]])
            }
        };

        for r in registers.iter() {
            if let Some(reg) = &r.register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value {
                    if let Ok(val) = u64::from_str_radix(&reg_value[2..], 16) {
                        if val != 0 {
                            // Find registers that are pointing to the value at a byte offset
                            if bytes_val == val {
                                ref_spans.push(Span::raw(format!(
                                    "${}(0x{:02x?}) ",
                                    r.name.clone(),
                                    val
                                )));
                            }
                        }
                    }
                }
            }
        }
    }
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
    let hexdump_active = app.hexdump.lock().unwrap().is_some();
    let mut pos = "".to_string();

    if hexdump_active {
        let r = app.hexdump.lock().unwrap().clone().unwrap();
        pos = format!("(0x{:02x?})", r.0);
        let data = &r.1;

        let skip = app.hexdump_scroll;
        let take = hexdump.height;
        let lines = to_hexdump_str(app, r.0, data, skip as usize, take as usize);
        let content_len = data.len() / HEXDUMP_WIDTH;

        let lines: Vec<Line> = lines.into_iter().collect();
        app.hexdump_scroll_state = app.hexdump_scroll_state.content_length(content_len);
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
