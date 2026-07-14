use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation},
};

use crate::{PtrSize, State};

use super::{BLUE, DARK_GRAY, GREEN, ORANGE, YELLOW, effective_mode, pane_block};

pub const HEXDUMP_WIDTH: usize = 16;

/// One display line of the hexdump: either a real 16-byte row (by row index)
/// or the `*` marker standing in for the rest of a run of zero rows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayRow {
    Row(usize),
    Collapsed,
}

/// Collapse runs of all-zero rows: the first row of a run is kept, the rest
/// become a single `*` marker, like hexyl. Scrolling operates on this list so
/// one scroll step always moves one visual line
fn display_rows(buffer: &[u8]) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    let mut zero_run = 0;
    for (i, chunk) in buffer.chunks(HEXDUMP_WIDTH).enumerate() {
        if chunk.iter().all(|&b| b == 0x00) {
            zero_run += 1;
            match zero_run {
                1 => rows.push(DisplayRow::Row(i)),
                2 => rows.push(DisplayRow::Collapsed),
                _ => {}
            }
        } else {
            zero_run = 0;
            rows.push(DisplayRow::Row(i));
        }
    }
    rows
}

/// Display index of the line showing `row`; rows hidden inside a collapsed
/// run land on the run's `*` marker. Used to jump to an address
pub fn display_index_of_row(buffer: &[u8], row: usize) -> usize {
    let mut index = 0;
    let mut prev_row = 0;
    for (i, display_row) in display_rows(buffer).iter().enumerate() {
        let first_row = match display_row {
            DisplayRow::Row(r) => *r,
            DisplayRow::Collapsed => prev_row + 1,
        };
        if first_row > row {
            break;
        }
        index = i;
        prev_row = first_row;
    }
    index
}

/// Render the given `rows` slice of the hexdump display lines
fn to_hexdump_str<'a>(
    state: &mut State,
    pos: u64,
    buffer: &[u8],
    rows: &[DisplayRow],
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    for display_row in rows {
        let row = match display_row {
            DisplayRow::Collapsed => {
                lines.push(Line::from(Span::styled("*", Style::default().fg(DARK_GRAY))));
                continue;
            }
            DisplayRow::Row(row) => *row,
        };
        let start = row * HEXDUMP_WIDTH;
        let chunk = &buffer[start..(start + HEXDUMP_WIDTH).min(buffer.len())];

        let mut hex_spans = Vec::new();
        // bytes
        for byte in chunk {
            let color = color(*byte);
            hex_spans.push(Span::styled(format!("{byte:02x} "), Style::default().fg(color)));
        }

        // ascii
        hex_spans.push(Span::raw("| "));
        for byte in chunk {
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
        for r in &state.registers {
            if let Some(reg) = &r.register {
                if !reg.is_set() {
                    continue;
                }
                if let Some(reg_value) = &reg.value
                    && let Ok(val) = u64::from_str_radix(&reg_value[2..], 16)
                {
                    for n in 0..=windows {
                        if val as usize == pos as usize + (row * HEXDUMP_WIDTH + n) {
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

        let line = vec![Span::raw(format!("{:08x}: ", row * HEXDUMP_WIDTH)), Span::raw("")]
            .into_iter()
            .chain(hex_spans)
            .chain(ref_spans)
            .collect::<Line>();

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

/// Which popup, if any, to overlay on the hexdump pane
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HexdumpPopup {
    None,
    Save,
    Goto,
}

fn hexdump_block<'a>(state: &State, pos: Option<String>) -> Block<'a> {
    let active = matches!(
        effective_mode(state),
        crate::Mode::OnlyHexdump
            | crate::Mode::OnlyHexdumpPopup
            | crate::Mode::OnlyHexdumpGotoPopup
    );
    pane_block("Hexdump", pos, "S save  : goto  H heap  T stack", active)
}

pub fn draw_hexdump(state: &mut State, f: &mut Frame, hexdump: Rect, popup: HexdumpPopup) {
    let hexdump_active = state.hexdump.is_some();

    if hexdump_active {
        let r = state.hexdump.clone().unwrap();
        let pos = format!("0x{:02x?}", r.0);
        let data = &r.1;

        // account for the top border
        let take = (hexdump.height as usize).saturating_sub(1);
        let rows = display_rows(data);
        state.hexdump_scroll.set_max_scroll(rows.len().saturating_sub(take));
        let skip = state.hexdump_scroll.scroll;
        let visible = &rows[skip.min(rows.len())..(skip + take).min(rows.len())];
        let lines = to_hexdump_str(state, r.0, data, visible);
        let paragraph = Paragraph::new(lines)
            .block(hexdump_block(state, Some(pos)))
            .style(Style::default().fg(Color::White));

        f.render_widget(paragraph, hexdump);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            hexdump,
            &mut state.hexdump_scroll.state,
        );
        if popup != HexdumpPopup::None {
            let (title, value) = match popup {
                HexdumpPopup::Save => ("Save to", state.hexdump_popup.value().to_string()),
                HexdumpPopup::Goto => ("Goto", state.hexdump_goto_popup.value().to_string()),
                HexdumpPopup::None => unreachable!(),
            };
            let area = popup_area(hexdump, 60);
            let txt_input = Paragraph::new(value).style(Style::default()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title.fg(YELLOW))
                    .border_style(Style::default().fg(ORANGE)),
            );
            f.render_widget(Clear, area);
            f.render_widget(txt_input, area);
        }
    } else {
        f.render_widget(Paragraph::new("").block(hexdump_block(state, None)), hexdump);
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

    fn test_state() -> State {
        let args = Args {
            gdb_path: None,
            remote: None,
            ptr_size: PtrSize::Size64,
            cmds: None,
            log_path: None,
        };
        State::new(args)
    }

    #[test]
    fn test_to_hexdump_str_empty() {
        let mut state = test_state();
        let buffer: Vec<u8> = vec![];
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &display_rows(&buffer));
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_to_hexdump_str_single_line() {
        let mut state = test_state();
        let buffer: Vec<u8> = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]; // "Hello"
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &display_rows(&buffer));
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_to_hexdump_str_multiple_lines() {
        let mut state = test_state();
        // Create 32 bytes which should span 2 lines (16 bytes per line)
        let buffer: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &display_rows(&buffer));
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_to_hexdump_str_collapses_zero_runs() {
        let mut state = test_state();
        // 4 rows (64 bytes) of all-zero should collapse to the first row plus a
        // single `*` marker
        let buffer: Vec<u8> = vec![0x00; 64];
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &display_rows(&buffer));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines.last().unwrap().to_string(), "*");

        // A non-zero row between zero runs breaks the collapse: zero row, `*`,
        // data row, zero row => 4 lines
        let mut buffer: Vec<u8> = vec![0x00; 48];
        buffer.extend_from_slice(&[0x41; 16]); // non-zero row
        buffer.extend_from_slice(&[0x00; 16]); // trailing zero row
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &display_rows(&buffer));
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_to_hexdump_str_window() {
        let mut state = test_state();
        // Create 64 bytes which should span 4 lines
        let buffer: Vec<u8> = (0..64).map(|i| i as u8).collect();
        // Skip the first line, take 2 lines
        let rows = display_rows(&buffer);
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &rows[1..3]);
        assert_eq!(lines.len(), 2);
        // addresses follow the rows, not the window position
        assert!(lines[0].to_string().starts_with("00000010: "));
        assert!(lines[1].to_string().starts_with("00000020: "));
    }

    #[test]
    fn test_display_rows_scrolls_one_line_per_step() {
        let mut state = test_state();
        // 2 data rows, a 10-row zero run, 2 data rows: 6 display lines
        let mut buffer: Vec<u8> = (0..32).map(|i| i as u8 + 1).collect();
        buffer.extend(vec![0u8; 160]);
        buffer.extend((0..32).map(|i| i as u8 + 1));
        let rows = display_rows(&buffer);
        assert_eq!(
            rows,
            vec![
                DisplayRow::Row(0),
                DisplayRow::Row(1),
                DisplayRow::Row(2),
                DisplayRow::Collapsed,
                DisplayRow::Row(12),
                DisplayRow::Row(13),
            ]
        );

        // scrolling by one from inside the zero run moves a full visual line
        let first = to_hexdump_str(&mut state, 0x1000, &buffer, &rows[3..5]);
        assert_eq!(first[0].to_string(), "*");
        assert!(first[1].to_string().starts_with("000000c0: "));
    }

    #[test]
    fn test_display_index_of_row() {
        // 2 data rows, a 10-row zero run (rows 2..12), 2 data rows
        let mut buffer: Vec<u8> = (0..32).map(|i| i as u8 + 1).collect();
        buffer.extend(vec![0u8; 160]);
        buffer.extend((0..32).map(|i| i as u8 + 1));

        assert_eq!(display_index_of_row(&buffer, 0), 0);
        assert_eq!(display_index_of_row(&buffer, 2), 2); // first row of the run
        assert_eq!(display_index_of_row(&buffer, 5), 3); // inside the run: `*`
        assert_eq!(display_index_of_row(&buffer, 11), 3);
        assert_eq!(display_index_of_row(&buffer, 12), 4);
        assert_eq!(display_index_of_row(&buffer, 13), 5);
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

    #[test]
    fn test_max_scroll_fills_viewport() {
        // 10 non-zero rows then 10 zero rows: 12 display lines. With a 4-line
        // viewport, max scroll is 8, and rendering from there fills the pane
        // down to the end of the buffer
        let mut state = test_state();
        let mut buffer: Vec<u8> = (0..160).map(|i| (i % 255) as u8 + 1).collect();
        buffer.extend(vec![0u8; 160]);
        let rows = display_rows(&buffer);
        assert_eq!(rows.len(), 12);

        let max_scroll = rows.len().saturating_sub(4);
        let lines = to_hexdump_str(&mut state, 0x1000, &buffer, &rows[max_scroll..]);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines.last().unwrap().to_string(), "*");
    }
}
