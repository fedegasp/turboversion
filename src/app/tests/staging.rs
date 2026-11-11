//
// src/app/tests/staging.rs
// TurboVersion
//
// 2026-11-11 Federico Gasperini
//

//! Stage, unstage e commit dalla UI: `Space` su un file della working copy
//! chiede la scrittura, e il `Model` cambia solo quando il repository,
//! riletto, dice che cosa è successo; il commit passa da un dialog che
//! rifiuta un messaggio vuoto e uno stage vuoto. Qui si verifica anche che
//! dopo una scrittura il diff della working copy letto prima non venga
//! mostrato come attuale.

use super::*;
use crate::dialog::Kind;
use crate::git::domain::{DiffSource, DiffTarget, FileDiff, FileStatus, Staging};
use crate::git::mock::MockGitRepo;
use crate::git::repo::{GitError, Repo};

/// Stato e stage del file `path` nella working copy del `Model`.
fn in_model(app: &TestApp, path: &str) -> Option<(FileStatus, Staging)> {
    app.working_copy_files
        .items()
        .iter()
        .find(|f| f.path == path)
        .map(|f| (f.status, f.staging))
}

fn selected_path(app: &TestApp) -> Option<String> {
    app.working_copy_files.selected().map(|f| f.path.clone())
}

/// Porta il cursore di Changes sul file `path` della working copy.
fn select_file(app: &mut TestApp, path: &str) {
    let index = app
        .working_copy_files
        .items()
        .iter()
        .position(|f| f.path == path)
        .unwrap_or_else(|| panic!("file {path:?} non trovato"));
    app.focus = Panel::Changes;
    // Passando dai tasti, così anche il Diff segue la selezione.
    for _ in 0..app.working_copy_files.len() {
        press(app, KeyCode::Up);
    }
    for _ in 0..index {
        press(app, KeyCode::Down);
    }
}

/// `update` con un tasto, trattenendo i `Cmd` invece di eseguirli.
fn press_and_hold(app: &mut TestApp, code: KeyCode) -> Vec<Cmd> {
    let msg = input::key_to_msg(KeyEvent::new(code, KeyModifiers::NONE)).expect("tasto noto");
    update(&mut app.model, msg)
}

/// Il diff che il pannello mostra: il suo testo, o il segnaposto.
fn shown_diff(app: &mut TestApp) -> Result<String, String> {
    match app.model.content_mut(Panel::Diff) {
        PanelContent::Diff { lines, .. } => Ok(lines.join("\n")),
        PanelContent::DiffNotice(notice) => Err(format!("{notice:?}")),
        _ => panic!("il pannello Diff mostra un diff o un segnaposto"),
    }
}

fn wc_target(path: &str) -> DiffTarget {
    DiffTarget {
        source: DiffSource::WorkingCopy,
        path: path.to_string(),
    }
}

fn open_commit_dialog(app: &mut TestApp) {
    run_menu_action(app, MenuAction::Commit);
}

#[test]
fn space_stages_an_unstaged_file_and_the_cursor_stays_on_it() {
    let mut app = TestApp::new();
    select_file(&mut app, "notes.txt");
    assert_eq!(
        in_model(&app, "notes.txt"),
        Some((FileStatus::Untracked, Staging::Unstaged))
    );

    press(&mut app, KeyCode::Char(' '));

    // Lo stato viene dal repository riletto: il file non tracciato è ora
    // aggiunto, cosa che una commutazione locale della casella non direbbe.
    assert_eq!(
        in_model(&app, "notes.txt"),
        Some((FileStatus::Added, Staging::Staged))
    );
    assert_eq!(selected_path(&app).as_deref(), Some("notes.txt"));
    assert!(app.dialog().is_none());
}

#[test]
fn space_unstages_a_staged_file() {
    let mut app = TestApp::new();
    select_file(&mut app, "src/main.rs");
    assert_eq!(
        in_model(&app, "src/main.rs").map(|s| s.1),
        Some(Staging::Staged)
    );

    press(&mut app, KeyCode::Char(' '));

    assert_eq!(
        in_model(&app, "src/main.rs").map(|s| s.1),
        Some(Staging::Unstaged)
    );
    assert_eq!(selected_path(&app).as_deref(), Some("src/main.rs"));
}

/// Un file in stage solo in parte vi entra per intero: è ciò che dice
/// `Model::stage_toggle`, e ciò che chiede `update`.
#[test]
fn space_on_a_partly_staged_file_asks_to_stage_it() {
    let mut app = TestApp::new();
    let mut files = app.repo.files();
    files[0].staging = Staging::Partly;
    let cmds = update(
        &mut app.model,
        Msg::Loaded {
            branches: app.repo.branches(),
            commits: app.repo.commits(),
            files,
        },
    );
    app.run(cmds);
    app.focus = Panel::Changes;

    let cmds = press_and_hold(&mut app, KeyCode::Char(' '));
    assert!(
        matches!(cmds.as_slice(), [Cmd::StageFile(p)] if p == "src/main.rs"),
        "{cmds:?}"
    );
}

#[test]
fn a_failed_stage_or_unstage_says_so_and_changes_nothing() {
    let mut app = TestApp::new();
    let before: Vec<_> = app
        .working_copy_files
        .items()
        .iter()
        .map(|f| f.staging)
        .collect();

    let cmds = update(
        &mut app.model,
        Msg::FileStaged {
            result: Err(GitError("Cannot stage 'x': boom".to_string())),
        },
    );
    assert!(cmds.is_empty(), "nessuna rilettura dopo un errore");
    let dialog = app.dialog().expect("un dialog d'errore");
    assert_eq!(dialog.title, "Stage");
    assert!(dialog.text.contains("boom"));
    press(&mut app, KeyCode::Enter);

    update(
        &mut app.model,
        Msg::FileUnstaged {
            result: Err(GitError("nope".to_string())),
        },
    );
    assert_eq!(app.dialog().map(|d| d.title.as_str()), Some("Unstage"));

    let after: Vec<_> = app
        .working_copy_files
        .items()
        .iter()
        .map(|f| f.staging)
        .collect();
    assert_eq!(after, before);
}

/// Il diff della working copy chiesto prima di uno stage torna prima del
/// suo esito (il worker esegue i `Cmd` in ordine): quando arriva l'esito
/// dello stage quel diff non si mostra più come attuale, il pannello torna
/// in attesa e se ne chiede uno nuovo, dopo la rilettura del repository.
#[test]
fn a_working_copy_diff_read_before_a_stage_is_not_shown_as_current() {
    let mut app = TestApp::new();
    app.focus = Panel::Changes;

    // Il cursore scende su `src/theme.rs`: il suo diff parte e resta in
    // volo, poi `Space` ne chiede lo stage.
    let read = press_and_hold(&mut app, KeyCode::Down);
    let theme = wc_target("src/theme.rs");
    assert!(
        matches!(read.as_slice(), [Cmd::LoadFileDiff(t)] if *t == theme),
        "{read:?}"
    );
    let write = press_and_hold(&mut app, KeyCode::Char(' '));
    assert!(
        matches!(write.as_slice(), [Cmd::StageFile(p)] if p == "src/theme.rs"),
        "{write:?}"
    );

    // Torna la lettura partita prima della scrittura, con un testo
    // riconoscibile.
    update(
        &mut app.model,
        Msg::FileDiffLoaded {
            target: theme.clone(),
            result: Ok(FileDiff::Text(vec!["stale".to_string()])),
        },
    );
    assert_eq!(shown_diff(&mut app), Ok("stale".to_string()));

    // Poi torna lo stage.
    let result = app.repo.stage_file("src/theme.rs");
    let cmds = update(&mut app.model, Msg::FileStaged { result });

    assert_eq!(
        shown_diff(&mut app),
        Err("Loading".to_string()),
        "il diff letto prima dello stage non è più quello a schermo"
    );
    assert!(
        matches!(cmds.as_slice(), [Cmd::LoadAll, Cmd::LoadFileDiff(t)] if *t == theme),
        "prima la rilettura, poi il diff: {cmds:?}"
    );

    app.run(cmds);
    let fresh = match app.repo.file_diff(&theme) {
        Ok(FileDiff::Text(lines)) => lines.join("\n"),
        other => panic!("atteso un diff testuale, non {other:?}"),
    };
    assert_eq!(shown_diff(&mut app), Ok(fresh));
    assert_eq!(
        in_model(&app, "src/theme.rs").map(|s| s.1),
        Some(Staging::Staged)
    );
}

#[test]
fn commit_creates_a_commit_from_what_is_staged() {
    let mut app = TestApp::new();
    let commits = app.repo.commits().len();

    open_commit_dialog(&mut app);
    assert_eq!(app.dialog().map(|d| d.kind), Some(Kind::Input));
    type_text(&mut app, "Wire the main loop");
    press(&mut app, KeyCode::Enter);

    assert!(
        app.dialog().is_none(),
        "nessun dialog dopo un commit riuscito"
    );
    assert_eq!(app.repo.commits().len(), commits + 1);
    assert_eq!(app.repo.commits()[0].msg, "Wire the main loop");
    assert!(in_model(&app, "src/main.rs").is_none(), "committato");
    // Restano altre modifiche: la riga non committata c'è ancora, ed è
    // ancora quella selezionata; sotto di lei il commit nuovo.
    assert!(matches!(
        app.history().selected(),
        Some(HistoryEntry::WorkingCopy)
    ));
    assert!(matches!(
        app.history().items().get(1),
        Some(HistoryEntry::Commit(c)) if c.msg == "Wire the main loop"
    ));
}

/// Committando tutto la working copy torna pulita e la riga "Uncommitted
/// changes" sparisce: la selezione, che era su quella riga, passa al
/// commit appena creato, e Changes ne mostra i file.
#[test]
fn committing_everything_removes_the_uncommitted_row() {
    let mut app = TestApp::new();
    for path in app.repo.files().into_iter().map(|f| f.path) {
        if in_model(&app, &path).map(|s| s.1) != Some(Staging::Staged) {
            select_file(&mut app, &path);
            press(&mut app, KeyCode::Char(' '));
        }
    }
    app.focus = Panel::History;

    open_commit_dialog(&mut app);
    type_text(&mut app, "Everything");
    press(&mut app, KeyCode::Enter);

    assert!(app.working_copy_files.is_empty());
    let new = app.repo.commits()[0].id.clone();
    assert!(matches!(
        app.history().selected(),
        Some(HistoryEntry::Commit(c)) if c.id == new
    ));
    assert!(matches!(
        app.changes(),
        crate::app::model::ChangesView::Commit(c) if *c.id() == new && c.files().is_some()
    ));
}

#[test]
fn an_empty_commit_message_is_refused_without_closing_the_dialog() {
    let mut app = TestApp::new();
    let commits = app.repo.commits().len();

    open_commit_dialog(&mut app);
    type_text(&mut app, "   ");
    let cmds = press_and_hold(&mut app, KeyCode::Enter);

    assert!(cmds.is_empty(), "nessun commit chiesto: {cmds:?}");
    let dialog = app.dialog().expect("il dialog si riapre");
    assert_eq!(dialog.kind, Kind::Input);
    assert!(dialog.text.contains("cannot be empty"), "{}", dialog.text);
    assert_eq!(dialog.input, "   ", "il testo digitato resta");
    assert_eq!(app.repo.commits().len(), commits);
}

/// Senza nulla in stage il dialog del messaggio non si apre nemmeno: si
/// dice subito che non c'è nulla da committare.
#[test]
fn commit_with_nothing_staged_says_so_instead_of_asking_a_message() {
    let mut app = TestApp::new();
    select_file(&mut app, "src/main.rs");
    press(&mut app, KeyCode::Char(' '));
    assert!(app
        .working_copy_files
        .items()
        .iter()
        .all(|f| f.staging == Staging::Unstaged));

    open_commit_dialog(&mut app);

    let dialog = app.dialog().expect("un dialog");
    assert_eq!(dialog.kind, Kind::Message);
    assert!(dialog.text.contains("Nothing to commit"), "{}", dialog.text);
}

/// Un commit rifiutato dal backend per una ragione che il messaggio non
/// può correggere (qui: l'indice è stato svuotato da un altro client) lo
/// dice in un messaggio, senza riaprire il campo come se bastasse
/// riscriverlo.
#[test]
fn a_commit_refused_for_another_reason_than_the_message_is_just_reported() {
    let mut app = TestApp::new();
    app.repo.unstage_file("src/main.rs").expect("file di prova");

    open_commit_dialog(&mut app);
    type_text(&mut app, "Too late");
    press(&mut app, KeyCode::Enter);

    let dialog = app.dialog().expect("un dialog");
    assert_eq!(dialog.kind, Kind::Message);
    assert_eq!(dialog.title, "Commit");
    assert!(dialog.text.contains("Nothing to commit"), "{}", dialog.text);
}

/// Un errore sul messaggio arrivato dal backend (la regola è la stessa
/// del dialog, ma il backend non si fida) riapre il campo con il testo.
#[test]
fn a_commit_refused_for_its_message_reopens_the_field() {
    let mut app = TestApp::new();
    update(
        &mut app.model,
        Msg::Committed {
            message: " ".to_string(),
            result: Err(GitError("Commit message cannot be empty.".to_string())),
        },
    );
    let dialog = app.dialog().expect("il dialog si riapre");
    assert_eq!(dialog.kind, Kind::Input);
    assert_eq!(dialog.input, " ");
}

/// Il messaggio di commit accetta più di un nome di ramo.
#[test]
fn the_commit_message_is_longer_than_a_branch_name() {
    let mut app = TestApp::new();
    open_commit_dialog(&mut app);
    let long = "x".repeat(150);
    type_text(&mut app, &long);
    assert_eq!(app.dialog().map(|d| d.input.clone()), Some(long));
}

/// Una directory non tracciata è una riga sola; messa in stage, la riga
/// diventa i file che contiene, e il cursore resta alla stessa altezza,
/// cioè sul primo di essi.
#[test]
fn space_on_an_untracked_directory_splits_it_into_its_files() {
    let repo =
        MockGitRepo::new().with_untracked_dir(2, "assets/", &["assets/a.png", "assets/b.png"]);
    let mut app = TestApp::with_repo(repo);
    select_file(&mut app, "assets/");
    let height = app.working_copy_files.selected_index();
    let rows = app.working_copy_files.len();

    press(&mut app, KeyCode::Char(' '));

    assert!(
        in_model(&app, "assets/").is_none(),
        "la riga della directory"
    );
    for path in ["assets/a.png", "assets/b.png"] {
        assert_eq!(
            in_model(&app, path),
            Some((FileStatus::Added, Staging::Staged)),
            "{path}"
        );
    }
    assert_eq!(app.working_copy_files.len(), rows + 1);
    assert_eq!(app.working_copy_files.selected_index(), height);
    assert_eq!(selected_path(&app).as_deref(), Some("assets/a.png"));
}

/// Perso il worker, stage e commit sono azioni dell'utente: si rifiutano
/// dicendolo, e nulla cambia.
#[test]
fn without_git_stage_and_commit_are_refused_out_loud() {
    let mut app = TestApp::new();
    update(&mut app.model, Msg::WorkerLost);
    press(&mut app, KeyCode::Enter); // chiude l'annuncio
    app.focus = Panel::Changes;

    let cmds = press_and_hold(&mut app, KeyCode::Char(' '));
    assert!(cmds.is_empty());
    assert_eq!(
        app.dialog().map(|d| d.title.as_str()),
        Some("Git unavailable")
    );
    press(&mut app, KeyCode::Enter);

    open_commit_dialog(&mut app);
    type_text(&mut app, "Lost");
    let cmds = press_and_hold(&mut app, KeyCode::Enter);
    assert!(cmds.is_empty());
    assert_eq!(
        app.dialog().map(|d| d.title.as_str()),
        Some("Git unavailable")
    );
    assert_eq!(
        in_model(&app, "src/main.rs").map(|s| s.1),
        Some(Staging::Staged)
    );
}
