//
// src/app/tests/changes.rs
// TurboVersion
//
// 2026-11-10 Federico Gasperini
//

//! Il pannello Changes segue la selezione di History: la working copy per
//! la riga delle modifiche non committate, i file del commit altrimenti.
//! Qui si verifica anche ciò che serve perché segua *solo* la selezione: le
//! risposte arrivate tardi si scartano, e `Space` non mette in stage i file
//! di un commit.

use super::*;
use crate::app::model::{ChangesNotice, ChangesView, CommitLoad};
use crate::effects;
use crate::git::domain::{CommitId, FileEntry};
use crate::git::repo::{GitError, Repo};

/// I percorsi della lista che Changes mostra, e se è la working copy.
fn shown(app: &mut TestApp) -> (bool, Vec<String>) {
    let paths = |files: &[FileEntry]| files.iter().map(|f| f.path.clone()).collect();
    match app.model.content_mut(Panel::Changes) {
        PanelContent::WorkingCopyFiles(list) => (true, paths(list.items())),
        PanelContent::CommitFiles(list) => (false, paths(list.items())),
        PanelContent::ChangesNotice(notice) => panic!("Changes non ha una lista: {notice:?}"),
        PanelContent::Branches(_) | PanelContent::History(_) | PanelContent::Diff { .. } => {
            panic!("Changes mostra sempre una lista di file")
        }
    }
}

/// Il motivo per cui Changes non ha una lista, o `None` se ce l'ha.
fn notice(app: &mut TestApp) -> Option<String> {
    match app.model.content_mut(Panel::Changes) {
        PanelContent::ChangesNotice(ChangesNotice::Loading) => Some("loading".to_string()),
        PanelContent::ChangesNotice(ChangesNotice::Failed(m)) => Some(format!("failed: {m}")),
        PanelContent::ChangesNotice(ChangesNotice::GitUnavailable) => {
            Some("unavailable".to_string())
        }
        PanelContent::WorkingCopyFiles(_)
        | PanelContent::CommitFiles(_)
        | PanelContent::Branches(_)
        | PanelContent::History(_)
        | PanelContent::Diff { .. } => None,
    }
}

fn paths_of_commit(app: &TestApp, id: &CommitId) -> Vec<String> {
    app.repo
        .files_of_commit(id)
        .expect("commit del backend di prova")
        .into_iter()
        .map(|f| f.path)
        .collect()
}

fn paths_of_working_copy(app: &TestApp) -> Vec<String> {
    app.repo.files().into_iter().map(|f| f.path).collect()
}

fn commit_id(app: &TestApp, index: usize) -> CommitId {
    app.repo.commits()[index].id.clone()
}

fn files_loaded(app: &TestApp, id: &CommitId) -> Msg {
    Msg::CommitFilesLoaded {
        id: id.clone(),
        result: app.repo.files_of_commit(id),
    }
}

/// `update` con un tasto, trattenendo i `Cmd` invece di eseguirli: è il
/// modo di far arrivare gli esiti più tardi, o in un altro ordine.
fn press_and_hold(app: &mut TestApp, code: KeyCode) -> Vec<Cmd> {
    let msg = input::key_to_msg(KeyEvent::new(code, KeyModifiers::NONE)).expect("tasto noto");
    update(&mut app.model, msg)
}

fn is_load_of(cmds: &[Cmd], id: &CommitId) -> bool {
    matches!(cmds, [Cmd::LoadCommitFiles(asked)] if asked == id)
}

#[test]
fn changes_follows_the_history_selection() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let wc = paths_of_working_copy(&app);
    assert_eq!(shown(&mut app), (true, wc.clone()), "riga non committata");

    press(&mut app, KeyCode::Down);
    let first = commit_id(&app, 0);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &first)));

    press(&mut app, KeyCode::Down);
    let second = commit_id(&app, 1);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &second)));
    assert_ne!(
        paths_of_commit(&app, &first),
        paths_of_commit(&app, &second),
        "i due commit di prova toccano file diversi"
    );

    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    assert_eq!(shown(&mut app), (true, wc), "di nuovo la working copy");
}

/// Tornare sulla working copy la ritrova com'era, cursore compreso: la sua
/// lista non viene sostituita mentre si guardano i commit.
#[test]
fn going_back_to_the_working_copy_keeps_its_cursor() {
    let mut app = TestApp::new();
    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.working_copy_files.selected_index(), Some(2));

    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Up);

    assert!(app.changes().is_working_copy());
    assert_eq!(app.working_copy_files.selected_index(), Some(2));
}

/// I file di un commit si chiedono una volta per ogni commit su cui la
/// selezione arriva, e non si chiedono affatto per la working copy, che è
/// già nel `Model`, né quando la selezione non si muove.
#[test]
fn files_are_asked_for_only_when_the_selection_reaches_a_new_commit() {
    let mut app = TestApp::new();
    app.focus = Panel::History;

    let cmds = press_and_hold(&mut app, KeyCode::Down);
    assert!(is_load_of(&cmds, &commit_id(&app, 0)), "{cmds:?}");
    assert!(
        matches!(app.changes(), ChangesView::Commit(c) if c.is_awaiting() && c.files().is_none()),
        "in attesa, e senza i file della working copy sotto l'identità del commit"
    );

    assert!(
        press_and_hold(&mut app, KeyCode::Up).is_empty(),
        "working copy"
    );

    let last = app.history().len();
    for _ in 0..last {
        app.send(Msg::Down);
    }
    assert!(
        press_and_hold(&mut app, KeyCode::Down).is_empty(),
        "già sull'ultima riga: la selezione non cambia"
    );
}

/// Scorrendo in fretta gli esiti arrivano quando la selezione è già
/// altrove: quello del commit selezionato si applica, gli altri si
/// scartano, qualunque sia l'ordine in cui arrivano.
#[test]
fn a_late_result_for_a_commit_no_longer_selected_is_discarded() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let (first, second) = (commit_id(&app, 0), commit_id(&app, 1));

    let asked_first = press_and_hold(&mut app, KeyCode::Down);
    let asked_second = press_and_hold(&mut app, KeyCode::Down);
    assert!(is_load_of(&asked_first, &first));
    assert!(is_load_of(&asked_second, &second));

    // Il caso che conta: l'esito vecchio arriva mentre si aspetta ancora
    // quello giusto. Applicarlo mostrerebbe i file del primo commit sotto
    // l'identità del secondo.
    app.run(asked_first);
    assert!(
        matches!(app.changes(), ChangesView::Commit(c) if c.is_awaiting() && c.files().is_none()),
        "l'esito del primo commit è arrivato tardi"
    );

    app.run(asked_second);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &second)));

    // E dopo quello giusto non lo sostituisce.
    let msg = files_loaded(&app, &first);
    update(&mut app.model, msg);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &second)));

    // Anche tornati sulla working copy un esito tardivo non la sostituisce.
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    let msg = files_loaded(&app, &first);
    update(&mut app.model, msg);
    assert_eq!(shown(&mut app), (true, paths_of_working_copy(&app)));
}

/// Due richieste per lo stesso commit (A, poi B, poi di nuovo A): la prima
/// che arriva mostra i file, la seconda è un doppione e non deve riportare
/// in cima il cursore che l'utente ha già mosso.
#[test]
fn a_second_result_for_the_same_commit_does_not_reset_its_cursor() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let first = commit_id(&app, 0);

    let a1 = press_and_hold(&mut app, KeyCode::Down);
    let b = press_and_hold(&mut app, KeyCode::Down);
    let a2 = press_and_hold(&mut app, KeyCode::Up);
    assert!(is_load_of(&a2, &first), "A di nuovo: la vista era su B");

    app.run(a1);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &first)));

    // Il doppione arriva con una lista diversa (vuota): se venisse
    // applicato, sostituirebbe quella già a schermo.
    let msg = Msg::CommitFilesLoaded {
        id: first.clone(),
        result: Ok(Vec::new()),
    };
    update(&mut app.model, msg);
    assert!(
        !shown(&mut app).1.is_empty(),
        "il doppione non ha sostituito la lista"
    );

    app.run(b);
    app.run(a2);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &first)));
}

#[test]
fn space_is_ignored_on_the_files_of_a_commit() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    let staged_before: Vec<bool> = app
        .working_copy_files
        .items()
        .iter()
        .map(|f| f.staged)
        .collect();

    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Char(' '));

    let ChangesView::Commit(commit) = app.changes() else {
        panic!("Changes mostra un commit");
    };
    let files = commit.files().expect("i file del commit sono arrivati");
    assert!(files.items().iter().all(|f| !f.staged));
    let staged_after: Vec<bool> = app
        .working_copy_files
        .items()
        .iter()
        .map(|f| f.staged)
        .collect();
    assert_eq!(
        staged_after, staged_before,
        "nemmeno la working copy nascosta"
    );

    // Sulla working copy lo stesso tasto commuta, come prima.
    app.focus = Panel::History;
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Char(' '));
    assert_ne!(
        app.working_copy_files.selected().map(|f| f.staged),
        staged_before.first().copied()
    );
}

/// Un errore di lettura si mostra dentro il pannello, e solo se riguarda il
/// commit a schermo: quello di un commit già lasciato non dice nulla su ciò
/// che si vede. Nessun dialog, che catturerebbe le frecce di chi scorre.
#[test]
fn a_failed_read_is_shown_in_the_panel_only_for_the_commit_on_screen() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let (first, second) = (commit_id(&app, 0), commit_id(&app, 1));
    press_and_hold(&mut app, KeyCode::Down);
    press_and_hold(&mut app, KeyCode::Down);
    let failed = |id: &CommitId| Msg::CommitFilesLoaded {
        id: id.clone(),
        result: Err(GitError("boom".to_string())),
    };

    update(&mut app.model, failed(&first));
    assert_eq!(
        notice(&mut app).as_deref(),
        Some("loading"),
        "esito obsoleto"
    );

    update(&mut app.model, failed(&second));
    assert_eq!(notice(&mut app).as_deref(), Some("failed: boom"));
    assert!(app.dialog().is_none(), "nessun dialog");
    assert!(
        matches!(app.changes(), ChangesView::Commit(c) if !c.is_awaiting()),
        "l'attesa è chiusa: nessun esito arriverà più"
    );

    // Le frecce continuano a muovere History.
    press(&mut app, KeyCode::Down);
    assert_eq!(app.history().selected_index(), Some(3));
}

/// Un commit senza file è un elenco vuoto arrivato, non un'attesa.
#[test]
fn a_commit_without_files_is_loaded_not_pending() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let first = commit_id(&app, 0);
    press_and_hold(&mut app, KeyCode::Down);
    update(
        &mut app.model,
        Msg::CommitFilesLoaded {
            id: first,
            result: Ok(Vec::new()),
        },
    );
    assert_eq!(notice(&mut app), None);
    assert_eq!(shown(&mut app), (false, Vec::new()));
}

/// Tornare sull'ultimo commit lasciato con i file già arrivati lo ritrova
/// com'era, cursore compreso, senza chiedere di nuovo i file.
#[test]
fn going_back_to_the_last_loaded_commit_does_not_read_it_again() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    // Il secondo commit di prova tocca due file: il cursore può muoversi.
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Down);
    let second = commit_id(&app, 1);

    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    let cmds = press_and_hold(&mut app, KeyCode::Up);
    assert!(cmds.is_empty(), "già letto: {cmds:?}");
    let ChangesView::Commit(commit) = app.changes() else {
        panic!("Changes mostra il commit");
    };
    assert_eq!(*commit.id(), second);
    assert_eq!(commit.files().and_then(|f| f.selected_index()), Some(1));

    // Anche passando per una riga che non è un commit: dal primo commit
    // alla working copy e ritorno. La vista conservata è una sola, quella
    // dell'ultimo commit lasciato.
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    assert!(app.changes().is_working_copy());
    assert!(press_and_hold(&mut app, KeyCode::Down).is_empty());
    assert!(
        is_load_of(&press_and_hold(&mut app, KeyCode::Down), &second),
        "il secondo commit non è più conservato: si rilegge"
    );
}

/// La perdita del worker mentre i file di un commit sono in arrivo chiude
/// l'attesa: il pannello dice che non arriveranno, invece di restare su
/// "Loading" per sempre.
#[test]
fn losing_the_worker_while_files_are_loading_says_they_will_not_come() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let first = commit_id(&app, 0);
    let pending = press_and_hold(&mut app, KeyCode::Down);
    assert_eq!(notice(&mut app).as_deref(), Some("loading"));

    update(&mut app.model, Msg::WorkerLost);
    assert_eq!(notice(&mut app).as_deref(), Some("unavailable"));

    // Se l'esito fosse comunque arrivato prima della morte del thread e
    // venisse drenato dopo, non riaprirebbe un'attesa ormai chiusa.
    assert!(is_load_of(&pending, &first));
    let late = files_loaded(&app, &first);
    update(&mut app.model, late);
    assert_eq!(notice(&mut app).as_deref(), Some("unavailable"));
}

/// Senza git, muoversi in History non apre un dialog a ogni freccia: i dati
/// già letti devono restare navigabili. Un commit mai letto dice che i
/// suoi file non sono disponibili, invece di restare "in attesa"; l'ultimo
/// letto prima della perdita resta leggibile.
#[test]
fn without_git_unread_commits_are_unavailable_and_the_last_read_one_survives() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    let first = commit_id(&app, 0);
    let first_paths = paths_of_commit(&app, &first);

    app.send(Msg::WorkerLost);
    press(&mut app, KeyCode::Enter);
    assert!(app.dialog().is_none(), "annuncio chiuso");
    assert_eq!(shown(&mut app), (false, first_paths.clone()), "già letto");

    let cmds = press_and_hold(&mut app, KeyCode::Down);
    assert!(cmds.is_empty(), "nessuno eseguirebbe la lettura");
    assert!(app.dialog().is_none());
    assert_eq!(app.history().selected_index(), Some(2), "ma ci si muove");
    assert!(matches!(
        app.changes(),
        ChangesView::Commit(c) if matches!(c.load(), CommitLoad::Unavailable)
    ));

    press(&mut app, KeyCode::Up);
    assert_eq!(shown(&mut app), (false, first_paths), "ripreso com'era");
}

/// Una rilettura del repository che lascia la selezione dov'era non
/// richiede di nuovo i file del commit, e non tocca il loro cursore.
#[test]
fn a_reload_keeps_the_selected_commit_and_its_files() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Down);
    let second = commit_id(&app, 1);

    let cmds = update(&mut app.model, effects::run(Cmd::LoadAll, &mut app.repo));
    assert!(cmds.is_empty(), "nessuna lettura nuova: {cmds:?}");
    assert!(matches!(
        app.history().selected(),
        Some(HistoryEntry::Commit(c)) if c.id == second
    ));
    let ChangesView::Commit(commit) = app.changes() else {
        panic!("Changes mostra ancora il commit");
    };
    assert_eq!(commit.files().and_then(|f| f.selected_index()), Some(1));
}

/// Se la rilettura sposta la selezione su un commit — la working copy è
/// tornata pulita, e la sua riga è sparita — Changes ne chiede i file.
#[test]
fn a_reload_that_moves_the_selection_onto_a_commit_asks_for_its_files() {
    let mut app = TestApp::new();
    assert!(app.changes().is_working_copy());
    let commits = app.repo.commits();

    let cmds = update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits,
            files: Vec::new(),
            diff: Vec::new(),
        },
    );
    let first = commit_id(&app, 0);
    assert!(is_load_of(&cmds, &first), "{cmds:?}");

    app.run(cmds);
    assert_eq!(shown(&mut app), (false, paths_of_commit(&app, &first)));
}

/// Una lettura fallita non resta conservata: lasciato il commit e tornati
/// sopra, i suoi file si chiedono di nuovo.
#[test]
fn a_failed_read_is_asked_again_when_the_commit_is_reached_again() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let first = commit_id(&app, 0);
    press_and_hold(&mut app, KeyCode::Down);
    update(
        &mut app.model,
        Msg::CommitFilesLoaded {
            id: first.clone(),
            result: Err(GitError("boom".to_string())),
        },
    );
    assert_eq!(notice(&mut app).as_deref(), Some("failed: boom"));

    press(&mut app, KeyCode::Down);
    let cmds = press_and_hold(&mut app, KeyCode::Up);
    assert!(is_load_of(&cmds, &first), "{cmds:?}");
    assert_eq!(notice(&mut app).as_deref(), Some("loading"));
}

/// Con il focus su Changes mentre mostra un segnaposto non c'è nulla da
/// percorrere né da mettere in stage: i tasti non cambiano lo stato e non
/// chiedono nulla.
#[test]
fn keys_on_a_changes_notice_change_nothing() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let first = commit_id(&app, 0);
    press_and_hold(&mut app, KeyCode::Down);
    let staged: Vec<bool> = app
        .working_copy_files
        .items()
        .iter()
        .map(|f| f.staged)
        .collect();

    app.focus = Panel::Changes;
    for key in [KeyCode::Down, KeyCode::Char(' '), KeyCode::Up] {
        assert!(press_and_hold(&mut app, key).is_empty(), "{key:?}");
        assert_eq!(notice(&mut app).as_deref(), Some("loading"));
        assert!(matches!(
            app.changes(),
            ChangesView::Commit(c) if *c.id() == first && c.is_awaiting()
        ));
    }
    let after: Vec<bool> = app
        .working_copy_files
        .items()
        .iter()
        .map(|f| f.staged)
        .collect();
    assert_eq!(after, staged);
    assert_eq!(app.history().selected_index(), Some(1));
}
