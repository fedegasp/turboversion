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
use super::model::{max_diff_scroll, MenuState, Model, Overlay};
use super::panel::Panel;
use super::results::{apply_branch_created, apply_branch_deleted, apply_loaded};

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
            diff,
        } => {
            apply_loaded(model, branches, commits, files, diff);
            Vec::new()
        }
        Msg::BranchCreated { name, result } => {
            apply_branch_created(model, name, result);
            Vec::new()
        }
        Msg::BranchDeleted { name, result } => {
            apply_branch_deleted(model, name, result);
            Vec::new()
        }
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
fn refuse_if_git_is_gone(model: &mut Model, cmds: Vec<Cmd>) -> Vec<Cmd> {
    if cmds.is_empty() || model.git_available() {
        return cmds;
    }
    model
        .overlays
        .push(Overlay::Dialog(dialogs::git_unavailable()));
    Vec::new()
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
        .min(max_diff_scroll(model.diff.len(), height));
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
        Msg::Up | Msg::Char { c: 'k', .. } => step(model, -1),
        Msg::Down | Msg::Char { c: 'j', .. } => step(model, 1),
        Msg::Char { c: ' ', .. } if model.focus == Panel::Changes => {
            // Su una lista vuota non c'è alcun file selezionato (vedi
            // l'invariante di `ListPanel`): niente da commutare.
            if let Some(file) = model.files.selected_mut() {
                file.staged = !file.staged;
            }
        }
        _ => {}
    }
    Vec::new()
}

/// Muove di `delta` righe il pannello a fuoco, qualunque sia il suo
/// contenuto: quale lista tocchi lo decide `Model::content_mut`, non questa
/// funzione.
fn step(model: &mut Model, delta: i32) {
    let focus = model.focus;
    if focus == Panel::History {
        // Una nuova selezione mostra un commit diverso: lo scroll del diff
        // precedente non ha più senso.
        model.diff_scroll = 0;
    }
    model.content_mut(focus).step(delta);
}
