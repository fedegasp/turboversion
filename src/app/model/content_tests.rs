//
// src/app/model/content_tests.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Test di `Model::content_mut`, l'unica tabella pannello -> dati: a
//! leggerla sono sia `update`, per muovere il cursore del pannello a
//! fuoco, sia il rendering, per sapere cosa disegnare. Quello che si
//! verifica qui è quindi anche ciò che ogni pannello riceve da disegnare:
//! la propria lista, il proprio cursore, le righe di History così come il
//! `Model` le tiene e lo scorrimento del Diff.

use super::*;
use crate::app::{model_for_tests, model_without_changes_for_tests};

/// Nome del primo elemento della lista di rami di `panel`: se i due
/// pannelli pescassero dalla lista sbagliata, qui comparirebbe un remoto
/// fra i locali (o viceversa).
fn first_branch(model: &mut Model, panel: Panel) -> String {
    match model.content_mut(panel) {
        PanelContent::Branches(list) => list
            .items()
            .first()
            .expect("lista di rami non vuota")
            .name
            .clone(),
        _ => panic!("{panel:?} deve mostrare una lista di rami"),
    }
}

#[test]
fn each_panel_shows_its_own_list() {
    let mut model = model_for_tests();
    assert_eq!(first_branch(&mut model, Panel::Branches), "main");
    assert_eq!(first_branch(&mut model, Panel::Remotes), "origin/main");
    assert!(matches!(
        model.content_mut(Panel::History),
        PanelContent::History(..)
    ));
    assert!(matches!(
        model.content_mut(Panel::Changes),
        PanelContent::Files(..)
    ));
    assert!(matches!(
        model.content_mut(Panel::Diff),
        PanelContent::Diff { .. }
    ));
}

#[test]
fn each_panel_borrows_its_own_cursor() {
    let mut model = model_for_tests();
    model.remote_branches.select(1);

    let PanelContent::Branches(list) = model.content_mut(Panel::Remotes) else {
        panic!("Remotes deve mostrare una lista di rami");
    };
    assert_eq!(list.parts_mut().1.selected(), Some(1));

    let PanelContent::Branches(list) = model.content_mut(Panel::Branches) else {
        panic!("Branches deve mostrare una lista di rami");
    };
    assert_eq!(
        list.parts_mut().1.selected(),
        Some(0),
        "il cursore dei locali"
    );
}

/// Il pannello History riceve le righe così come il `Model` le tiene,
/// riga non committata compresa: nessuna riga viene sintetizzata al
/// momento di disegnare, altrimenti il cursore (che è del `Model`)
/// conterebbe righe diverse da quelle disegnate.
#[test]
fn the_history_panel_receives_the_rows_the_model_holds() {
    let mut model = model_for_tests();
    let expected = model.history().len();
    assert!(
        !model.files.is_empty(),
        "i dati di partenza hanno modifiche"
    );

    let PanelContent::History(list) = model.content_mut(Panel::History) else {
        panic!("History deve mostrare le sue righe");
    };
    let (items, state) = list.parts_mut();
    assert_eq!(items.len(), expected);
    assert!(matches!(items.first(), Some(HistoryEntry::WorkingCopy)));
    assert_eq!(state.selected(), Some(0), "il cursore di History");

    // Con una working copy pulita restano i soli commit, e la prima riga
    // torna a essere un commit.
    let mut model = model_without_changes_for_tests();
    let PanelContent::History(list) = model.content_mut(Panel::History) else {
        panic!("History deve mostrare le sue righe");
    };
    let items = list.items();
    assert_eq!(items.len(), expected - 1);
    assert!(matches!(items.first(), Some(HistoryEntry::Commit(_))));
}

#[test]
fn the_diff_panel_carries_its_scroll_position() {
    let mut model = model_for_tests();
    model.diff_scroll = 2;
    let PanelContent::Diff { lines, scroll, .. } = model.content_mut(Panel::Diff) else {
        panic!("Diff deve mostrare righe di testo");
    };
    assert_eq!(*scroll, 2);
    assert!(!lines.is_empty());
}
