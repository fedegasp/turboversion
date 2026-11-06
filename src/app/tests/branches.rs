//
// src/app/tests/branches.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use super::*;
use crate::git::domain::BranchKind;

#[test]
fn new_branch_is_created_and_selected() {
    let mut app = TestApp::new();
    let n = app.local_branches.len();
    run_menu_action(&mut app, MenuAction::NewBranch);
    type_text(&mut app, "  feature/x ");
    press(&mut app, KeyCode::Enter);

    assert!(app.dialog().is_none());
    assert_eq!(app.local_branches.len(), n + 1);
    assert_eq!(app.local_branches.items()[n].name, "feature/x");
    assert_eq!(app.local_branches.items()[n].kind, BranchKind::Local);
    assert_eq!(app.local_branches.selected_index(), Some(n));
}

#[test]
fn new_branch_cancel_and_validation_keeps_typed_text_for_correction() {
    let mut app = TestApp::new();
    let n = app.local_branches.len();

    run_menu_action(&mut app, MenuAction::NewBranch);
    type_text(&mut app, "abc");
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.local_branches.len(), n);

    for bad in [
        "",
        "two words",
        "develop",
        "-oops",
        "a..b",
        "a@{b",
        "trailing/",
        "trailing.lock",
    ] {
        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, bad);
        press(&mut app, KeyCode::Enter);

        assert!(app.dialog().is_some(), "errore atteso per {bad:?}");
        let d = app.dialog().unwrap();
        assert_eq!(
            d.kind,
            crate::dialog::Kind::Input,
            "deve restare modificabile per correggere il testo, per {bad:?}"
        );
        assert_eq!(
            d.input, bad,
            "il testo digitato non deve andare perso per {bad:?}"
        );

        press(&mut app, KeyCode::Esc);
        assert!(app.dialog().is_none());
        assert_eq!(app.local_branches.len(), n);
    }
}

#[test]
fn current_branch_cannot_be_deleted() {
    let mut app = TestApp::new();
    let n = app.local_branches.len();
    run_menu_action(&mut app, MenuAction::DeleteBranch);
    assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.local_branches.len(), n);
}

#[test]
fn loaded_branches_are_split_by_kind_and_tags_are_dropped() {
    let app = TestApp::new();
    assert!(app
        .local_branches
        .items()
        .iter()
        .all(|b| b.kind == BranchKind::Local));
    assert!(app
        .remote_branches
        .items()
        .iter()
        .all(|b| b.kind == BranchKind::Remote));
    assert!(app
        .local_branches
        .items()
        .iter()
        .any(|b| b.name == "develop"));
    assert!(app
        .remote_branches
        .items()
        .iter()
        .any(|b| b.name == "origin/main"));
    assert!(app
        .local_branches
        .items()
        .iter()
        .chain(app.remote_branches.items())
        .all(|b| b.name != "v0.1.0"));
}

#[test]
fn delete_from_the_remotes_panel_is_refused_instead_of_deleting_a_local_branch() {
    let mut app = TestApp::new();
    let n = app.local_branches.len();
    app.focus = Panel::Remotes;
    // Un locale cancellabile resta selezionato nell'altro pannello: la
    // richiesta non deve ricadere su di lui.
    let i = local_position(&app, "develop");
    app.local_branches.select(i);

    run_menu_action(&mut app, MenuAction::DeleteBranch);
    assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
    assert!(app
        .dialog()
        .unwrap()
        .text
        .contains("Remote branches cannot be deleted"));
    press(&mut app, KeyCode::Enter);

    assert_eq!(app.local_branches.len(), n);
    assert!(app
        .local_branches
        .items()
        .iter()
        .any(|b| b.name == "develop"));
}

#[test]
fn deleting_without_any_local_branch_shows_a_message_instead_of_doing_nothing_silently() {
    let mut app = TestApp::new();
    // "Nessun ramo selezionato" e "nessun ramo" sono ormai la stessa
    // cosa: `ListPanel` non ammette una lista piena senza selezione.
    update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits: Vec::new(),
            files: Vec::new(),
            diff: Vec::new(),
        },
    );
    run_menu_action(&mut app, MenuAction::DeleteBranch);
    assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
    assert!(app.dialog().unwrap().text.contains("No branch selected"));
}

#[test]
fn delete_branch_asks_confirmation_defaulting_to_no() {
    let mut app = TestApp::new();
    let n = app.local_branches.len();
    let i = local_position(&app, "develop");
    app.local_branches.select(i);

    run_menu_action(&mut app, MenuAction::DeleteBranch);
    press(&mut app, KeyCode::Enter); // default: No
    assert_eq!(app.local_branches.len(), n);

    run_menu_action(&mut app, MenuAction::DeleteBranch);
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.local_branches.len(), n - 1);
    assert!(app
        .local_branches
        .items()
        .iter()
        .all(|b| b.name != "develop"));
}

#[test]
fn deleting_last_branch_keeps_selection_in_range() {
    let mut app = TestApp::new();
    // Aggiunge un ramo locale in coda (gli unici cancellabili), lo
    // seleziona e lo cancella: la selezione deve restare nei limiti
    // della lista ridotta.
    run_menu_action(&mut app, MenuAction::NewBranch);
    type_text(&mut app, "temp");
    press(&mut app, KeyCode::Enter);
    let last = app.local_branches.len() - 1;
    assert_eq!(app.local_branches.selected_index(), Some(last));

    run_menu_action(&mut app, MenuAction::DeleteBranch);
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.local_branches.selected_index(), Some(last - 1));
}

#[test]
fn each_branch_panel_moves_only_its_own_selection() {
    let mut app = TestApp::new();
    assert!(app.remote_branches.len() > 1, "servono almeno due remoti");

    app.focus = Panel::Remotes;
    press(&mut app, KeyCode::Down);
    assert_eq!(app.remote_branches.selected_index(), Some(1));
    assert_eq!(
        app.local_branches.selected_index(),
        Some(0),
        "i locali non si muovono"
    );

    app.focus = Panel::Branches;
    press(&mut app, KeyCode::Down);
    assert_eq!(app.local_branches.selected_index(), Some(1));
    assert_eq!(
        app.remote_branches.selected_index(),
        Some(1),
        "i remoti non si muovono"
    );
}

/// Il ramo su cui agiscono le azioni sui rami è quello puntato nel
/// pannello a fuoco quando quel pannello elenca rami, e quello di
/// "Branches" in tutti gli altri casi.
#[test]
fn the_targeted_branch_comes_from_the_focused_branch_panel_or_from_branches() {
    let mut app = TestApp::new();
    let i = local_position(&app, "develop");
    app.local_branches.select(i);

    for focus in [Panel::Branches, Panel::History, Panel::Changes, Panel::Diff] {
        app.focus = focus;
        assert_eq!(
            app.target_branch().map(|b| b.name.as_str()),
            Some("develop"),
            "con il focus su {focus:?} il ramo è quello di Branches"
        );
    }

    app.focus = Panel::Remotes;
    let remote = app.remote_branches.selected().map(|b| b.name.clone());
    assert_eq!(
        app.target_branch().map(|b| b.name.clone()),
        remote,
        "con il focus su Remotes il ramo è quello puntato lì"
    );
    assert_eq!(
        app.target_branch().map(|b| b.kind),
        Some(BranchKind::Remote)
    );
}

/// Il rifiuto è una proprietà del ramo puntato, non del pannello a
/// fuoco. Con il focus su "Remotes" ma senza alcun remoto non c'è nessun
/// ramo remoto da rifiutare: la risposta giusta è che non c'è selezione.
/// Quel che non deve succedere, in nessuno dei due casi, è cancellare il
/// ramo locale selezionato nell'altro pannello.
#[test]
fn with_an_empty_remotes_panel_focused_there_is_no_branch_to_refuse() {
    let mut app = TestApp::new();
    let locals = app.local_branches.items().to_vec();
    let n = locals.len();
    update(
        &mut app.model,
        Msg::Loaded {
            branches: locals,
            commits: Vec::new(),
            files: Vec::new(),
            diff: Vec::new(),
        },
    );
    assert!(app.remote_branches.is_empty());
    app.focus = Panel::Remotes;

    run_menu_action(&mut app, MenuAction::DeleteBranch);
    let d = app.dialog().expect("un messaggio, non una conferma");
    assert_eq!(d.kind, crate::dialog::Kind::Message);
    assert!(d.text.contains("No branch selected"), "{:?}", d.text);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.local_branches.len(), n, "nessun locale cancellato");
}

/// Lo stesso criterio dal lato opposto: con il focus su un pannello che non
/// elenca rami la decisione segue il ramo di "Branches", compreso il suo
/// nome nella conferma — il focus non entra nel merito.
#[test]
fn delete_from_a_non_branch_panel_confirms_on_the_branch_selected_in_branches() {
    let mut app = TestApp::new();
    let i = local_position(&app, "feature/menu");
    app.local_branches.select(i);
    app.focus = Panel::Diff;

    run_menu_action(&mut app, MenuAction::DeleteBranch);
    let d = app.dialog().expect("la conferma di cancellazione");
    assert_eq!(d.kind, crate::dialog::Kind::Confirm);
    assert!(d.text.contains("feature/menu"), "{:?}", d.text);
    press(&mut app, KeyCode::Char('y'));
    assert!(app
        .local_branches
        .items()
        .iter()
        .all(|b| b.name != "feature/menu"));
}
