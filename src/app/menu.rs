//
// src/app/menu.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::dialog::{Action, Dialog};
use crate::msg::{Cmd, Msg};

use super::dialog_flow::ask_delete_branch;
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

pub(super) fn update_menu(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let Some(&Overlay::Menu(mut m)) = model.overlays.last() else {
        return Vec::new();
    };
    let n_menus = MENUS.len();
    let n_items = MENUS[m.menu].1.len();
    match msg {
        Msg::Esc | Msg::F10 => {
            model.overlays.pop();
            return Vec::new();
        }
        Msg::Left => {
            m.menu = (m.menu + n_menus - 1) % n_menus;
            m.item = 0;
        }
        Msg::Right => {
            m.menu = (m.menu + 1) % n_menus;
            m.item = 0;
        }
        Msg::Up => m.item = (m.item + n_items - 1) % n_items,
        Msg::Down => m.item = (m.item + 1) % n_items,
        Msg::Enter => {
            model.overlays.pop();
            let (label, action) = MENUS[m.menu].1[m.item];
            activate(model, label, action);
            return Vec::new();
        }
        _ => {}
    }
    *model
        .overlays
        .last_mut()
        .expect("il top era un Menu un istante fa") = Overlay::Menu(m);
    Vec::new()
}

fn activate(model: &mut Model, label: &str, action: MenuAction) {
    match action {
        MenuAction::Quit => model.running = false,
        MenuAction::About => {
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "About",
                "TurboVersion\nTurbo Vision meets version control.",
            )));
        }
        MenuAction::NewBranch => {
            model.overlays.push(Overlay::Dialog(Dialog::input(
                "New branch",
                "Branch name:",
                Action::NewBranch,
            )));
        }
        MenuAction::DeleteBranch => ask_delete_branch(model),
        MenuAction::OpenRepository
        | MenuAction::CloneRepository
        | MenuAction::Checkout
        | MenuAction::Commit
        | MenuAction::Amend
        | MenuAction::Stash
        | MenuAction::ZoomPanel
        | MenuAction::Refresh => {
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "Not implemented",
                format!("\"{label}\" is not available yet."),
            )));
        }
    }
}
