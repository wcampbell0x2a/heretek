use asm::draw_asm;
use bt::draw_bt;
use hexdump::draw_hexdump;
use input::draw_input;
use mapping::draw_mapping;
use output::draw_output;
use ratatui::Frame;
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::Layout;
use ratatui::prelude::Stylize;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use registers::draw_registers;
use source::draw_source;
use stack::draw_stack;
use symbols::draw_symbols;
use title::draw_title_area;

use crate::deref::Deref;
use crate::{Mode, State};

pub mod asm;
pub mod bt;
pub mod hexdump;
pub mod input;
pub mod mapping;
pub mod output;
pub mod registers;
pub mod source;
pub mod stack;
pub mod symbols;
pub mod title;

// Ayu bell colors
const BLUE: Color = Color::Rgb(0x59, 0xc2, 0xff);
const PURPLE: Color = Color::Rgb(0xd2, 0xa6, 0xff);
const ORANGE: Color = Color::Rgb(0xff, 0x8f, 0x40);
const YELLOW: Color = Color::Rgb(0xe6, 0xb4, 0x50);
const GREEN: Color = Color::Rgb(0xaa, 0xd9, 0x4c);
const RED: Color = Color::Rgb(0xff, 0x33, 0x33);
const DARK_GRAY: Color = Color::Rgb(0x20, 0x27, 0x34);
const GRAY: Color = Color::Rgb(0x44, 0x44, 0x44);
const GRAY_FG: Color = Color::Rgb(100, 100, 100);

const HEAP_COLOR: Color = GREEN;
const STACK_COLOR: Color = PURPLE;
const TEXT_COLOR: Color = RED;
const STRING_COLOR: Color = YELLOW;
const ASM_COLOR: Color = ORANGE;

const SAVED_OUTPUT: usize = 10;

/// Amount of stack addresses we save/display
pub const SAVED_STACK: u16 = 14;

pub const SCROLL_CONTROL_TEXT: &str = "(up(k), down(j), 50 up(K), 50 down(J), top(g), bottom(G))";

fn draw_mode_content(state: &mut State, f: &mut Frame, top: ratatui::layout::Rect, mode: Mode) {
    match mode {
        Mode::All => {
            if state.registers.is_empty() {
                let vertical = Layout::vertical([10 + 10 + 1 + 11]);
                let [register] = vertical.areas(top);

                draw_registers(state, f, register);
                return;
            }

            let register_size = Min(10);
            let stack_size = Length(10 + 1);
            // 5 previous, 5 now + after
            let asm_size = Length(11);

            // Only show source if we have source information
            if !state.source_lines.is_empty() && state.current_source_line.is_some() {
                let source_size = Fill(1);
                let vertical = Layout::vertical([register_size, stack_size, asm_size, source_size]);
                let [register, stack, asm, source] = vertical.areas(top);

                draw_registers(state, f, register);
                draw_stack(state, f, stack);
                draw_asm(state, f, asm);
                draw_source(state, f, source);
            } else {
                let vertical = Layout::vertical([register_size, stack_size, asm_size]);
                let [register, stack, asm] = vertical.areas(top);

                draw_registers(state, f, register);
                draw_stack(state, f, stack);
                draw_asm(state, f, asm);
            }
        }
        Mode::OnlyRegister => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_registers(state, f, all);
        }
        Mode::OnlyStack => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_stack(state, f, all);
        }
        Mode::OnlyInstructions => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_asm(state, f, all);
        }
        Mode::OnlyMapping => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_mapping(state, f, all);
        }
        Mode::OnlyHexdump => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_hexdump(state, f, all, false);
        }
        Mode::OnlyHexdumpPopup => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_hexdump(state, f, all, true);
        }
        Mode::OnlySymbols => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_symbols(state, f, all);
        }
        Mode::OnlySource => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            draw_source(state, f, all);
        }
        _ => (),
    }
}

pub fn ui(f: &mut Frame, state: &mut State) {
    let (completions, bt_len, mode) = { (state.completions.clone(), state.bt.len(), state.mode) };

    // TODO: register size should depend on arch
    let top_size = Fill(1);

    // If only output, then no top and fill all with output
    if let Mode::OnlyOutput = mode {
        let output_size = Fill(1);
        let completions_len = u16::from(!completions.is_empty());
        let vertical =
            Layout::vertical([Length(2), output_size, Length(3), Length(completions_len)]);
        let [title_area, output, input, completions_area] = vertical.areas(f.area());

        // Add completions if any are found
        let completions = completions.join(" ");
        if completions_area.area() != 0 {
            let completions_str = Paragraph::new(completions);
            f.render_widget(completions_str, completions_area);
        }

        draw_title_area(state, f, title_area);
        draw_output(state, f, output, true);
        draw_input(title_area, state, f, input);
        return;
    }

    // the rest will include the top
    let output_size = Length(SAVED_OUTPUT as u16);

    let top = if bt_len == 0 {
        let completions_len = u16::from(!completions.is_empty());
        let vertical = Layout::vertical([
            Length(2),
            top_size,
            output_size,
            Length(3),
            Length(completions_len),
        ]);
        let [title_area, top, output, input, completions_area] = vertical.areas(f.area());

        // Add completions if any are found
        let completions = completions.join(" ");
        if completions_area.area() != 0 {
            let completions_str = Paragraph::new(completions);
            f.render_widget(completions_str, completions_area);
        }
        draw_title_area(state, f, title_area);
        draw_output(state, f, output, false);
        draw_input(title_area, state, f, input);

        top
    } else {
        let completions_len = u16::from(!completions.is_empty());
        let vertical = Layout::vertical([
            Length(2),
            top_size,
            Length(bt_len as u16 + 1),
            output_size,
            Length(3),
            Length(completions_len),
        ]);
        let [title_area, top, bt_area, output, input, completions_area] = vertical.areas(f.area());

        // Add completions if any are found
        let completions = completions.join(" ");
        if completions_area.area() != 0 {
            let completions_str = Paragraph::new(completions);
            f.render_widget(completions_str, completions_area);
        }
        draw_bt(state, f, bt_area);
        draw_title_area(state, f, title_area);
        draw_output(state, f, output, false);
        draw_input(title_area, state, f, input);

        top
    };

    let display_mode =
        if matches!(mode, Mode::QuitConfirmation) { state.previous_mode } else { mode };

    draw_mode_content(state, f, top, display_mode);

    // Draw quit confirmation popup on top if in quit confirmation mode
    if matches!(mode, Mode::QuitConfirmation) {
        draw_quit_confirmation(f);
    }
}

/// Apply color to val
pub fn apply_val_color(span: &mut Span, is_stack: bool, is_heap: bool, is_text: bool) {
    // TOOD: remove clone
    if is_stack {
        *span = span.clone().style(Style::new().fg(STACK_COLOR));
    } else if is_heap {
        *span = span.clone().style(Style::new().fg(HEAP_COLOR));
    } else if is_text {
        *span = span.clone().style(Style::new().fg(TEXT_COLOR));
    }
}

/// Add deref value to span
pub fn add_deref_to_span(
    deref: &Deref,
    spans: &mut Vec<Span>,
    state: &mut State,
    filepath: &str,
    longest_cells: &mut usize,
    width: usize,
) {
    for (i, v) in deref.map.iter().enumerate() {
        // check if ascii
        if *v > 0xff {
            let bytes = (*v).to_le_bytes();
            if bytes
                .iter()
                .all(|a| a.is_ascii_alphabetic() || a.is_ascii_graphic() || a.is_ascii_whitespace())
            {
                // if we detect it's ascii, the rest is ascii
                let mut full_s = String::new();
                for r in deref.map.iter().skip(i) {
                    let bytes = (*r).to_le_bytes();
                    if let Ok(s) = std::str::from_utf8(&bytes) {
                        full_s.push_str(s);
                    }
                }
                let cell =
                    Span::from(format!("→ \"{full_s}\"")).style(Style::new().fg(STRING_COLOR));
                spans.push(cell);
                return;
            }
        }

        // if not, it's a value
        let hex_string = format!("0x{v:02x}");
        let hex_width = hex_string.len();
        let padding_width = width.saturating_sub(hex_width);
        let mut span =
            Span::from(format!("→ {hex_string}{:padding$}", "", padding = padding_width));
        let (is_stack, is_heap, is_text) = state.classify_val(*v, filepath);
        apply_val_color(&mut span, is_stack, is_heap, is_text);
        spans.push(span);
    }
    if deref.repeated_pattern {
        spans.push(Span::from("→ [loop detected]").style(Style::new().fg(GRAY)));
    }
    if !deref.final_assembly.is_empty() {
        spans.push(
            Span::from(format!("→ {:width$}", deref.final_assembly, width = width))
                .style(Style::new().fg(ASM_COLOR)),
        );
    }
    if spans.len() > *longest_cells {
        *longest_cells = spans.len();
    }
}

fn quit_popup_area(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Flex};
    let vertical = Layout::vertical([Constraint::Length(3)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(60)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

fn draw_quit_confirmation(f: &mut Frame) {
    use ratatui::widgets::{Block, Borders, Clear};
    let area = quit_popup_area(f.area());
    let message =
        Paragraph::new("Are you sure you want to exit? (Enter to confirm, Esc to cancel)")
            .style(Style::default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Quit Confirmation".fg(YELLOW))
                    .border_style(Style::default().fg(ORANGE)),
            );
    f.render_widget(Clear, area);
    f.render_widget(message, area);
}
