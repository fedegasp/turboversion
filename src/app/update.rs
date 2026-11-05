//
// src/app/update.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::msg::{Cmd, Msg};

use super::dialog_flow::update_dialog;
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
        key_msg => dispatch_key(model, key_msg),
    }
}

fn dispatch_key(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    // Oggi un solo overlay è mai attivo: se questo smette di essere vero,
    // "cima dello stack" e "overlay attivo" divergerebbero silenziosamente.
    debug_assert!(model.overlays.len() <= 1, "più di un overlay impilato");
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
fn apply_diff_viewport(model: &mut Model, height: u16) {
    model.diff_viewport = height;
    model.diff_scroll = model
        .diff_scroll
        .min(max_diff_scroll(model.diff.len(), height));
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
