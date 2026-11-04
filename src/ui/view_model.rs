//
// src/ui/view_model.rs
// TurboVersion
//
// 2026-11-04 Federico Gasperini
//

//! Adattamento `Model` -> dati da disegnare.
//!
//! Il `Model` è organizzato per *stato dell'applicazione*; un pannello
//! mostra ciò che serve all'utente, e le due forme non coincidono sempre.
//! Qui vivono le traduzioni fra l'una e l'altra, come funzioni pure: niente
//! formattazione (colori, troncamenti, checkbox e larghezze delle colonne
//! restano in `ui::panels`), niente accesso a git, nessuno stato proprio.
//! Questo modulo dice *quali dati* mostra un pannello; i pannelli dicono
//! *come appaiono*.
//!
//! Il modulo esiste per due ragioni, nessuna delle quali è "qui i tipi
//! reggeranno senza modifiche": `Rows` andrà riscritto appena un pannello
//! mostrerà qualcosa che non è una riga del `Model` (la riga dello stato
//! non committato in cima a History non è un `Commit`, e nessuno `&[Commit]`
//! può contenerla).
//!
//! 1. **Riduzione di capacità.** A `ui/` arrivano `(&[T], &mut ListState)`,
//!    non `&mut ListPanel<T>`: il rendering può leggere gli elementi e
//!    muovere il cursore, ma non ha modo di chiamare `set`, `retain` o
//!    `push_selected`. "Solo `update` muta il Model" smette di essere una
//!    disciplina da ricordare e diventa una proprietà dei tipi.
//! 2. **Un tipo view-facing separato da quello update-facing.** Quando il
//!    pannello History dovrà mostrare anche righe sintetiche, a cambiare
//!    saranno la variante di `Rows` e `panels::history`; `PanelContent`,
//!    che serve a `update` e non ha bisogno di quelle righe, resterà
//!    com'è. Senza questo strato la stessa modifica dovrebbe passare dal
//!    `Model`.
//!
//! I `ListState` restano del `Model` e vengono presi in prestito: ratatui
//! ci tiene anche l'offset di scorrimento, calcolato mentre disegna, che
//! deve sopravvivere da un frame al successivo. Un modello parallelo
//! ricostruito a ogni frame lo perderebbe.

use std::borrow::Cow;

use ratatui::widgets::ListState;

use crate::app::{Model, Panel, PanelContent};
use crate::git::domain::{Branch, Commit, FileEntry};

/// Ciò che un pannello deve disegnare: i dati già tipizzati, più il
/// cursore con cui scorrerli.
pub(crate) enum Rows<'a> {
    Branches(&'a [Branch], &'a mut ListState),
    Commits(&'a [Commit], &'a mut ListState),
    Files(&'a [FileEntry], &'a mut ListState),
    /// Lo scorrimento del diff arriva per valore, non come `&mut` come i
    /// `ListState` delle liste: quelli ratatui li aggiorna mentre disegna
    /// (ci tiene l'offset), mentre lo scorrimento del diff lo muove solo
    /// `update`, e il renderer non deve poterlo cambiare.
    Diff(&'a [String], u16),
}

/// Le scorciatoie da mostrare nella barra di stato.
pub(crate) type Hints = Cow<'static, [(&'static str, &'static str)]>;

/// Scorciatoie attive mentre un menu è aperto. Sta qui e non in
/// `panels::menubar` perché è una tabella di tasti, cioè un dato che
/// dipende dallo stato dell'app, non dalla resa del menu; a leggerla è solo
/// `hints` qui sotto.
const MENU_KEYS: &[(&str, &str)] = &[
    ("←→", "Switch"),
    ("↑↓", "Select"),
    ("Enter", "Activate"),
    ("Esc", "Close"),
];

/// Scorciatoie del pannello principale; `Space Stage` compare solo quando
/// il focus è sul pannello Changes, dove quel tasto ha davvero effetto.
fn main_hints(focus: Panel) -> Vec<(&'static str, &'static str)> {
    let mut hints = vec![("Tab", "Panel"), ("↑↓", "Move")];
    if focus == Panel::Changes {
        hints.push(("Space", "Stage"));
    }
    hints.push(("F10", "Menu"));
    hints.push(("Alt-X", "Quit"));
    hints
}

/// Le scorciatoie attive: quelle dell'overlay in cima, se ce n'è uno, o
/// quelle del pannello a fuoco.
pub(crate) fn hints(model: &Model) -> Hints {
    match (model.dialog(), model.menu()) {
        (Some(d), _) => Cow::Borrowed(d.hints()),
        (None, Some(_)) => Cow::Borrowed(MENU_KEYS),
        (None, None) => Cow::Owned(main_hints(model.focus)),
    }
}

/// Le righe del pannello `panel`. Si appoggia a `Model::content_mut`
/// invece di ripetere il match su `Panel`: la corrispondenza fra pannello e
/// dati è scritta una volta sola, dal lato del `Model`, e vale sia per il
/// rendering sia per il movimento del cursore in `app::update`.
pub(crate) fn rows(model: &mut Model, panel: Panel) -> Rows<'_> {
    match model.content_mut(panel) {
        PanelContent::Branches(list) => {
            let (items, state) = list.parts_mut();
            Rows::Branches(items, state)
        }
        PanelContent::Commits(list) => {
            let (items, state) = list.parts_mut();
            Rows::Commits(items, state)
        }
        PanelContent::Files(list) => {
            let (items, state) = list.parts_mut();
            Rows::Files(items, state)
        }
        // Il limite dello scorrimento (`viewport`) serve solo a `update`:
        // al renderer basta sapere da quale riga partire.
        PanelContent::Diff { lines, scroll, .. } => Rows::Diff(lines, *scroll),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model_for_tests;

    /// Nome del primo elemento disegnato da ciascun pannello di rami: se i
    /// due pannelli pescassero dalla lista sbagliata, qui comparirebbe un
    /// remoto fra i locali (o viceversa).
    fn first_branch(model: &mut Model, panel: Panel) -> String {
        match rows(model, panel) {
            Rows::Branches(items, _) => {
                items.first().expect("lista di rami non vuota").name.clone()
            }
            _ => panic!("{panel:?} deve mostrare una lista di rami"),
        }
    }

    #[test]
    fn each_panel_shows_its_own_list() {
        let mut model = model_for_tests();
        assert_eq!(first_branch(&mut model, Panel::Branches), "main");
        assert_eq!(first_branch(&mut model, Panel::Remotes), "origin/main");
        assert!(matches!(
            rows(&mut model, Panel::History),
            Rows::Commits(..)
        ));
        assert!(matches!(rows(&mut model, Panel::Changes), Rows::Files(..)));
        assert!(matches!(rows(&mut model, Panel::Diff), Rows::Diff(..)));
    }

    #[test]
    fn each_panel_borrows_its_own_cursor() {
        let mut model = model_for_tests();
        model.remote_branches.select(1);

        let Rows::Branches(_, state) = rows(&mut model, Panel::Remotes) else {
            panic!("Remotes deve mostrare una lista di rami");
        };
        assert_eq!(state.selected(), Some(1));

        let Rows::Branches(_, state) = rows(&mut model, Panel::Branches) else {
            panic!("Branches deve mostrare una lista di rami");
        };
        assert_eq!(state.selected(), Some(0), "il cursore dei locali");
    }

    #[test]
    fn the_diff_panel_carries_its_scroll_position() {
        let mut model = model_for_tests();
        model.diff_scroll = 2;
        let Rows::Diff(lines, scroll) = rows(&mut model, Panel::Diff) else {
            panic!("Diff deve mostrare righe di testo");
        };
        assert_eq!(scroll, 2);
        assert!(!lines.is_empty());
    }
}
