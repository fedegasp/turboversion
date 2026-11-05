//
// src/app/model/history_tests.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Test dell'invariante di History. Stanno sotto `model` perché leggono i
//! campi privati `commits` e `history`, visibili solo a questo modulo e ai
//! suoi figli.

use super::*;
use crate::app::tests::{press, TestApp};
use crate::app::update;
use crate::msg::Msg;
use ratatui::crossterm::event::KeyCode;

/// Un commit sintetico: i test su History guardano la forma della
/// lista, non il contenuto dei singoli commit.
fn commit(hash: &str) -> Commit {
    Commit {
        graph: "*".to_string(),
        hash: hash.to_string(),
        msg: format!("commit {hash}"),
        author: "Tester".to_string(),
    }
}

fn modified_file(path: &str) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        status: crate::git::domain::FileStatus::Modified,
        staged: false,
    }
}

/// Rimpiazza i dati caricati, come farebbe l'esito di `Cmd::LoadAll`.
fn load(app: &mut TestApp, commits: Vec<Commit>, files: Vec<FileEntry>) {
    update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits,
            files,
            diff: Vec::new(),
        },
    );
}

fn history_hashes(app: &TestApp) -> Vec<&str> {
    app.history
        .items()
        .iter()
        .filter_map(|e| match e {
            HistoryEntry::WorkingCopy => None,
            HistoryEntry::Commit(c) => Some(c.hash.as_str()),
        })
        .collect()
}

#[test]
fn history_shows_the_uncommitted_row_only_when_the_working_copy_has_changes() {
    let mut app = TestApp::new();
    // I dati di partenza hanno file modificati.
    assert!(matches!(
        app.history.items().first(),
        Some(HistoryEntry::WorkingCopy)
    ));
    assert_eq!(app.history.len(), app.commits.len() + 1);

    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222")],
        Vec::new(),
    );
    assert_eq!(history_hashes(&app), ["aaa1111", "bbb2222"]);
    assert!(
        !app.history
            .items()
            .iter()
            .any(|e| matches!(e, HistoryEntry::WorkingCopy)),
        "senza modifiche la riga non deve esistere, nemmeno vuota"
    );

    load(
        &mut app,
        vec![commit("aaa1111")],
        vec![modified_file("src/main.rs")],
    );
    assert!(matches!(
        app.history.items().first(),
        Some(HistoryEntry::WorkingCopy)
    ));
    assert_eq!(history_hashes(&app), ["aaa1111"]);

    // Modifiche in un repository senza commit: la riga è tutta la lista.
    load(&mut app, Vec::new(), vec![modified_file("src/main.rs")]);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history.selected_index(), Some(0));
}

/// Il cursore percorre la riga non committata e i commit come un'unica
/// lista: la selezione di History non è un indice in `Model::commits`.
#[test]
fn the_history_cursor_walks_the_uncommitted_row_and_the_commits_together() {
    let mut app = TestApp::new();
    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222")],
        vec![modified_file("src/main.rs")],
    );
    app.focus = Panel::History;

    assert!(matches!(
        app.history.selected(),
        Some(HistoryEntry::WorkingCopy)
    ));

    press(&mut app, KeyCode::Down);
    assert!(matches!(app.history.selected(), Some(HistoryEntry::Commit(c)) if c.hash == "aaa1111"));
    press(&mut app, KeyCode::Down);
    assert!(matches!(app.history.selected(), Some(HistoryEntry::Commit(c)) if c.hash == "bbb2222"));
    press(&mut app, KeyCode::Down);
    assert_eq!(app.history.selected_index(), Some(2), "ultima riga");

    for _ in 0..5 {
        press(&mut app, KeyCode::Up);
    }
    assert!(matches!(
        app.history.selected(),
        Some(HistoryEntry::WorkingCopy)
    ));
}

/// Ricaricare sostituisce le righe di History e riporta il cursore in
/// cima, anche se era su un commit: le righe sono nuove e quella
/// selezionata può non esserci più. Chi vorrà ritrovare la selezione
/// dovrà ancorarla all'identità della riga, non all'indice.
#[test]
fn reloading_brings_the_history_cursor_back_to_the_top() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.history.selected_index(), Some(2));

    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222")],
        vec![modified_file("src/main.rs")],
    );
    assert_eq!(app.history.selected_index(), Some(0));
    assert!(matches!(
        app.history.selected(),
        Some(HistoryEntry::WorkingCopy)
    ));
}

/// La riga in cima dipende dalla working copy letta al caricamento,
/// non dal contenuto del pannello Changes: svuotare quel pannello non
/// deve far sparire la riga, né riempirlo farla comparire.
#[test]
fn the_uncommitted_row_does_not_follow_the_changes_panel() {
    let mut app = TestApp::new();
    load(
        &mut app,
        vec![commit("aaa1111")],
        vec![modified_file("src/main.rs")],
    );
    assert!(matches!(
        app.history.items().first(),
        Some(HistoryEntry::WorkingCopy)
    ));

    app.files.set(Vec::new());
    app.rebuild_history();
    assert!(
        matches!(app.history.items().first(), Some(HistoryEntry::WorkingCopy)),
        "la working copy è ancora sporca: la riga resta"
    );

    load(&mut app, vec![commit("aaa1111")], Vec::new());
    app.files.set(vec![modified_file("src/main.rs")]);
    app.rebuild_history();
    assert!(
        !app.history
            .items()
            .iter()
            .any(|e| matches!(e, HistoryEntry::WorkingCopy)),
        "riempire il pannello Changes non committa nulla"
    );
}

#[test]
fn an_empty_history_has_no_selection_and_moving_does_not_panic() {
    let mut app = TestApp::new();
    load(&mut app, Vec::new(), Vec::new());
    assert!(app.history.is_empty());
    assert_eq!(app.history.selected_index(), None);

    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Up);
    assert_eq!(app.history.selected_index(), None);
}
