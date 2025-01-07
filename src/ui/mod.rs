use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::Frame;

use crate::deref::Deref;
use crate::{App, Mode};

pub mod asm;
pub mod hexdump;
pub mod input;
pub mod mapping;
pub mod output;
pub mod registers;
pub mod stack;
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

const HEAP_COLOR: Color = GREEN;
const STACK_COLOR: Color = PURPLE;
const TEXT_COLOR: Color = RED;

const SAVED_OUTPUT: usize = 10;

/// Amount of stack addresses we save/display
pub const SAVED_STACK: u16 = 14;

pub const SCROLL_CONTROL_TEXT: &str = "(up(k), down(j), 50 up(K), 50 down(J), top(g), bottom(G))";

pub fn ui(f: &mut Frame, app: &mut App) {
    // TODO: register size should depend on arch
    let top_size = Fill(1);

    // If only output, then no top and fill all with output
    if let Mode::OnlyOutput = app.mode {
        let output_size = Fill(1);
        let vertical = Layout::vertical([Length(2), output_size, Length(3)]);
        let [title_area, output, input] = vertical.areas(f.area());

        title::draw_title_area(app, f, title_area);
        output::draw_output(app, f, output, true);
        input::draw_input(title_area, app, f, input);
        return;
    }

    // the rest will include the top
    let output_size = Length(SAVED_OUTPUT as u16);

    let vertical = Layout::vertical([Length(2), top_size, output_size, Length(3)]);
    let [title_area, top, output, input] = vertical.areas(f.area());

    title::draw_title_area(app, f, title_area);
    output::draw_output(app, f, output, false);
    input::draw_input(title_area, app, f, input);

    match app.mode {
        Mode::All => {
            let register_size = Min(10);
            let stack_size = Length(10 + 1);
            // 5 previous, 5 now + after
            let asm_size = Length(11);
            let vertical = Layout::vertical([register_size, stack_size, asm_size]);
            let [register, stack, asm] = vertical.areas(top);

            registers::draw_registers(app, f, register);
            stack::draw_stack(app, f, stack);
            asm::draw_asm(app, f, asm);
        }
        Mode::OnlyRegister => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            registers::draw_registers(app, f, all);
        }
        Mode::OnlyStack => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            stack::draw_stack(app, f, all);
        }
        Mode::OnlyInstructions => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            asm::draw_asm(app, f, all);
        }
        Mode::OnlyMapping => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            mapping::draw_mapping(app, f, all);
        }
        Mode::OnlyHexdump => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            hexdump::draw_hexdump(app, f, all, false);
        }
        Mode::OnlyHexdumpPopup => {
            let vertical = Layout::vertical([Fill(1)]);
            let [all] = vertical.areas(top);
            hexdump::draw_hexdump(app, f, all, true);
        }
        _ => (),
    }
}

/// Apply color to val
pub fn apply_val_color(span: &mut Span, is_stack: bool, is_heap: bool, is_text: bool) {
    // TOOD: remove clone
    if is_stack {
        *span = span.clone().style(Style::new().fg(STACK_COLOR))
    } else if is_heap {
        *span = span.clone().style(Style::new().fg(HEAP_COLOR))
    } else if is_text {
        *span = span.clone().style(Style::new().fg(TEXT_COLOR))
    }
}

/// Add deref value to span
pub fn add_deref_to_span(
    deref: &Deref,
    spans: &mut Vec<Span>,
    app: &App,
    filepath: &str,
    longest_cells: &mut usize,
    width: usize,
) {
    for (i, v) in deref.map.iter().enumerate() {
        // check if ascii if last deref
        if i + 1 == deref.map.len() && *v > 0xff {
            let bytes = (*v).to_le_bytes();
            if bytes
                .iter()
                .all(|a| a.is_ascii_alphabetic() || a.is_ascii_graphic() || a.is_ascii_whitespace())
            {
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    let cell = Span::from(format!("➛ \"{}\"", s)).style(Style::new().fg(YELLOW));
                    spans.push(cell);
                    continue;
                }
            }
        }

        // if not, it's a value
        let hex_string = format!("0x{:02x}", v);
        let hex_width = hex_string.len();
        let padding_width = width.saturating_sub(hex_width);
        let mut span =
            Span::from(format!("➛ {}{:padding$}", hex_string, "", padding = padding_width));
        let (is_stack, is_heap, is_text) = app.classify_val(*v, filepath);
        apply_val_color(&mut span, is_stack, is_heap, is_text);
        spans.push(span);
    }
    if deref.repeated_pattern {
        spans.push(Span::from("➛ [loop detected]").style(Style::new().fg(GRAY)));
    }
    if !deref.final_assembly.is_empty() {
        spans.push(
            Span::from(format!("➛ {:width$}", deref.final_assembly, width = width))
                .style(Style::new().fg(ORANGE)),
        );
    }
    if spans.len() > *longest_cells {
        *longest_cells = spans.len();
    }
}
