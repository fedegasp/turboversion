//
// src/app/dialogs.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Costruzione dei dialog che l'applicazione sa mostrare: una funzione per
//! dialog, che ne fissa titolo, testo, tipo, `Action` e pulsante di default.
//! È l'unico posto in cui un `Dialog` viene costruito, così i testi mostrati
//! all'utente si leggono tutti insieme e un cambio di forma dei dialog tocca
//! un file solo.
//!
//! Qui non si decide *quale* dialog mostrare: quella scelta (il caso che si
//! applica, le guardie sul focus o sulla selezione) resta a chi chiama, che
//! si limita a impilare il `Dialog` ricevuto.

use crate::dialog::{Action, Dialog};
use crate::git::repo::GitError;

/// Titolo condiviso da tutti i dialog della cancellazione di un ramo,
/// falliti o rifiutati: l'utente ha chiesto una cosa sola. Una
/// cancellazione riuscita non apre alcun dialog, aggiorna la lista.
const DELETE_BRANCH: &str = "Delete branch";

pub(super) fn about() -> Dialog {
    Dialog::info("About", "TurboVersion\nTurbo Vision meets version control.")
}

/// Voce di menu riconosciuta ma non ancora collegata a nulla. `label` è il
/// testo della voce, così il messaggio nomina ciò che l'utente ha premuto.
pub(super) fn not_implemented(label: &str) -> Dialog {
    Dialog::info(
        "Not implemented",
        format!("\"{label}\" is not available yet."),
    )
}

/// Le operazioni git non sono più possibili: il thread che le esegue non
/// c'è più. Il testo dice cosa smette di funzionare e cosa continua a
/// funzionare — i dati già letti restano a schermo e si possono navigare
/// — quindi non c'è motivo di chiudere l'applicazione di colpo né di
/// allarmare chi legge.
///
/// Uno stesso dialog per due momenti: l'annuncio della perdita, e il
/// rifiuto di un'operazione chiesta dopo. La cosa da dire è la stessa, e
/// due testi diversi per la stessa condizione la farebbero sembrare due
/// condizioni diverse.
pub(super) fn git_unavailable() -> Dialog {
    Dialog::info(
        "Git unavailable",
        "Git operations are no longer available in this session.\n\
         What is already loaded stays on screen and can still be browsed.\n\
         Restart TurboVersion to use git again.",
    )
}

pub(super) fn new_branch() -> Dialog {
    Dialog::input("New branch", "Branch name:", Action::NewBranch)
}

/// Lo stesso dialog di `new_branch`, riaperto dopo un tentativo fallito con
/// l'errore in calce al prompt e il testo già digitato ancora nel campo: un
/// messaggio d'errore separato lo scarterebbe e costringerebbe a ridigitarlo
/// da capo.
pub(super) fn new_branch_retry(typed: String, error: &GitError) -> Dialog {
    let mut d = Dialog::input(
        "New branch",
        format!("Branch name:\n{error}"),
        Action::NewBranch,
    );
    d.input = typed;
    d
}

/// Conferma della cancellazione di un ramo. È un'azione distruttiva: il
/// pulsante di default è "No", quindi un `Enter` battuto per inerzia non
/// cancella nulla.
pub(super) fn delete_branch_confirm(name: &str) -> Dialog {
    let mut d = Dialog::confirm(
        DELETE_BRANCH,
        format!("Delete branch '{name}'?"),
        Action::DeleteBranch(name.to_string()),
    );
    d.button = 1;
    d
}

pub(super) fn delete_branch_refused_remote() -> Dialog {
    Dialog::info(DELETE_BRANCH, "Remote branches cannot be deleted here.")
}

pub(super) fn delete_branch_none_selected() -> Dialog {
    Dialog::info(DELETE_BRANCH, "No branch selected.")
}

pub(super) fn delete_branch_refused_current(name: &str) -> Dialog {
    Dialog::info(
        DELETE_BRANCH,
        format!("Cannot delete the current branch '{name}'."),
    )
}

pub(super) fn delete_branch_failed(error: &GitError) -> Dialog {
    Dialog::info(DELETE_BRANCH, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::Kind;

    #[test]
    fn delete_confirmation_defaults_to_no_and_carries_the_branch_name() {
        let d = delete_branch_confirm("develop");
        assert_eq!(d.kind, Kind::Confirm);
        assert_eq!(d.buttons()[d.button], "No");
        assert_eq!(d.action, Action::DeleteBranch("develop".into()));
    }

    #[test]
    fn retrying_a_new_branch_keeps_the_typed_text_and_shows_the_error() {
        let d = new_branch_retry("bad name".into(), &GitError("nope".into()));
        assert_eq!(d.kind, Kind::Input);
        assert_eq!(d.input, "bad name");
        assert!(d.text.contains("nope"));
    }
}
