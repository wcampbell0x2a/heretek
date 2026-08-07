use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{
    ASM_COLOR, BLUE, GRAY_FG, GREEN, HEAP_COLOR, ORANGE, STACK_COLOR, STRING_COLOR, TEXT_COLOR,
    YELLOW,
};

fn header(text: &str) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), Style::new().fg(BLUE).bold()))
}

fn entry(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key:<9}"), Style::new().fg(GREEN)),
        Span::raw(desc.to_string()),
    ])
}

fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Max(height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Max(width)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn draw_help(f: &mut Frame) {
    let left = vec![
        header("Global"),
        entry("F1-F9", "switch pane"),
        entry("Tab", "next pane"),
        entry("i", "command input"),
        entry("Ctrl+C", "interrupt gdb"),
        entry("q", "quit"),
        entry("?", "toggle this help"),
        Line::default(),
        header("Scrolling"),
        entry("j / k", "down / up"),
        entry("J / K", "down / up 50"),
        entry("g / G", "top / bottom"),
        Line::default(),
        header("Colors"),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Heap ", Style::new().fg(HEAP_COLOR).bold()),
            Span::styled("Stack ", Style::new().fg(STACK_COLOR).bold()),
            Span::styled("Code ", Style::new().fg(TEXT_COLOR).bold()),
            Span::styled("String ", Style::new().fg(STRING_COLOR).bold()),
            Span::styled("Asm", Style::new().fg(ASM_COLOR).bold()),
        ]),
    ];

    let right = vec![
        header("Mapping"),
        entry("H", "hexdump region"),
        Line::default(),
        header("Hexdump"),
        entry("S", "save to file"),
        entry("H", "goto heap"),
        entry("T", "goto stack"),
        Line::default(),
        header("Symbols"),
        entry("/", "search (fuzzy)"),
        entry("r", "refresh"),
        entry("⏎", "disassemble"),
        entry("Esc", "back"),
        Line::default(),
        header("Input"),
        entry("⏎", "send command"),
        entry("↑ / ↓", "command history"),
        entry("Tab", "complete"),
    ];

    let height = left.len().max(right.len()) as u16 + 2;
    let area = popup_area(f.area(), 66, height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(ORANGE))
        .title(Span::styled("Help", Style::new().fg(YELLOW).bold()))
        .title_bottom(
            Line::from(Span::styled("Esc/q/? close", Style::new().fg(GRAY_FG))).right_aligned(),
        );
    let inner = block.inner(area);

    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
    let [left_area, right_area] = columns.areas(inner);
    f.render_widget(Paragraph::new(left), left_area);
    f.render_widget(Paragraph::new(right), right_area);
}
