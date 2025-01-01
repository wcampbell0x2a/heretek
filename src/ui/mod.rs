use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::Cell;
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

pub const SCROLL_CONTROL_TEXT: &str = "(up(k), down(j), 50 up(K), 50 down(J), top(g))";

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
            let register_size = Min(30);
            let stack_size = Min(10);
            let asm_size = Min(15);
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
pub fn apply_val_color(cell: &mut Cell, is_stack: bool, is_heap: bool, is_text: bool) {
    // TOOD: remove clone
    if is_stack {
        *cell = cell.clone().style(Style::new().fg(STACK_COLOR))
    } else if is_heap {
        *cell = cell.clone().style(Style::new().fg(HEAP_COLOR))
    } else if is_text {
        *cell = cell.clone().style(Style::new().fg(TEXT_COLOR))
    }
}

/// Add deref value to cells
pub fn add_deref_to_cell(
    deref: &Deref,
    cells: &mut Vec<Cell>,
    app: &App,
    filepath: &str,
    longest_cells: &mut usize,
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
                    let cell = Cell::from(format!("➛ \"{}\"", s)).style(Style::new().fg(YELLOW));
                    cells.push(cell);
                    continue;
                }
            }
        }

        // if not, it's a value
        let mut cell = Cell::from(format!("➛ 0x{:02x}", v));
        let (is_stack, is_heap, is_text) = app.classify_val(*v, filepath);
        apply_val_color(&mut cell, is_stack, is_heap, is_text);
        cells.push(cell);
    }
    if deref.repeated_pattern {
        cells.push(Cell::from("➛ [loop detected]").style(Style::new().fg(GRAY)));
    }
    if !deref.final_assembly.is_empty() {
        cells
            .push(Cell::from(format!("➛ {}", deref.final_assembly)).style(Style::new().fg(ORANGE)));
    }
    if cells.len() > *longest_cells {
        *longest_cells = cells.len();
    }
}
