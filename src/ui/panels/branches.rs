//
// src/ui/panels/branches.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "Branches": lista dei rami, con `*` sul ramo corrente.

use ratatui::{layout::Rect, style::Style, widgets::ListItem, Frame};

use crate::app::Model;
use crate::ui::text::fit;

pub(crate) fn render(f: &mut Frame, model: &mut Model, area: Rect, selection: Style) {
    let width = area.width as usize;
    let items: Vec<ListItem> = model
        .branch_list
        .iter()
        .map(|b| {
            let mark = if b.current { '*' } else { ' ' };
            ListItem::new(fit(&format!("{mark} {}", b.name), width))
        })
        .collect();
    super::render_list(f, items, area, selection, &mut model.branches);
}
