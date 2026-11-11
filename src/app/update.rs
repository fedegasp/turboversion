//
// src/app/update.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::msg::{Cmd, Msg};

use super::dialog_flow::update_dialog;
use super::dialogs;
use super::menu::{open_menu_by_hotkey, update_menu};
use super::model::{max_diff_scroll, MenuState, Model, Overlay, StageToggle};
use super::panel::Panel;
use super::results::{
    apply_branch_created, apply_branch_deleted, apply_commit_files, apply_committed,
    apply_file_diff, apply_loaded, apply_staging_written,
};

pub(crate) fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Quit => {
            model.running = false;
            Vec::new()
        }
        Msg::OpenMenuByHotkey(c) => {
            open_menu_by_hotkey(model, c);
            Vec::new()
        }
        Msg::DiffViewport(height) => {
            apply_diff_viewport(model, height);
            Vec::new()
        }
        Msg::WorkerLost => {
            apply_worker_lost(model);
            Vec::new()
        }
        Msg::Loaded {
            branches,
            commits,
            files,
        } => apply_loaded(model, branches, commits, files),
        Msg::CommitFilesLoaded { id, result } => apply_commit_files(model, id, result),
        Msg::FileDiffLoaded { target, result } => apply_file_diff(model, target, result),
        Msg::BranchCreated { name, result } => {
            apply_branch_created(model, name, result);
            Vec::new()
        }
        Msg::BranchDeleted { name, result } => {
            apply_branch_deleted(model, name, result);
            Vec::new()
        }
        Msg::FileStaged { result } => apply_staging_written(model, result, dialogs::stage_failed),
        Msg::FileUnstaged { result } => {
            apply_staging_written(model, result, dialogs::unstage_failed)
        }
        Msg::Committed { message, result } => apply_committed(model, message, result),
        // Le varianti restanti sono scorciatoie "di tasto" generiche: il
        // loro significato dipende da quale overlay è in cima (dialog >
        // menu > pannelli principali), vedi `dispatch_key`.
        key_msg => {
            let cmds = dispatch_key(model, key_msg);
            refuse_if_git_is_gone(model, cmds)
        }
    }
}

/// Rifiuta, dicendolo, i `Cmd` nati da un'azione dell'utente quando le
/// operazioni git non sono più possibili: nessuno li eseguirebbe, e
/// l'azione si chiuderebbe senza effetto e senza spiegazione.
///
/// Sta qui, a valle di ogni tasto, e non su ciascuna azione: un `Cmd`
/// aggiunto domani è coperto senza doverci ripensare. I `Msg` che non
/// vengono da un tasto non passano di qui, e non devono: un esito o una
/// misura del layout non chiedono niente a git.
///
/// Tutti i `Cmd` vengono scartati, ma il dialog compare solo se almeno uno
/// era stato chiesto dall'utente (vedi `asked_by_the_user`): una lettura
/// che segue la selezione, rifiutata con un dialog, ne aprirebbe uno a ogni
/// freccia e renderebbe innavigabili proprio i dati che restano a schermo.
fn refuse_if_git_is_gone(model: &mut Model, cmds: Vec<Cmd>) -> Vec<Cmd> {
    if cmds.is_empty() || model.git_available() {
        return cmds;
    }
    if cmds.iter().any(asked_by_the_user) {
        model
            .overlays
            .push(Overlay::Dialog(dialogs::git_unavailable()));
    }
    Vec::new()
}

/// Se `cmd` è qualcosa che l'utente ha chiesto, e che quindi va rifiutato
/// dicendolo, o una lettura che l'applicazione fa da sé per seguirne la
/// navigazione. Il match è esaustivo: un `Cmd` nuovo non compila finché non
/// si dice in quale dei due casi cade.
fn asked_by_the_user(cmd: &Cmd) -> bool {
    match cmd {
        Cmd::LoadAll
        | Cmd::CreateBranch(_)
        | Cmd::DeleteBranch(_)
        | Cmd::StageFile(_)
        | Cmd::UnstageFile(_)
        | Cmd::Commit(_) => true,
        Cmd::LoadCommitFiles(_) | Cmd::LoadFileDiff(_) => false,
    }
}

/// Smista un tasto all'overlay in cima allo stack, o ai pannelli se non
/// ce n'è nessuno. Lo stack può contenere più di un overlay: l'annuncio
/// della perdita delle operazioni git si impila anche sopra un menu o un
/// dialog già aperti, e chiuderlo riporta sotto quello che c'era.
fn dispatch_key(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match model.overlays.last() {
        Some(Overlay::Dialog(_)) => update_dialog(model, msg),
        Some(Overlay::Menu(_)) => update_menu(model, msg),
        None => update_main(model, msg),
    }
}

/// Registra l'altezza del pannello Diff e riporta subito lo scorrimento
/// entro il nuovo limite: ingrandendo il terminale, uno scorrimento che
/// prima era al fondo lascerebbe altrimenti righe vuote sotto l'ultima riga
/// del diff fino al tasto successivo.
///
/// Sta qui e non fra gli `apply_*` di `results.rs` perché `Msg::DiffViewport`
/// non è l'esito di un `Cmd`: nasce dal layout, dentro il processo della UI
/// (vedi la regola nel doc-comment di `results.rs`).
fn apply_diff_viewport(model: &mut Model, height: u16) {
    model.diff_viewport = height;
    model.diff_scroll = model
        .diff_scroll
        .min(max_diff_scroll(model.diff_line_count(), height));
}

/// Registra che le operazioni git non sono più disponibili e lo annuncia
/// una volta sola: il canale verso il worker, una volta chiuso, resta
/// chiuso, e ogni `Cmd` successivo fallisce allo stesso modo — una pila di
/// dialog identici non direbbe niente di più del primo. A rendere
/// l'annuncio irripetibile è `Model::lose_git`, non chi chiama.
///
/// L'applicazione resta in esecuzione: branch, log, file e diff già letti
/// sono ancora validi e si navigano come prima. Chiudere di colpo
/// toglierebbe all'utente anche quello che ha sotto gli occhi. Ciò che
/// non si può più fare lo dice il dialog, e dopo la sua chiusura la barra
/// di stato (vedi `ui::hints`).
///
/// Sta qui e non fra gli `apply_*` di `results.rs` per la stessa ragione
/// di `apply_diff_viewport`: non è l'esito di un `Cmd`, è qualcosa che il
/// ciclo della UI osserva sui canali del worker.
fn apply_worker_lost(model: &mut Model) {
    if model.lose_git() {
        model
            .overlays
            .push(Overlay::Dialog(dialogs::git_unavailable()));
    }
}

fn update_main(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::F10 => model
            .overlays
            .push(Overlay::Menu(MenuState { menu: 0, item: 0 })),
        Msg::Tab => model.focus = model.focus.next(),
        Msg::BackTab => model.focus = model.focus.prev(),
        Msg::Up | Msg::Char { c: 'k', .. } => return step(model, -1),
        Msg::Down | Msg::Char { c: 'j', .. } => return step(model, 1),
        Msg::Char { c: ' ', .. } if model.focus == Panel::Changes => return toggle_stage(model),
        _ => {}
    }
    Vec::new()
}

/// Chiede di mettere in stage il file selezionato in Changes, o di
/// toglierlo, se Changes mostra la working copy: sui file di un commit —
/// che non hanno un indice in cui entrare o da cui uscire — e su una lista
/// vuota non c'è nulla da chiedere (vedi `Model::stage_toggle`).
///
/// Il `Model` non cambia qui: la casella del file cambia quando il
/// repository, riletto dopo la scrittura, dice che cosa è successo davvero.
/// Cambiarla subito mostrerebbe per un momento uno stato che, se la
/// scrittura fallisce, non è mai esistito.
fn toggle_stage(model: &Model) -> Vec<Cmd> {
    let Some((toggle, path)) = model.stage_toggle() else {
        return Vec::new();
    };
    let path = path.to_string();
    vec![match toggle {
        StageToggle::Stage => Cmd::StageFile(path),
        StageToggle::Unstage => Cmd::UnstageFile(path),
    }]
}

/// Muove di `delta` righe il pannello a fuoco, qualunque sia il suo
/// contenuto: quale lista tocchi lo decide `Model::content_mut`, non questa
/// funzione. Poi Changes e Diff seguono la nuova selezione, se è cambiata:
/// su History Changes passa a un'altra riga, su Changes Diff a un altro
/// file. Un passo che non cambia nulla (sul Diff, o fermo al bordo di una
/// lista) non chiede nulla e non tocca lo scorrimento del diff.
fn step(model: &mut Model, delta: i32) -> Vec<Cmd> {
    let focus = model.focus;
    model.content_mut(focus).step(delta);
    model.follow_selection()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::domain::CommitId;

    fn model_without_git() -> Model {
        let mut model = Model::new();
        model.lose_git();
        model
    }

    fn navigation_read() -> Cmd {
        Cmd::LoadCommitFiles(CommitId("abc".to_string()))
    }

    fn diff_read() -> Cmd {
        Cmd::LoadFileDiff(crate::git::domain::DiffTarget {
            source: crate::git::domain::DiffSource::WorkingCopy,
            path: "a.txt".to_string(),
        })
    }

    /// Una lettura che segue la navigazione, da sola, si scarta in
    /// silenzio; insieme a un'azione chiesta dall'utente si scarta lo
    /// stesso, ma il dialog compare: l'azione dell'utente va rifiutata
    /// dicendolo, qualunque cosa la accompagni.
    #[test]
    fn without_git_a_user_action_is_refused_aloud_even_next_to_a_navigation_read() {
        let mut model = model_without_git();
        assert!(refuse_if_git_is_gone(&mut model, vec![navigation_read(), diff_read()]).is_empty());
        assert!(model.dialog().is_none(), "solo navigazione: nessun dialog");

        for cmds in [
            vec![navigation_read(), Cmd::DeleteBranch("develop".to_string())],
            vec![Cmd::CreateBranch("x".to_string()), navigation_read()],
            vec![diff_read(), Cmd::DeleteBranch("develop".to_string())],
            vec![navigation_read(), Cmd::StageFile("a.txt".to_string())],
            vec![Cmd::UnstageFile("a.txt".to_string()), diff_read()],
            vec![diff_read(), Cmd::Commit("message".to_string())],
        ] {
            let mut model = model_without_git();
            assert!(refuse_if_git_is_gone(&mut model, cmds).is_empty());
            assert!(model.dialog().is_some(), "azione dell'utente: dialog");
        }
    }

    #[test]
    fn with_git_every_cmd_goes_through() {
        let mut model = Model::new();
        let cmds = refuse_if_git_is_gone(&mut model, vec![navigation_read(), Cmd::LoadAll]);
        assert_eq!(cmds.len(), 2);
        assert!(model.dialog().is_none());
    }
}
