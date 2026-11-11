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
        id: CommitId(hash.to_string()),
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

/// Rimpiazza i dati caricati, come farebbe l'esito di `Cmd::LoadAll`. I
/// `Cmd` che ne seguono (i file del commit selezionato) non vengono
/// eseguiti: qui conta la forma di History, e questi commit sintetici il
/// backend di prova non li conosce.
fn load(app: &mut TestApp, commits: Vec<Commit>, files: Vec<FileEntry>) {
    update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits,
            files,
        },
    );
}

/// Muove il cursore di History senza eseguire i `Cmd` che ne seguono: i
/// commit sintetici di questi test il backend di prova non li conosce, e
/// la lettura dei loro file aprirebbe un dialog d'errore.
fn step_history(app: &mut TestApp, msg: Msg) {
    app.focus = Panel::History;
    update(&mut app.model, msg);
}

fn selected_hash(app: &TestApp) -> Option<&str> {
    match app.history.selected() {
        Some(HistoryEntry::Commit(c)) => Some(c.hash.as_str()),
        Some(HistoryEntry::WorkingCopy) | None => None,
    }
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

    step_history(&mut app, Msg::Down);
    assert!(matches!(app.history.selected(), Some(HistoryEntry::Commit(c)) if c.hash == "aaa1111"));
    step_history(&mut app, Msg::Down);
    assert!(matches!(app.history.selected(), Some(HistoryEntry::Commit(c)) if c.hash == "bbb2222"));
    step_history(&mut app, Msg::Down);
    assert_eq!(app.history.selected_index(), Some(2), "ultima riga");

    for _ in 0..5 {
        step_history(&mut app, Msg::Up);
    }
    assert!(matches!(
        app.history.selected(),
        Some(HistoryEntry::WorkingCopy)
    ));
}

/// Una rilettura ritrova la riga selezionata per identità, non per
/// posizione: un commit nuovo in cima, o la riga della working copy che
/// compare, spostano le righe ma non la selezione.
#[test]
fn reloading_keeps_the_selected_row_even_when_it_moves() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222"), commit("ccc3333")],
        Vec::new(),
    );
    step_history(&mut app, Msg::Down);
    assert_eq!(selected_hash(&app), Some("bbb2222"));

    // Un commit nuovo in cima e la working copy sporca: due righe sopra.
    load(
        &mut app,
        vec![
            commit("ddd4444"),
            commit("aaa1111"),
            commit("bbb2222"),
            commit("ccc3333"),
        ],
        vec![modified_file("src/main.rs")],
    );
    assert_eq!(app.history.selected_index(), Some(3));
    assert_eq!(selected_hash(&app), Some("bbb2222"));

    // Anche la riga della working copy si ritrova, se c'è ancora.
    for _ in 0..5 {
        step_history(&mut app, Msg::Up);
    }
    load(
        &mut app,
        vec![commit("aaa1111")],
        vec![modified_file("src/other.rs")],
    );
    assert!(matches!(
        app.history.selected(),
        Some(HistoryEntry::WorkingCopy)
    ));
}

/// Se la working copy torna pulita la sua riga sparisce: il cursore che
/// ci stava sopra scende sul commit più recente, che ne prende il posto.
#[test]
fn when_the_uncommitted_row_goes_away_the_cursor_falls_on_the_latest_commit() {
    let mut app = TestApp::new();
    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222")],
        vec![modified_file("src/main.rs")],
    );
    assert!(matches!(
        app.history.selected(),
        Some(HistoryEntry::WorkingCopy)
    ));

    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222")],
        Vec::new(),
    );
    assert_eq!(selected_hash(&app), Some("aaa1111"));
}

/// Un commit che non c'è più (riscritto, o fuori dal log) lascia il
/// cursore alla stessa altezza, dentro i limiti della lista nuova, invece
/// di riportarlo in cima.
#[test]
fn a_selected_commit_that_is_gone_leaves_the_cursor_at_the_same_height() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    load(
        &mut app,
        vec![commit("aaa1111"), commit("bbb2222"), commit("ccc3333")],
        Vec::new(),
    );
    step_history(&mut app, Msg::Down);
    assert_eq!(selected_hash(&app), Some("bbb2222"));

    load(
        &mut app,
        vec![commit("aaa1111"), commit("eee5555"), commit("ccc3333")],
        Vec::new(),
    );
    assert_eq!(selected_hash(&app), Some("eee5555"), "stessa altezza");

    step_history(&mut app, Msg::Down);
    load(&mut app, vec![commit("aaa1111")], Vec::new());
    assert_eq!(selected_hash(&app), Some("aaa1111"), "limitato all'ultima");
}

/// La riga in cima dipende dalla working copy letta al caricamento, non
/// da ciò che mostra il pannello Changes: guardare i file di un commit non
/// la fa comparire su una working copy pulita, né sparire da una sporca.
/// Se ne dipendesse, ogni cambio di selezione ricomporrebbe History e il
/// cursore inseguirebbe le proprie ricariche.
#[test]
fn the_uncommitted_row_does_not_follow_the_changes_panel() {
    let mut app = TestApp::new();
    assert!(app.working_copy_dirty, "i dati di partenza hanno modifiche");
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    assert!(
        matches!(app.changes(), ChangesView::Commit(c) if c.files().is_some_and(|f| !f.is_empty())),
        "Changes mostra i file di un commit"
    );
    assert!(
        matches!(app.history.items().first(), Some(HistoryEntry::WorkingCopy)),
        "la working copy è ancora sporca: la riga resta"
    );

    // Working copy pulita, con Changes su un commit che ha dei file.
    let commits = app.commits.clone();
    let cmds = update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits,
            files: Vec::new(),
        },
    );
    app.run(cmds);
    assert!(
        matches!(app.changes(), ChangesView::Commit(c) if c.files().is_some_and(|f| !f.is_empty()))
    );
    assert!(
        !app.history
            .items()
            .iter()
            .any(|e| matches!(e, HistoryEntry::WorkingCopy)),
        "i file di un commit non sono modifiche da committare"
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
