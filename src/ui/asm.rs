use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Cell, Table, TableState};
use ratatui::{layout::Rect, style::Style, widgets::Row, Frame};

use super::{GREEN, ORANGE, PURPLE};

use crate::App;

pub fn draw_asm(app: &App, f: &mut Frame, asm: Rect) {
    // Asm
    // TODO: cache the pc_index if this doesn't change
    let mut rows = vec![];
    let mut pc_index = None;
    let mut function_name = None;
    if let Ok(asm) = app.asm.lock() {
        let mut entries: Vec<_> = asm.clone().into_iter().collect();
        entries.sort_by(|a, b| a.address.cmp(&b.address));
        let mut index = 0;
        let app_cur_lock = app.current_pc.lock().unwrap();
        for a in entries.iter() {
            if a.address == *app_cur_lock {
                pc_index = Some(index);
                if let Some(func_name) = &a.func_name {
                    function_name = Some(func_name.clone());
                }
            }
            let addr_cell =
                Cell::from(format!("0x{:02x}", a.address)).style(Style::default().fg(PURPLE));
            let mut row = vec![addr_cell];

            if let Some(function_name) = &a.func_name {
                let function_cell = Cell::from(format!("{}+{:02x}", function_name, a.offset))
                    .style(Style::default().fg(PURPLE));
                row.push(function_cell);
            } else {
                row.push(Cell::from(""));
            }

            let inst_cell = if let Some(pc_index) = pc_index {
                if pc_index == index {
                    Cell::from(a.inst.to_string()).fg(GREEN)
                } else {
                    Cell::from(a.inst.to_string()).white()
                }
            } else {
                Cell::from(a.inst.to_string()).dark_gray()
            };
            row.push(inst_cell);

            rows.push(Row::new(row));
            index += 1;
        }
    }

    let tital = if let Some(function_name) = function_name {
        Title::from(format!("Instructions ({})", function_name).fg(ORANGE))
    } else {
        Title::from("Instructions".fg(ORANGE))
    };
    if let Some(pc_index) = pc_index {
        let widths = [Constraint::Length(16), Constraint::Percentage(10), Constraint::Fill(1)];
        let table = Table::new(rows, widths)
            .block(Block::default().borders(Borders::TOP).title(tital))
            .row_highlight_style(Style::new().fg(GREEN))
            .highlight_symbol(">>");
        let start_offset = if pc_index < 5 { 0 } else { pc_index - 5 };
        let mut table_state =
            TableState::default().with_offset(start_offset).with_selected(pc_index);
        f.render_stateful_widget(table, asm, &mut table_state);
    } else {
        let block = Block::default().borders(Borders::TOP).title(tital);
        f.render_widget(block, asm);
    }
}
