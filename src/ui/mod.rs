use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::Cell;
use ratatui::Frame;

use crate::{App, Mode};

pub mod asm;
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

const HEAP_COLOR: Color = GREEN;
const STACK_COLOR: Color = PURPLE;
const TEXT_COLOR: Color = RED;

const SAVED_OUTPUT: usize = 10;

pub fn ui(f: &mut Frame, app: &App) {
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
