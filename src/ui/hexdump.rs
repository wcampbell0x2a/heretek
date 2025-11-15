use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation},
};

use crate::{PtrSize, State};

use super::{BLUE, DARK_GRAY, GREEN, ORANGE, SCROLL_CONTROL_TEXT, YELLOW};

pub const HEXDUMP_WIDTH: usize = 16;

/// Convert bytes in hexdump, `skip` that many lines, `take` that many lines
fn to_hexdump_str<'a>(
    state: &mut State,
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
            hex_spans.push(Span::styled(format!("{byte:02x} "), Style::default().fg(color)));
        }

        // ascii
        hex_spans.push(Span::raw("| "));
        for byte in chunk.iter() {
            let ascii_char = if byte.is_ascii_graphic() { *byte as char } else { '.' };
            let color = color(*byte);
            hex_spans.push(Span::styled(ascii_char.to_string(), Style::default().fg(color)));
        }

        // check if value has a register reference
        let thirty = state.ptr_size == PtrSize::Size32;

        let mut ref_spans = Vec::new();

        ref_spans.push(Span::raw("| "));

        // NOTE: This is disabled, since it's mostly useless?
        //deref_bytes_to_registers(&endian, chunk, thirty, &mut ref_spans, &registers);

        let windows = if thirty { 4 } else { 8 };
        for r in state.registers.iter() {
            if let Some(reg) = &r.register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value
                    && let Ok(val) = u64::from_str_radix(&reg_value[2..], 16)
                {
                    for n in 0..=windows {
                        if val as usize == pos as usize + ((offset + skip) * HEXDUMP_WIDTH + n) {
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

pub fn color(byte: u8) -> Color {
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

fn block(pos: &str) -> Block<'_> {
    Block::default().borders(Borders::ALL).title(
        format!("Hexdump{pos} {SCROLL_CONTROL_TEXT}, Save(S), HEAP(H), STACK(T))").fg(ORANGE),
    )
}

pub fn draw_hexdump(state: &mut State, f: &mut Frame, hexdump: Rect, show_popup: bool) {
    let hexdump_active = state.hexdump.is_some();
    let mut pos = "".to_string();

    if hexdump_active {
        let r = state.hexdump.clone().unwrap();
        pos = format!("(0x{:02x?})", r.0);
        let data = &r.1;

        let skip = state.hexdump_scroll.scroll;
        let take = hexdump.height;
        let lines = to_hexdump_str(state, r.0, data, skip, take as usize);
        let content_len = data.len() / HEXDUMP_WIDTH;

        let lines: Vec<Line> = lines.into_iter().collect();
        state.hexdump_scroll.state = state.hexdump_scroll.state.content_length(content_len);
        let paragraph =
            Paragraph::new(lines).block(block(&pos)).style(Style::default().fg(Color::White));

        f.render_widget(paragraph, hexdump);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            hexdump,
            &mut state.hexdump_scroll.state,
        );
        if show_popup {
            let area = popup_area(hexdump, 60);
            let txt_input = Paragraph::new(state.hexdump_popup.value().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, PtrSize};

    #[test]
    fn test_color_null_byte() {
        assert_eq!(color(0x00), DARK_GRAY);
    }

    #[test]
    fn test_color_ascii_graphic() {
        assert_eq!(color(b'A'), BLUE);
        assert_eq!(color(b'z'), BLUE);
        assert_eq!(color(b'!'), BLUE);
    }

    #[test]
    fn test_color_ascii_whitespace() {
        assert_eq!(color(b' '), GREEN);
        assert_eq!(color(b'\t'), GREEN);
        assert_eq!(color(b'\n'), GREEN);
    }

    #[test]
    fn test_color_ascii_non_graphic() {
        assert_eq!(color(0x01), ORANGE); // SOH - ascii but not graphic/whitespace
        assert_eq!(color(0x7F), ORANGE); // DEL - ascii but not graphic/whitespace
    }

    #[test]
    fn test_color_non_ascii() {
        assert_eq!(color(0x80), YELLOW);
        assert_eq!(color(0xFF), YELLOW);
    }

    #[test]
    fn test_hexdump_width_constant() {
        assert_eq!(HEXDUMP_WIDTH, 16);
    }

    #[test]
    fn test_to_hexdump_str_empty() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        let buffer: Vec<u8> = vec![];
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, 0, 10);
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_to_hexdump_str_single_line() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        let buffer: Vec<u8> = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]; // "Hello"
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, 0, 10);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_to_hexdump_str_multiple_lines() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        // Create 32 bytes which should span 2 lines (16 bytes per line)
        let buffer: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, 0, 10);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_to_hexdump_str_with_skip() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        // Create 48 bytes which should span 3 lines (16 bytes per line)
        let buffer: Vec<u8> = (0..48).map(|i| i as u8).collect();
        // Skip first line, take 2 lines
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, 1, 2);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_to_hexdump_str_with_take_limit() {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        let mut state = State::new(args);
        // Create 64 bytes which should span 4 lines
        let buffer: Vec<u8> = (0..64).map(|i| i as u8).collect();
        // Take only 2 lines
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, 0, 2);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_block_creation() {
        // Just verify the block function returns successfully
        let _b = block("(0x1234)");
        let _b2 = block("");
    }

    #[test]
    fn test_popup_area_dimensions() {
        let area = Rect::new(0, 0, 100, 100);
        let popup = popup_area(area, 60);
        assert_eq!(popup.width, 60);
        assert_eq!(popup.height, 3);
    }

    #[test]
    fn test_popup_area_different_sizes() {
        let area = Rect::new(0, 0, 200, 50);
        let popup = popup_area(area, 80);
        assert_eq!(popup.width, 160); // 80% of 200
        assert_eq!(popup.height, 3);
    }
}
