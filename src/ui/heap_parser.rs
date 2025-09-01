use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation},
};

use crate::State;
use cogitator::MallocChunk;

use super::{GREEN, ORANGE, PURPLE, SCROLL_CONTROL_TEXT, YELLOW};

fn format_heap_chunks(chunks: &[MallocChunk], skip: usize, take: usize) -> Vec<Line> {
    let mut lines = Vec::new();

    for (i, chunk) in chunks.iter().skip(skip).take(take).enumerate() {
        let actual_index = skip + i;
        let is_last = actual_index == chunks.len() - 1;
        
        // Check if this chunk is free by looking at next chunk's PREV_INUSE bit
        // This matches the logic in print_heap
        let is_free = if is_last {
            false
        } else if let Some(next_chunk) = chunks.get(actual_index + 1) {
            (next_chunk.size & 0x1) == 0  // Next chunk doesn't have PREV_INUSE set
        } else {
            chunk.fd.is_some() && chunk.bk.is_some()
        };
        
        let chunk_type = if is_last {
            "Top chunk"
        } else if is_free {
            if (chunk.size & !0x7) >= 0x400 {
                "Free chunk (unsortedbin)"
            } else {
                "Free chunk"
            }
        } else {
            "Allocated chunk"
        };
        let size_without_flags = chunk.size & !0x7;

        // Header line with chunk type and flags
        let mut header_spans = Vec::new();
        header_spans.push(Span::styled(
            format!("{} | ", chunk_type),
            Style::default().fg(if chunk_type.contains("Allocated") {
                GREEN
            } else if chunk_type.contains("Free") {
                YELLOW
            } else {
                PURPLE // Top chunk
            }),
        ));

        if (chunk.size & 0x1) != 0 {
            header_spans.push(Span::styled("PREV_INUSE ", Style::default().fg(GREEN)));
        }
        if (chunk.size & 0x2) != 0 {
            header_spans.push(Span::styled("IS_MMAPPED ", Style::default().fg(YELLOW)));
        }
        if (chunk.size & 0x4) != 0 {
            header_spans.push(Span::styled("NON_MAIN_ARENA ", Style::default().fg(ORANGE)));
        }

        lines.push(Line::from(header_spans));

        // Address line
        lines.push(Line::from(vec![
            Span::raw("Addr: "),
            Span::styled(format!("0x{:x}", chunk.address), Style::default().fg(ORANGE)),
        ]));

        // Size line
        lines.push(Line::from(vec![
            Span::raw("Size: "),
            Span::styled(format!("0x{:x}", size_without_flags), Style::default().fg(GREEN)),
            Span::raw(" (with flag bits: "),
            Span::styled(format!("0x{:x}", chunk.size), Style::default().fg(YELLOW)),
            Span::raw(")"),
        ]));

        // Empty line for spacing
        lines.push(Line::from(""));
    }

    lines
}

fn block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(format!("Heap Parser {SCROLL_CONTROL_TEXT}, Parse(P))").fg(ORANGE))
}

pub fn draw_heap_parser(state: &mut State, f: &mut Frame, area: Rect) {
    if state.heap_chunks.is_empty() {
        let paragraph = Paragraph::new("No heap chunks parsed yet. Press 'P' to parse heap.")
            .block(block())
            .style(Style::default().fg(Color::White));
        f.render_widget(paragraph, area);
        return;
    }

    let skip = state.heap_parser_scroll.scroll;
    let take = area.height as usize;
    let lines = format_heap_chunks(&state.heap_chunks, skip, take);
    let content_len = state.heap_chunks.len();

    state.heap_parser_scroll.state = state.heap_parser_scroll.state.content_length(content_len);
    let paragraph = Paragraph::new(lines).block(block()).style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut state.heap_parser_scroll.state,
    );
}
