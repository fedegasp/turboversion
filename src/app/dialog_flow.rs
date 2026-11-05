//
// src/app/dialog_flow.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::dialog::{Action, Dialog};
use crate::msg::{Cmd, Msg};

use super::model::{Model, Overlay};
use super::panel::Panel;

pub(super) fn update_dialog(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let Some(Overlay::Dialog(dialog)) = model.overlays.last_mut() else {
        return Vec::new();
    };
    let Some(accepted) = dialog.on_msg(msg) else {
        return Vec::new();
    };
    let Some(Overlay::Dialog(dialog)) = model.overlays.pop() else {
        unreachable!("appena verificato Some(Overlay::Dialog(_)) poche righe sopra")
    };
    if accepted {
        apply_dialog(dialog)
    } else {
        Vec::new()
    }
}

/// Chiede conferma per cancellare il ramo selezionato nel pannello
/// "Branches". L'operazione guarda sempre quella selezione, anche con il
/// focus altrove, come è normale per una voce di menu. L'eccezione è
/// "Remotes": è l'altro pannello che elenca rami, quindi è l'unico in cui
/// "Delete" sarebbe ambiguo su quale dei due elenchi stia agendo. Lì si
/// rifiuta invece di cancellare un ramo a cui l'utente non stava pensando.
pub(super) fn ask_delete_branch(model: &mut Model) {
    if model.focus == Panel::Remotes {
        model.overlays.push(Overlay::Dialog(Dialog::info(
            "Delete branch",
            "Remote branches cannot be deleted here.",
        )));
        return;
    }
    let Some(b) = model.local_branches.selected() else {
        model.overlays.push(Overlay::Dialog(Dialog::info(
            "Delete branch",
            "No branch selected.",
        )));
        return;
    };
    let dialog = if b.current {
        Dialog::info(
            "Delete branch",
            format!("Cannot delete the current branch '{}'.", b.name),
        )
    } else {
        let mut d = Dialog::confirm(
            "Delete branch",
            format!("Delete branch '{}'?", b.name),
            Action::DeleteBranch(b.name.clone()),
        );
        d.button = 1; // azione distruttiva: default su "No"
        d
    };
    model.overlays.push(Overlay::Dialog(dialog));
}

/// Traduce l'accettazione di un dialog nell'effetto da richiedere, se c'è.
fn apply_dialog(dialog: Dialog) -> Vec<Cmd> {
    match dialog.action {
        Action::Info => Vec::new(),
        Action::NewBranch => vec![Cmd::CreateBranch(dialog.input.trim().to_string())],
        Action::DeleteBranch(name) => vec![Cmd::DeleteBranch(name)],
    }
}
