//
// src/app/results.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Applica al `Model` gli esiti dei `Cmd`: i `Msg` che il `Worker` rimanda
//! dopo aver interrogato `Repo`.
//!
//! Regola per i prossimi `apply_*`: qui ci sta la risposta a un `Msg`
//! prodotto dal worker in esito a un `Cmd` — asincrono, fallibile, con
//! dentro dati letti dal repository. Un `Msg` che nasce invece dentro il
//! processo della UI (un tasto, una misura del layout) si gestisce nel file
//! che lo interpreta — `update.rs` per i tasti dei pannelli,
//! `dialog_flow.rs` per quelli dei dialog, `menu.rs` per il menu — dove
//! l'effetto è immediato: sul `Model`, o sul `Cmd` da restituire.

use crate::git::domain::{Branch, BranchKind, Commit, CommitId, DiffTarget, FileDiff, FileEntry};
use crate::git::repo::GitError;
use crate::msg::Cmd;

use super::dialogs;
use super::model::{Model, Overlay};

/// Applica una lettura completa del repository. Restituisce le letture che
/// la nuova selezione richiede: i file di un commit quando History seleziona
/// un commit che Changes non sta già mostrando (al primo caricamento di una
/// working copy pulita, o quando il commit selezionato prima non c'è più),
/// e il diff del file selezionato in Changes. Il diff di un file della
/// working copy si rilegge sempre, anche se il file è lo stesso: è appena
/// stato riletto il disco, e quel diff può essere cambiato.
pub(super) fn apply_loaded(
    model: &mut Model,
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    files: Vec<FileEntry>,
) -> Vec<Cmd> {
    let (local, remote) = split_branches(branches);
    model.local_branches.set(local);
    model.remote_branches.set(remote);
    // `files` è ciò che `Repo::files` ha letto dalla working copy: è qui,
    // al caricamento, che si decide se ci sono modifiche non committate.
    // Chiederlo più tardi al pannello Changes legherebbe la riga in cima a
    // History a ciò che quel pannello sta mostrando, che può essere un
    // commit.
    let working_copy_dirty = !files.is_empty();
    model.working_copy_files.set(files);
    model.set_history(commits, working_copy_dirty);
    model.forget_working_copy_diff();
    model.follow_selection()
}

/// Applica l'esito della lettura dei file di un commit, se il pannello
/// Changes lo sta ancora aspettando; altrimenti è obsoleto e si scarta.
///
/// Un errore non apre un dialog: si mostra dentro il pannello, al posto
/// della lista. Un dialog catturerebbe le frecce di chi sta scorrendo
/// History, proprio mentre si allontana dal commit illeggibile.
///
/// Restituisce le letture che l'esito rende possibili o necessarie: il diff
/// del primo file appena arrivato, oppure la lettura voluta mentre questa
/// era in volo (vedi `Model::follow_selection`).
pub(super) fn apply_commit_files(
    model: &mut Model,
    id: CommitId,
    result: Result<Vec<FileEntry>, GitError>,
) -> Vec<Cmd> {
    model.receive_commit_files(&id, result.map_err(|err| err.to_string()))
}

/// Applica l'esito della lettura del diff di un file, se il pannello Diff
/// lo sta ancora aspettando; altrimenti è obsoleto e si scarta. Come per i
/// file di un commit, un errore si mostra nel pannello e non in un dialog.
/// Restituisce la lettura voluta mentre questa era in volo, se c'è.
pub(super) fn apply_file_diff(
    model: &mut Model,
    target: DiffTarget,
    result: Result<FileDiff, GitError>,
) -> Vec<Cmd> {
    model.receive_file_diff(&target, result.map_err(|err| err.to_string()))
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
            model
                .overlays
                .push(Overlay::Dialog(dialogs::new_branch_retry(name, &err)));
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
            model
                .overlays
                .push(Overlay::Dialog(dialogs::delete_branch_failed(&err)));
        }
    }
}
