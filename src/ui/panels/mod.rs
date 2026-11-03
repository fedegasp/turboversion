//
// src/ui/panels/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Rendering dei singoli pannelli: ciascun modulo legge il `Model` e
//! disegna, senza mai chiamare `git/`.

pub(crate) mod branches;
pub(crate) mod changes;
pub(crate) mod dialog;
pub(crate) mod diff;
pub(crate) mod history;
pub(crate) mod menubar;
pub(crate) mod statusbar;

use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{List, ListItem, ListState},
    Frame,
};

use crate::ui::theme;

/// Renderizza una lista con lo stile delle finestre e l'evidenziazione
/// della selezione: condiviso da branches/history/changes.
pub(crate) fn render_list(
    f: &mut Frame,
    items: Vec<ListItem>,
    area: Rect,
    selection: Style,
    state: &mut ListState,
) {
    f.render_stateful_widget(
        List::new(items)
            .style(theme::window())
            .highlight_style(selection),
        area,
        state,
    );
}
