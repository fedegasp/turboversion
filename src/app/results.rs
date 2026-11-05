//
// src/app/results.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::dialog::{Action, Dialog};
use crate::git::domain::{Branch, BranchKind, Commit, FileEntry};
use crate::git::repo::GitError;

use super::model::{Model, Overlay};

pub(super) fn apply_loaded(
    model: &mut Model,
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    files: Vec<FileEntry>,
    diff: Vec<String>,
) {
    let (local, remote) = split_branches(branches);
    model.local_branches.set(local);
    model.remote_branches.set(remote);
    // `files` è ciò che `Repo::files` ha letto dalla working copy: è qui,
    // al caricamento, che si decide se ci sono modifiche non committate.
    // Chiederlo più tardi al pannello Changes legherebbe la riga in cima a
    // History a ciò che quel pannello sta mostrando.
    let working_copy_dirty = !files.is_empty();
    model.files.set(files);
    model.set_history(commits, working_copy_dirty);
    model.diff = diff;
    model.diff_scroll = 0;
}

/// Divide i riferimenti restituiti da `Repo::branches` nelle due liste che
/// la UI mostra. I tag esistono nel dominio e i backend continuano a
/// restituirli (la validazione dei nomi duplicati si appoggia alla lista
/// completa), ma non hanno un pannello: qui vengono scartati.
fn split_branches(branches: Vec<Branch>) -> (Vec<Branch>, Vec<Branch>) {
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for b in branches {
        match b.kind {
            BranchKind::Local => local.push(b),
            BranchKind::Remote => remote.push(b),
            BranchKind::Tag => {}
        }
    }
    (local, remote)
}

pub(super) fn apply_branch_created(
    model: &mut Model,
    name: String,
    result: Result<Branch, GitError>,
) {
    match result {
        Ok(branch) => {
            // Aggiornamento locale invece di una ricarica dal
            // repository: il ramo finisce in coda ai locali, mentre una
            // rilettura lo collocherebbe nell'ordine in cui il repository
            // elenca i suoi riferimenti. È una divergenza solo di
            // ordinamento, che sparirà quando l'esito Ok potrà chiedere una
            // ricarica completa.
            model.local_branches.push_selected(branch);
        }
        Err(err) => {
            // Riapre il dialog con il testo già digitato, invece di un
            // messaggio d'errore che lo scarterebbe e costringerebbe a
            // ridigitarlo da capo.
            let mut d = Dialog::input(
                "New branch",
                format!("Branch name:\n{err}"),
                Action::NewBranch,
            );
            d.input = name;
            model.overlays.push(Overlay::Dialog(d));
        }
    }
}

pub(super) fn apply_branch_deleted(model: &mut Model, name: String, result: Result<(), GitError>) {
    match result {
        Ok(()) => {
            // Aggiornamento locale invece di una ricarica: la lista
            // contiene solo rami locali, e i loro nomi sono unici, quindi
            // il confronto per nome toglie esattamente il ramo cancellato.
            model.local_branches.retain(|b| b.name != name);
        }
        Err(err) => {
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "Delete branch",
                err.to_string(),
            )));
        }
    }
}
