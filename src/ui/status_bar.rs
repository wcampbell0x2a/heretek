use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{BLUE, DARK_GRAY, GRAY_FG, GREEN, ORANGE, YELLOW};
use crate::{InputMode, State};

const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Wall-clock based so no tick state is needed; the draw loop already
/// redraws fast enough while gdb is busy
fn spinner_frame() -> &'static str {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    SPINNER_FRAMES[(millis / 120) as usize % SPINNER_FRAMES.len()]
}

pub fn draw_status_bar(state: &State, f: &mut Frame, area: Rect) {
    let mut spans = vec![Span::raw(" ")];

    // execution state
    if state.executing {
        spans.push(Span::styled(format!("{} running", spinner_frame()), Style::new().fg(YELLOW)));
    } else if !state.registers.is_empty() || state.current_pc != 0 {
        spans.push(Span::styled("● stopped", Style::new().fg(GREEN)));
        if let Some(function) = state.bt.first().and_then(|b| b.function.clone()) {
            spans.push(Span::styled(format!(" in {function}"), Style::new().fg(GREEN)));
        }
        if let (Some(file), Some(line)) = (&state.current_source_file, state.current_source_line) {
            let filename = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
            spans.push(Span::styled(format!(" @ {filename}:{line}"), Style::new().fg(GRAY_FG)));
        }
    } else {
        spans.push(Span::styled("○ no program", Style::new().fg(GRAY_FG)));
    }

    // last async result from gdb
    let detail = state.async_result.strip_prefix("Status: ").unwrap_or(&state.async_result);
    if !detail.is_empty() {
        spans.push(Span::styled("  │  ", Style::new().fg(GRAY_FG)));
        spans.push(Span::styled(detail.to_string(), Style::new().fg(ORANGE)));
    }

    // commands in flight that are not execution related (hexdump reads, symbols, ...)
    if !state.executing && (!state.written.is_empty() || !state.next_write.is_empty()) {
        spans.push(Span::styled(
            format!("  {} waiting on gdb", spinner_frame()),
            Style::new().fg(GRAY_FG),
        ));
    }

    let input_hint = match state.input_mode {
        InputMode::Normal => "i input  ",
        InputMode::Editing => "Esc done  ⏎ send  ",
    };
    let right = Line::from(vec![
        Span::styled(input_hint, Style::new().fg(GRAY_FG)),
        Span::styled("? help", Style::new().fg(BLUE).bold()),
        Span::raw(" "),
    ])
    .right_aligned();

    let bg = Style::new().bg(DARK_GRAY);
    f.render_widget(Paragraph::new(Line::from(spans)).style(bg), area);
    f.render_widget(Paragraph::new(right), area);
}
