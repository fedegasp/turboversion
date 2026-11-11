//
// src/app/tests/diff_follow.rs
// TurboVersion
//
// 2026-11-11 Federico Gasperini
//

//! Il pannello Diff segue il file selezionato in Changes, nel contesto di
//! Changes: la working copy, o il commit selezionato in History. Qui si
//! verifica anche ciò che lo rende affidabile: gli esiti superati dalla
//! selezione si scartano, scorrendo in fretta si legge solo dove ci si
//! ferma, e senza git il pannello dice perché non ha righe.

use super::*;
use crate::app::model::Load;
use crate::effects;
use crate::git::domain::{CommitId, DiffSource, DiffTarget, FileDiff};
use crate::git::repo::{GitError, Repo};

/// Il diff che il pannello mostra: il suo testo, o il segnaposto al suo
/// posto.
fn shown(app: &mut TestApp) -> Result<String, String> {
    match app.model.content_mut(Panel::Diff) {
        PanelContent::Diff { lines, .. } => Ok(lines.join("\n")),
        PanelContent::DiffNotice(notice) => Err(format!("{notice:?}")),
        _ => panic!("il pannello Diff mostra un diff o un segnaposto"),
    }
}

fn target(source: DiffSource, path: &str) -> DiffTarget {
    DiffTarget {
        source,
        path: path.to_string(),
    }
}

fn wc_target(app: &TestApp, index: usize) -> DiffTarget {
    target(
        DiffSource::WorkingCopy,
        &app.repo.files()[index].path.clone(),
    )
}

fn commit_id(app: &TestApp, index: usize) -> CommitId {
    app.repo.commits()[index].id.clone()
}

/// Il testo che il backend di prova dà per `target`.
fn text_of(app: &TestApp, target: &DiffTarget) -> String {
    match app.repo.file_diff(target) {
        Ok(FileDiff::Text(lines)) => lines.join("\n"),
        other => panic!("{target:?}: atteso un diff testuale, non {other:?}"),
    }
}

/// `update` con un tasto, trattenendo i `Cmd` invece di eseguirli.
fn press_and_hold(app: &mut TestApp, code: KeyCode) -> Vec<Cmd> {
    let msg = input::key_to_msg(KeyEvent::new(code, KeyModifiers::NONE)).expect("tasto noto");
    update(&mut app.model, msg)
}

fn diff_reads(cmds: &[Cmd]) -> Vec<&DiffTarget> {
    cmds.iter()
        .filter_map(|c| match c {
            Cmd::LoadFileDiff(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn diff_loaded(target: &DiffTarget, result: Result<FileDiff, GitError>) -> Msg {
    Msg::FileDiffLoaded {
        target: target.clone(),
        result,
    }
}

#[test]
fn the_diff_follows_the_file_selected_in_the_working_copy() {
    let mut app = TestApp::new();
    let first = wc_target(&app, 0);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &first)), "all'avvio");

    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Down);
    let second = wc_target(&app, 1);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &second)));
    assert_ne!(text_of(&app, &first), text_of(&app, &second));
}

/// Su un commit il diff è quello del file selezionato fra i file del
/// commit, nel contesto del commit: lo stesso percorso nella working copy
/// o in un altro commit avrebbe un altro diff.
#[test]
fn on_a_commit_the_diff_is_that_of_the_file_in_the_commit() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    let second = commit_id(&app, 1);
    let files = app.repo.files_of_commit(&second).expect("commit di prova");

    let expected = target(DiffSource::Commit(second.clone()), &files[0].path);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &expected)));

    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Down);
    let expected = target(DiffSource::Commit(second), &files[1].path);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &expected)));

    // Di nuovo sulla working copy: il diff torna quello del suo file.
    app.focus = Panel::History;
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    let wc = wc_target(&app, 0);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &wc)));
}

/// Finché i file di un commit non arrivano non c'è un file selezionato, e
/// quindi nessun diff da chiedere: il diff aspetta i file, e lo dice.
#[test]
fn the_diff_waits_for_the_files_of_a_commit_and_then_reads_the_first() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let cmds = press_and_hold(&mut app, KeyCode::Down);
    assert!(diff_reads(&cmds).is_empty(), "nessun file ancora: {cmds:?}");
    assert_eq!(shown(&mut app).unwrap_err(), "Loading");

    let first = commit_id(&app, 0);
    let msg = effects::run(Cmd::LoadCommitFiles(first.clone()), &mut app.repo);
    let next = update(&mut app.model, msg);
    let path = app.repo.files_of_commit(&first).expect("commit")[0]
        .path
        .clone();
    assert_eq!(
        diff_reads(&next),
        [&target(DiffSource::Commit(first), &path)],
        "arrivati i file, si chiede il diff del primo"
    );
}

/// Un diff arrivato quando la selezione è già su un altro file si scarta,
/// e il suo ritorno fa partire la lettura del file su cui ci si è fermati.
#[test]
fn a_late_diff_for_a_file_no_longer_selected_is_discarded() {
    let mut app = TestApp::new();
    app.focus = Panel::Changes;
    let (second, third) = (wc_target(&app, 1), wc_target(&app, 2));

    let asked = press_and_hold(&mut app, KeyCode::Down);
    assert_eq!(diff_reads(&asked), [&second]);
    assert!(press_and_hold(&mut app, KeyCode::Down).is_empty(), "fusa");

    let late = diff_loaded(&second, app.repo.file_diff(&second));
    let next = update(&mut app.model, late);
    assert_eq!(shown(&mut app).unwrap_err(), "Loading", "obsoleto");
    assert_eq!(diff_reads(&next), [&third]);

    app.run(next);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &third)));

    // Dopo quello giusto, un esito tardivo non lo sostituisce.
    let late = diff_loaded(&second, app.repo.file_diff(&second));
    update(&mut app.model, late);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &third)));
}

/// Scorrendo la lista dei file, al worker va una lettura del diff per
/// volta: quella del primo file raggiunto, poi quella dell'ultimo.
#[test]
fn scrolling_through_the_files_reads_only_where_the_selection_stops() {
    let mut app = TestApp::new();
    app.focus = Panel::Changes;
    let mut asked = Vec::new();
    for _ in 0..4 {
        asked.extend(press_and_hold(&mut app, KeyCode::Down));
    }
    let (second, last) = (wc_target(&app, 1), wc_target(&app, 4));
    assert_eq!(diff_reads(&asked), [&second], "{asked:?}");

    let msg = effects::run(Cmd::LoadFileDiff(second), &mut app.repo);
    let next = update(&mut app.model, msg);
    assert_eq!(diff_reads(&next), [&last]);
    app.run(next);
    assert_eq!(shown(&mut app), Ok(text_of(&app, &last)));
}

/// Il diff dipende da Changes, che dipende da History: scorrendo History
/// in fretta non parte nessuna lettura di diff per i commit attraversati.
#[test]
fn scrolling_through_history_reads_no_diff_of_the_commits_crossed() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let mut cmds = Vec::new();
    for _ in 0..4 {
        cmds.extend(press_and_hold(&mut app, KeyCode::Down));
    }
    assert!(diff_reads(&cmds).is_empty(), "{cmds:?}");

    let mut reads = 0;
    let mut queue = cmds;
    while let Some(cmd) = queue.pop() {
        reads += usize::from(matches!(cmd, Cmd::LoadFileDiff(_)));
        let msg = effects::run(cmd, &mut app.repo);
        queue.extend(update(&mut app.model, msg));
    }
    assert_eq!(reads, 1, "solo il diff del commit su cui ci si è fermati");
    let fourth = commit_id(&app, 3);
    assert!(matches!(
        app.diff(),
        Some(d) if d.key().source == DiffSource::Commit(fourth.clone())
    ));
}

/// Cambiare diff riporta lo scorrimento in cima; un tasto che non cambia
/// il file selezionato lo lascia dov'è.
#[test]
fn the_scroll_resets_only_when_the_diff_changes() {
    let mut app = TestApp::new();
    app.show_diff_for_tests((0..50).map(|i| format!("line {i}")).collect());
    app.send(Msg::DiffViewport(10));
    app.focus = Panel::Diff;
    for _ in 0..5 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.diff_scroll, 5);

    // Su Changes, fermi in cima: la selezione non cambia.
    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Up);
    assert_eq!(app.diff_scroll, 5, "stesso file, stesso diff");

    press(&mut app, KeyCode::Down);
    assert_eq!(app.diff_scroll, 0, "un altro file");
}

/// Su un segnaposto non c'è nulla da scorrere, e l'altezza del pannello
/// non inventa uno scorrimento.
#[test]
fn a_diff_notice_does_not_scroll() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press_and_hold(&mut app, KeyCode::Down);
    assert_eq!(shown(&mut app).unwrap_err(), "Loading");

    app.focus = Panel::Diff;
    press(&mut app, KeyCode::Down);
    app.send(Msg::DiffViewport(3));
    assert_eq!(app.diff_scroll, 0);
}

/// Binari, file senza righe cambiate ed errori si dicono nel pannello,
/// senza dialog: un dialog catturerebbe le frecce di chi sta scorrendo.
#[test]
fn every_diff_without_lines_says_why_in_the_panel() {
    let cases = [
        (Ok(FileDiff::Binary), "Binary"),
        (Ok(FileDiff::NoTextChanges), "NoTextChanges"),
        (Ok(FileDiff::NestedRepository), "NestedRepository"),
        (Err(GitError("boom".to_string())), "Failed(\"boom\")"),
    ];
    for (result, expected) in cases {
        let mut app = TestApp::new();
        app.focus = Panel::Changes;
        let second = wc_target(&app, 1);
        press_and_hold(&mut app, KeyCode::Down);
        update(&mut app.model, diff_loaded(&second, result));
        assert_eq!(shown(&mut app).unwrap_err(), expected);
        assert!(app.dialog().is_none());
    }
}

/// Senza file in Changes non c'è nessun diff da mostrare, né da chiedere;
/// il pannello dice perché, guardando anche lo stato di Changes.
#[test]
fn without_a_selected_file_there_is_no_diff() {
    let mut app = TestApp::new();
    let cmds = update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits: Vec::new(),
            files: Vec::new(),
        },
    );
    assert!(cmds.is_empty(), "{cmds:?}");
    assert_eq!(shown(&mut app).unwrap_err(), "NoFile");

    // I file di un commit non si sono potuti leggere: non è "nessun file
    // selezionato", e l'errore lo mostra già Changes.
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press_and_hold(&mut app, KeyCode::Down);
    let id = commit_id(&app, 0);
    update(
        &mut app.model,
        Msg::CommitFilesLoaded {
            id,
            result: Err(GitError("boom".to_string())),
        },
    );
    assert_eq!(shown(&mut app).unwrap_err(), "FilesNotRead");

    // I file di un commit vuoto sono arrivati: lì sì, nessun file.
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press_and_hold(&mut app, KeyCode::Down);
    let id = commit_id(&app, 0);
    let next = update(
        &mut app.model,
        Msg::CommitFilesLoaded {
            id,
            result: Ok(Vec::new()),
        },
    );
    assert!(diff_reads(&next).is_empty());
    assert_eq!(shown(&mut app).unwrap_err(), "NoFile");
}

/// Una rilettura del repository rilegge il diff della working copy anche
/// se il file selezionato è lo stesso: i file su disco possono essere
/// cambiati. Il diff di un file in un commit invece resta.
#[test]
fn a_reload_reads_the_working_copy_diff_again_but_not_a_commit_one() {
    let mut app = TestApp::new();
    let first = wc_target(&app, 0);
    let cmds = update(&mut app.model, effects::run(Cmd::LoadAll, &mut app.repo));
    assert_eq!(diff_reads(&cmds), [&first]);
    app.run(cmds);

    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    let cmds = update(&mut app.model, effects::run(Cmd::LoadAll, &mut app.repo));
    assert!(cmds.is_empty(), "il commit non è cambiato: {cmds:?}");
    assert!(shown(&mut app).is_ok());
}

/// La perdita del worker con un diff in volo chiude l'attesa: il pannello
/// dice che il diff non arriverà, e un esito tardivo non la riapre.
#[test]
fn losing_the_worker_while_a_diff_is_loading_says_it_will_not_come() {
    let mut app = TestApp::new();
    app.focus = Panel::Changes;
    let second = wc_target(&app, 1);
    let pending = press_and_hold(&mut app, KeyCode::Down);
    assert_eq!(diff_reads(&pending), [&second]);
    assert_eq!(shown(&mut app).unwrap_err(), "Loading");

    update(&mut app.model, Msg::WorkerLost);
    assert_eq!(shown(&mut app).unwrap_err(), "GitUnavailable");

    let late = diff_loaded(&second, app.repo.file_diff(&second));
    let next = update(&mut app.model, late);
    assert!(next.is_empty());
    assert_eq!(shown(&mut app).unwrap_err(), "GitUnavailable");
}

/// Senza git, i file e i diff dei commit già guardati restano leggibili,
/// e non solo quelli dell'ultimo: è ciò che la conservazione delle viste
/// compra. Un commit mai guardato non ha né file né diff, e lo dice;
/// muoversi non apre dialog e non chiede nulla.
#[test]
fn without_git_the_commits_already_viewed_keep_their_files_and_diff() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    let mut seen = Vec::new();
    for _ in 0..3 {
        press(&mut app, KeyCode::Down);
        seen.push(shown(&mut app).expect("diff arrivato"));
    }
    assert_eq!(app.kept_counts(), (2, 2), "i due commit lasciati");

    app.send(Msg::WorkerLost);
    press(&mut app, KeyCode::Enter);

    for expected in seen.iter().rev().skip(1) {
        let cmds = press_and_hold(&mut app, KeyCode::Up);
        assert!(cmds.is_empty(), "{cmds:?}");
        assert_eq!(shown(&mut app).as_ref(), Ok(expected));
    }
    assert!(app.dialog().is_none());

    // Il quarto commit non è mai stato guardato: senza i suoi file anche
    // il diff è "git non disponibile", non "nessun file selezionato".
    for _ in 0..3 {
        assert!(press_and_hold(&mut app, KeyCode::Down).is_empty());
    }
    assert!(matches!(
        app.changes(),
        crate::app::model::ChangesView::Commit(c) if matches!(c.load(), Load::Unavailable)
    ));
    assert_eq!(shown(&mut app).unwrap_err(), "GitUnavailable");

    // Sulla working copy il diff si rileggerebbe: senza git non si può.
    for _ in 0..4 {
        press(&mut app, KeyCode::Up);
    }
    assert!(app.changes().is_working_copy());
    assert_eq!(shown(&mut app).unwrap_err(), "GitUnavailable");
}

/// La perdita del worker mentre si aspettano i file di un commit chiude
/// anche l'attesa del diff, che da quei file dipende: il Diff dice "git non
/// disponibile", non "in arrivo" per sempre né "nessun file".
#[test]
fn losing_the_worker_while_the_files_of_a_commit_load_closes_the_diff_too() {
    let mut app = TestApp::new();
    app.focus = Panel::History;
    press_and_hold(&mut app, KeyCode::Down);
    assert_eq!(shown(&mut app).unwrap_err(), "Loading");

    update(&mut app.model, Msg::WorkerLost);
    assert_eq!(shown(&mut app).unwrap_err(), "GitUnavailable");
}
