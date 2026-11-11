//
// src/ui/panels/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Rendering dei singoli pannelli: ciascun modulo riceve per parametro i
//! dati da disegnare — presi dal `Model` da `ui::draw_panel`, che li chiede
//! a `Model::content_mut` — e decide solo come appaiono: colori,
//! troncamenti, larghezza delle colonne. Nessuno di essi legge il `Model` né chiama `git/`: così
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
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
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

/// Una riga di testo al posto del contenuto di un pannello, per dire perché
/// non c'è nulla da elencare o da scorrere: condivisa da Changes e Diff. Va
/// a capo: un messaggio d'errore del backend può essere più largo del
/// pannello, e troncato non si leggerebbe.
///
/// I testi che le arrivano usano solo caratteri a larghezza certa (tre
/// punti ASCII invece dell'ellissi Unicode, che è "East Asian ambiguous" e
/// su alcuni terminali occupa due celle).
pub(crate) fn render_notice_text(f: &mut Frame, text: &str, area: Rect) {
    f.render_widget(
        Paragraph::new(text)
            .style(theme::window().fg(theme::DIM))
            .wrap(Wrap { trim: true }),
        area,
    );
}
