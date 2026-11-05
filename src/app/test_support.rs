//
// src/app/test_support.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use crate::effects;
use crate::msg::Msg;

use super::init_cmds;
use super::model::Model;
use super::update::update;

/// Un `Model` popolato con gli stessi dati di un avvio reale (passati
/// attraverso `init_cmds` + `effects::run`, come farebbe `main.rs`), per i
/// test di `app` e di `ui` che hanno bisogno di dati non vuoti.
pub(crate) fn model_for_tests() -> Model {
    let mut model = Model::new();
    let mut repo = crate::git::mock::MockGitRepo::new();
    for cmd in init_cmds() {
        let msg = effects::run(cmd, &mut repo);
        update(&mut model, msg);
    }
    model
}

/// Come `model_for_tests`, ma con una working copy pulita: nessun file
/// modificato, quindi History mostra i soli commit.
pub(crate) fn model_without_changes_for_tests() -> Model {
    use crate::git::repo::Repo;

    let mut model = model_for_tests();
    let repo = crate::git::mock::MockGitRepo::new();
    update(
        &mut model,
        Msg::Loaded {
            branches: repo.branches(),
            commits: repo.commits(),
            files: Vec::new(),
            diff: repo.diff(),
        },
    );
    model
}
