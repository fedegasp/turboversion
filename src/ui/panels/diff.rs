//
// src/ui/panels/diff.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "Diff": contenuto del diff del commit/file selezionato.

use ratatui::{
    layout::Rect,
    text::Line,
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::Model;
use crate::ui::text::expand_tabs;
use crate::ui::theme;

pub(crate) fn render(f: &mut Frame, model: &Model, area: Rect) {
    let lines: Vec<Line> = model
        .diff
        .iter()
        .map(|l| {
            let color = match l.chars().next() {
                Some('+') => Some(theme::ADDED),
                Some('-') => Some(theme::REMOVED),
                Some('@') => Some(theme::HUNK),
                _ => None,
            };
            let text = expand_tabs(l).into_owned();
            match color {
                Some(c) => Line::styled(text, c),
                None => Line::raw(text),
            }
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines)
            .style(theme::window())
            .wrap(Wrap { trim: false })
            .scroll((model.diff_scroll, 0)),
        area,
    );
}
