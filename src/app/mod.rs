//
// src/app/mod.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! `Model` (stato dell'applicazione) e `update` (`Model`, `Msg` -> `Vec<Cmd>`).
//! `update` è puro: nessun I/O, nessun rendering, nessun `unwrap`. Le
//! operazioni verso git passano da un `Cmd` restituito qui, inviato al
//! `Worker` (vedi `worker.rs`) il cui `Msg` di esito torna, più tardi, a un
//! giro successivo di `run`.

mod dialog_flow;
mod dialogs;
mod list_panel;
mod menu;
mod model;
mod panel;
mod results;
mod run;
mod update;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use menu::MenuAction;
pub(crate) use menu::MENUS;
#[cfg(test)]
pub(crate) use model::Overlay;
pub(crate) use model::{ChangesNotice, HistoryEntry, MenuState, Model, PanelContent};
pub(crate) use panel::Panel;
pub(crate) use run::run;
#[cfg(test)]
pub(crate) use test_support::{model_for_tests, model_without_changes_for_tests};
pub(crate) use update::update;

use crate::msg::Cmd;

/// `Cmd` da eseguire all'avvio, prima del primo frame (vedi `main.rs`).
pub(crate) fn init_cmds() -> Vec<Cmd> {
    vec![Cmd::LoadAll]
}
