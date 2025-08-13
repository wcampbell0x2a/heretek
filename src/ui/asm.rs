use ansi_to_tui::IntoText;
use bat::PrettyPrinter;
use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Cell, Table, TableState};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Row};

use super::{GREEN, ORANGE, PURPLE};

use crate::State;

pub fn draw_asm(state: &mut State, f: &mut Frame, asm: Rect) {
    // Asm
    // TODO: cache the pc_index if this doesn't change
    let mut rows = vec![];
    let mut pc_index = None;
    let mut function_name = None;
    let mut tallest_function_len = 0;

    // Display asm, this will already be in a sorted order
    for (index, a) in state.asm.iter().enumerate() {
        if a.address == state.current_pc {
            pc_index = Some(index);
            if let Some(func_name) = &a.func_name {
                function_name = Some(func_name.clone());
                if func_name.len() > tallest_function_len {
                    tallest_function_len = func_name.len();
                }
            }
        }
        let addr_cell =
            Cell::from(format!("0x{:02x}", a.address)).style(Style::default().fg(PURPLE));
        let mut row = vec![addr_cell];

        if let Some(function_name) = &a.func_name {
            let function_cell = Cell::from(format!("{function_name}+{:02x}", a.offset))
                .style(Style::default().fg(PURPLE));
            row.push(function_cell);
        } else {
            row.push(Cell::from(""));
        }

        let inst_cell = if let Some(pc_index) = pc_index {
            if pc_index == index {
                Cell::from(a.inst.to_string()).fg(GREEN)
            } else {
                let mut bytes = String::new();
                PrettyPrinter::new()
                    .input_from_bytes(a.inst.as_bytes())
                    .language("ARM Assembly")
                    .print_with_writer(Some(&mut bytes))
                    .unwrap();
                Cell::from(bytes.into_text().unwrap()).white()
                // Cell::from(a.inst.to_string()).white()
            }
        } else {
            let mut bytes = String::new();
            PrettyPrinter::new()
                .input_from_bytes(a.inst.as_bytes())
                .language("ARM Assembly")
                .print_with_writer(Some(&mut bytes))
                .unwrap();
            Cell::from(bytes.into_text().unwrap())
        };
        row.push(inst_cell);

        rows.push(Row::new(row));
    }

    let tital = if let Some(function_name) = function_name {
        Title::from(format!("Instructions ({function_name})").fg(ORANGE))
    } else {
        Title::from("Instructions".fg(ORANGE))
    };
    if let Some(pc_index) = pc_index {
        let widths = [
            Constraint::Length(16),
            Constraint::Length(tallest_function_len as u16 + 5),
            Constraint::Fill(1),
        ];
        let table = Table::new(rows, widths)
            .block(Block::default().borders(Borders::TOP).title(tital))
            .row_highlight_style(Style::new().fg(GREEN))
            .highlight_symbol(">>");
        let start_offset = pc_index.saturating_sub(5);
        let mut table_state =
            TableState::default().with_offset(start_offset).with_selected(pc_index);
        f.render_stateful_widget(table, asm, &mut table_state);
    } else {
        let block = Block::default().borders(Borders::TOP).title(tital);
        f.render_widget(block, asm);
    }
}
