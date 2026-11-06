//
// src/ui/hints.rs
// TurboVersion
//
// 2026-11-04 Federico Gasperini
//

//! Le scorciatoie da mostrare nella barra di stato: non sono un campo del
//! `Model` ma una funzione del suo stato (quale overlay è aperto, quale
//! pannello ha il focus), quindi vanno calcolate a ogni frame.
//!
//! Qui vivono solo funzioni pure: niente formattazione (colori,
//! troncamenti, larghezze restano in `ui::panels`), niente accesso a git,
//! nessuno stato proprio.
//!
//! Il file si chiamava `view_model.rs` e adattava anche le righe dei
//! pannelli; quelle non passano più da qui. Le consegnava una
//! traduzione da `PanelContent` (la tabella pannello -> dati del `Model`) a
//! un enum parallelo, `Rows`, che esisteva per una ragione sola: ridurre
//! ciò che il rendering poteva fare, consegnando `(&[T], &mut ListState)`
//! invece di un `&mut ListPanel<T>`. Quella riduzione sta ora nel tipo
//! stesso: i metodi che cambiano lista e selezione sono visibili solo
//! dentro `crate::app`, e `ListPanel` non si può nemmeno ricostruire da
//! fuori (nessun `From`, nessun `Default`). L'enum parallelo non comprava
//! quindi più nulla, e `ui::draw_panel` fa ora il match direttamente su
//! `PanelContent`.

use std::borrow::Cow;

use crate::app::{Model, Panel};

/// Le scorciatoie da mostrare nella barra di stato.
pub(crate) type Hints = Cow<'static, [(&'static str, &'static str)]>;

/// Scorciatoie attive mentre un menu è aperto. Sta qui e non in
/// `panels::menubar` perché è una tabella di tasti, cioè un dato che
/// dipende dallo stato dell'app, non dalla resa del menu; a leggerla è solo
/// `active` qui sotto.
const MENU_KEYS: &[(&str, &str)] = &[
    ("←→", "Switch"),
    ("↑↓", "Select"),
    ("Enter", "Activate"),
    ("Esc", "Close"),
];

/// Avviso in testa alle scorciatoie quando le operazioni git non sono più
/// possibili. Non è una scorciatoia, ma la barra di stato è l'unico posto
/// che lo ricorda dopo che il dialog d'annuncio è stato chiuso: i pannelli
/// continuano a mostrare dati validi e nulla, altrimenti, direbbe che non
/// si aggiornano più.
///
/// In testa e non in coda perché la barra di stato viene tagliata a
/// destra: in coda sarebbe la prima cosa a sparire su un terminale
/// stretto, cioè proprio dove serve di più.
const GIT_UNAVAILABLE: (&str, &str) = ("!", "Git unavailable");

/// Scorciatoie del pannello principale; `Space Stage` compare solo quando
/// il focus è sul pannello Changes, dove quel tasto ha davvero effetto.
fn main_hints(model: &Model) -> Vec<(&'static str, &'static str)> {
    let mut hints = Vec::new();
    if !model.git_available() {
        hints.push(GIT_UNAVAILABLE);
    }
    hints.extend([("Tab", "Panel"), ("↑↓", "Move")]);
    if model.focus == Panel::Changes {
        hints.push(("Space", "Stage"));
    }
    hints.push(("F10", "Menu"));
    hints.push(("Alt-X", "Quit"));
    hints
}

/// Le scorciatoie attive: quelle dell'overlay in cima, se ce n'è uno, o
/// quelle del pannello a fuoco.
pub(crate) fn active(model: &Model) -> Hints {
    match (model.dialog(), model.menu()) {
        (Some(d), _) => Cow::Borrowed(d.hints()),
        (None, Some(_)) => Cow::Borrowed(MENU_KEYS),
        (None, None) => Cow::Owned(main_hints(model)),
    }
}
