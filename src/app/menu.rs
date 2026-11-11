//
// src/app/menu.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::msg::{Cmd, Msg};

use super::dialog_flow::{ask_commit, ask_delete_branch};
use super::dialogs;
use super::model::{MenuState, Model, Overlay};

/// Azione associata a una voce di menu. Il dispatch in `activate` avviene su
/// questo tipo, non sul testo della label: rinominare una label non cambia
/// più silenziosamente il comportamento (vedi `activate`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MenuAction {
    OpenRepository,
    CloneRepository,
    Quit,
    NewBranch,
    Checkout,
    DeleteBranch,
    Commit,
    Amend,
    Stash,
    ZoomPanel,
    Refresh,
    About,
}

pub(crate) const MENUS: &[(&str, &[(&str, MenuAction)])] = &[
    (
        "File",
        &[
            ("Open repository...", MenuAction::OpenRepository),
            ("Clone...", MenuAction::CloneRepository),
            ("Quit", MenuAction::Quit),
        ],
    ),
    (
        "Branch",
        &[
            ("New branch...", MenuAction::NewBranch),
            ("Checkout", MenuAction::Checkout),
            ("Delete", MenuAction::DeleteBranch),
        ],
    ),
    (
        "Commit",
        &[
            ("Commit...", MenuAction::Commit),
            ("Amend", MenuAction::Amend),
            ("Stash", MenuAction::Stash),
        ],
    ),
    (
        "View",
        &[
            ("Zoom panel", MenuAction::ZoomPanel),
            ("Refresh", MenuAction::Refresh),
        ],
    ),
    ("Help", &[("About", MenuAction::About)]),
];

pub(super) fn open_menu_by_hotkey(model: &mut Model, c: char) {
    if model.dialog().is_some() {
        return;
    }
    if let Some(i) = MENUS
        .iter()
        .position(|(n, _)| n.starts_with(c.to_ascii_uppercase()))
    {
        let state = MenuState { menu: i, item: 0 };
        match model.overlays.last_mut() {
            Some(Overlay::Menu(m)) => *m = state,
            _ => model.overlays.push(Overlay::Menu(state)),
        }
    }
}

/// Gestisce i tasti mentre il menu è l'overlay in cima: `Esc` e `F10` lo
/// chiudono, le frecce spostano la selezione fra i menu e fra le voci,
/// `Enter` attiva la voce selezionata e chiude il menu. Non restituisce mai
/// un `Cmd`: l'attivazione al più impila un dialog, e l'effetto nasce dopo,
/// quando quel dialog viene accettato.
pub(super) fn update_menu(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let Some(Overlay::Menu(m)) = model.overlays.last_mut() else {
        return Vec::new();
    };
    let Some(&(_, items)) = MENUS.get(m.menu) else {
        return Vec::new();
    };
    match msg {
        Msg::Esc | Msg::F10 => {
            model.overlays.pop();
        }
        Msg::Left => {
            m.menu = prev_index(m.menu, MENUS.len());
            m.item = 0;
        }
        Msg::Right => {
            m.menu = next_index(m.menu, MENUS.len());
            m.item = 0;
        }
        // `items` è il menu corrente: il suo conteggio vale qui, mentre
        // `Left`/`Right` cambiano menu e riportano la voce a zero.
        Msg::Up => m.item = prev_index(m.item, items.len()),
        Msg::Down => m.item = next_index(m.item, items.len()),
        Msg::Enter => {
            let Some(&(label, action)) = items.get(m.item) else {
                return Vec::new();
            };
            // `pop` vuole lo stack in esclusiva: la voce va letta prima.
            model.overlays.pop();
            activate(model, label, action);
        }
        _ => {}
    }
    Vec::new()
}

/// Indice precedente in un elenco circolare di `len` voci; su un elenco
/// vuoto non c'è nessuna voce su cui tornare e resta `0`.
fn prev_index(i: usize, len: usize) -> usize {
    if i == 0 {
        len.saturating_sub(1)
    } else {
        i - 1
    }
}

/// Indice successivo in un elenco circolare di `len` voci; su un elenco
/// vuoto non c'è nessuna voce su cui avanzare e resta `0`.
fn next_index(i: usize, len: usize) -> usize {
    if i + 1 >= len {
        0
    } else {
        i + 1
    }
}

fn activate(model: &mut Model, label: &str, action: MenuAction) {
    match action {
        MenuAction::Quit => model.running = false,
        MenuAction::About => model.overlays.push(Overlay::Dialog(dialogs::about())),
        MenuAction::NewBranch => model.overlays.push(Overlay::Dialog(dialogs::new_branch())),
        MenuAction::DeleteBranch => ask_delete_branch(model),
        MenuAction::Commit => ask_commit(model),
        MenuAction::OpenRepository
        | MenuAction::CloneRepository
        | MenuAction::Checkout
        | MenuAction::Amend
        | MenuAction::Stash
        | MenuAction::ZoomPanel
        | MenuAction::Refresh => {
            model
                .overlays
                .push(Overlay::Dialog(dialogs::not_implemented(label)));
        }
    }
}
