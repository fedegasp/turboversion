//
// src/ui/panels/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Rendering dei singoli pannelli: ciascun modulo riceve per parametro i
//! dati da disegnare — scelti da `ui::view_model`, che li prende dal
//! `Model` — e decide solo come appaiono: colori, troncamenti, larghezza
//! delle colonne. Nessuno di essi legge il `Model` né chiama `git/`: così
//! `branches` può servire due pannelli, `history` una lista di righe che
//! non sono tutte commit, e nessun pannello può modificare lo stato
//! dell'applicazione mentre disegna.

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
