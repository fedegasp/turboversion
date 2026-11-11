//
// src/app/dialog_flow.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::dialog::{Action, Dialog};
use crate::git::domain::{BranchKind, Staging};
use crate::git::validate::validate_commit_message;
use crate::msg::{Cmd, Msg};

use super::dialogs;
use super::model::{Model, Overlay};

pub(super) fn update_dialog(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let Some(Overlay::Dialog(dialog)) = model.overlays.last_mut() else {
        return Vec::new();
    };
    let Some(accepted) = dialog.on_msg(msg) else {
        return Vec::new();
    };
    // Il dialog esce comunque dallo stack; un overlay di altro tipo in cima
    // non produce effetti.
    let Some(Overlay::Dialog(dialog)) = model.overlays.pop() else {
        return Vec::new();
    };
    if accepted {
        apply_dialog(model, dialog)
    } else {
        Vec::new()
    }
}

/// Chiede conferma per cancellare un ramo. Il ramo su cui l'azione agisce
/// è quello che `Model::target_branch` indica: la selezione del pannello a
/// fuoco quando elenca rami, quella di "Branches" altrimenti, come è
/// normale per una voce di menu azionata con il focus da un'altra parte.
///
/// La risposta dipende poi solo da quel ramo, non da dove si trova il
/// focus: un remoto non si cancella da qui, il ramo corrente nemmeno, e
/// senza alcun ramo puntato non c'è niente da cancellare. Una scorciatoia
/// contestuale che chiamasse questa funzione otterrebbe quindi la stessa
/// decisione, senza doverla ripensare.
pub(super) fn ask_delete_branch(model: &mut Model) {
    let dialog = match model.target_branch() {
        None => dialogs::delete_branch_none_selected(),
        // I tag non compaiono in nessun pannello, quindi non possono
        // essere il ramo puntato; stanno con i remoti perché come loro
        // non sono rami locali, gli unici che l'app cancella.
        Some(b) => match b.kind {
            BranchKind::Remote | BranchKind::Tag => dialogs::delete_branch_refused_remote(),
            BranchKind::Local if b.current => dialogs::delete_branch_refused_current(&b.name),
            BranchKind::Local => dialogs::delete_branch_confirm(&b.name),
        },
    };
    model.overlays.push(Overlay::Dialog(dialog));
}

/// Apre il dialog del messaggio di commit, o dice subito che non c'è nulla
/// da committare. Lo stage si guarda nei file della working copy già letti,
/// anche se Changes sta mostrando un commit: il commit si fa sempre dalla
/// working copy. Il backend rifà comunque il controllo sull'indice vero,
/// che un altro client può aver cambiato.
pub(super) fn ask_commit(model: &mut Model) {
    let anything_staged = model
        .working_copy_files
        .items()
        .iter()
        .any(|f| f.staging != Staging::Unstaged);
    let dialog = if anything_staged {
        dialogs::commit()
    } else {
        dialogs::nothing_to_commit()
    };
    model.overlays.push(Overlay::Dialog(dialog));
}

/// Traduce l'accettazione di un dialog nell'effetto da richiedere, se c'è.
fn apply_dialog(model: &mut Model, dialog: Dialog) -> Vec<Cmd> {
    match dialog.action {
        Action::Info => Vec::new(),
        Action::NewBranch => vec![Cmd::CreateBranch(dialog.input.trim().to_string())],
        Action::DeleteBranch(name) => vec![Cmd::DeleteBranch(name)],
        Action::Commit => {
            // Un messaggio vuoto si rifiuta qui, con la regola del backend:
            // il dialog si riapre con l'errore senza passare dal worker.
            if let Some(err) = validate_commit_message(&dialog.input) {
                model
                    .overlays
                    .push(Overlay::Dialog(dialogs::commit_retry(dialog.input, &err)));
                return Vec::new();
            }
            vec![Cmd::Commit(dialog.input.trim().to_string())]
        }
    }
}
