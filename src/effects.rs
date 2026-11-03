//
// src/effects.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Esegue i `Cmd` contro il confine `GitRepo` e produce il `Msg` di esito.
//! Questo è l'unico modulo che invoca `GitRepo`.
//!
//! Oggi l'unico backend è `MockGitRepo`, che opera in memoria e risponde in
//! microsecondi: eseguire i comandi in modo sincrono, nello stesso ciclo di
//! `app::run`, è quindi sufficiente e non blocca la UI. Quando arriverà un
//! vero backend Git (vedi roadmap in README.md) questa funzione andrà
//! spostata su un worker thread, inviando il `Msg` di esito tramite canale.

use crate::git::repo::GitRepo;
use crate::msg::{Cmd, Msg};

pub(crate) fn run(cmd: Cmd, repo: &mut dyn GitRepo) -> Msg {
    match cmd {
        Cmd::LoadAll => Msg::Loaded {
            branches: repo.branches(),
            commits: repo.commits(),
            files: repo.files(),
            diff: repo.diff(),
        },
        Cmd::CreateBranch(name) => {
            let result = repo.create_branch(&name);
            Msg::BranchCreated { name, result }
        }
        Cmd::DeleteBranch(name) => {
            let result = repo.delete_branch(&name);
            Msg::BranchDeleted { name, result }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::mock::MockGitRepo;

    #[test]
    fn load_all_reads_every_source_from_the_repo() {
        let mut repo = MockGitRepo::new();
        let msg = run(Cmd::LoadAll, &mut repo);
        let Msg::Loaded {
            branches,
            commits,
            files,
            diff,
        } = msg
        else {
            panic!("expected Msg::Loaded");
        };
        assert_eq!(branches.len(), repo.branches().len());
        assert_eq!(commits.len(), repo.commits().len());
        assert_eq!(files.len(), repo.files().len());
        assert_eq!(diff.len(), repo.diff().len());
    }

    #[test]
    fn create_branch_cmd_carries_the_attempted_name_on_both_outcomes() {
        let mut repo = MockGitRepo::new();

        let msg = run(Cmd::CreateBranch("feature/x".to_string()), &mut repo);
        let Msg::BranchCreated { name, result } = msg else {
            panic!("expected Msg::BranchCreated");
        };
        assert_eq!(name, "feature/x");
        assert!(result.is_ok());

        let msg = run(Cmd::CreateBranch("develop".to_string()), &mut repo);
        let Msg::BranchCreated { name, result } = msg else {
            panic!("expected Msg::BranchCreated");
        };
        assert_eq!(name, "develop");
        assert!(result.is_err());
    }

    #[test]
    fn delete_branch_cmd_removes_it_from_the_repo() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        let msg = run(Cmd::DeleteBranch("develop".to_string()), &mut repo);
        let Msg::BranchDeleted { name, result } = msg else {
            panic!("expected Msg::BranchDeleted");
        };
        assert_eq!(name, "develop");
        assert!(result.is_ok());
        assert_eq!(repo.branches().len(), n - 1);
    }
}
