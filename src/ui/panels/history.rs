//
// src/ui/panels/history.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "History": log dei commit con grafo, hash, messaggio e autore.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::ListItem,
    Frame,
};

use crate::app::Model;
use crate::ui::text::fit;
use crate::ui::theme;

const GRAPH_W: usize = 4;
const HASH_W: usize = 8;
const AUTHOR_W: usize = 12;

pub(crate) fn render(f: &mut Frame, model: &mut Model, area: Rect, selection: Style) {
    let avail = (area.width as usize).saturating_sub(GRAPH_W + HASH_W);
    let author_w = AUTHOR_W.min(avail);
    let msg_w = avail.saturating_sub(author_w);
    let items: Vec<ListItem> = model
        .commits
        .iter()
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::styled(fit(&c.graph, GRAPH_W), theme::GRAPH),
                Span::styled(fit(&format!("{} ", c.hash), HASH_W), theme::HASH),
                Span::raw(fit(&c.msg, msg_w)),
                Span::styled(fit(&c.author, author_w), theme::DIM),
            ]))
        })
        .collect();
    super::render_list(f, items, area, selection, &mut model.history);
}
