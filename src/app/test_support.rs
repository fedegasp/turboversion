//
// src/app/test_support.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use std::collections::VecDeque;

use crate::effects;
use crate::git::repo::Repo;
use crate::msg::{Cmd, Msg};

use super::init_cmds;
use super::model::Model;
use super::update::update;

/// Esegue `cmds` contro `repo` e applica gli esiti, finché nessun esito
/// chiede altro: come l'avvio di `main.rs`, ma nello stesso thread e contro
/// un backend in memoria. È una coda perché un esito può chiedere altri
/// `Cmd` (una rilettura del log che seleziona un commit ne chiede i file).
pub(crate) fn run_to_completion(model: &mut Model, repo: &mut dyn Repo, cmds: Vec<Cmd>) {
    let mut queue: VecDeque<Cmd> = cmds.into();
    while let Some(cmd) = queue.pop_front() {
        let msg = effects::run(cmd, repo);
        queue.extend(update(model, msg));
    }
}

/// Un `Model` popolato con gli stessi dati di un avvio reale (passati
/// attraverso `init_cmds` + `effects::run`, come farebbe `main.rs`), per i
/// test di `app` e di `ui` che hanno bisogno di dati non vuoti.
pub(crate) fn model_for_tests() -> Model {
    let mut model = Model::new();
    let mut repo = crate::git::mock::MockGitRepo::new();
    run_to_completion(&mut model, &mut repo, init_cmds());
    model
}

/// Come `model_for_tests`, ma con una working copy pulita: nessun file
/// modificato, quindi History mostra i soli commit (e Changes i file del
/// primo, che è selezionato).
pub(crate) fn model_without_changes_for_tests() -> Model {
    let mut model = model_for_tests();
    let mut repo = crate::git::mock::MockGitRepo::new();
    let cmds = update(
        &mut model,
        Msg::Loaded {
            branches: repo.branches(),
            commits: repo.commits(),
            files: Vec::new(),
            diff: repo.diff(),
        },
    );
    run_to_completion(&mut model, &mut repo, cmds);
    model
}
