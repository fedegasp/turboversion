//
// src/effects.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Esegue i `Cmd` contro il confine `Repo` e produce il `Msg` di esito.
//! Questo è l'unico modulo che invoca `Repo`.
//!
//! `run` resta sincrona e non sa nulla di thread: in produzione (backend
//! `git::git2::GitRepo`, che può bloccare su I/O) è `worker::Worker` a
//! chiamarla da un thread dedicato, fuori dal thread UI (vedi `worker.rs`).
//! Nei test (`MockGitRepo`, in memoria, risposta immediata) `TestApp`/
//! `model_for_tests` in `src/app/` la chiamano invece direttamente, nello
//! stesso thread, per restare deterministici e non dipendere dal timing di
//! un thread in background.

use crate::git::repo::Repo;
use crate::msg::{Cmd, Msg};

/// Un `Cmd` entra, un `Msg` esce: sempre uno, mai zero né due. Su questo
/// si regge il conto dei `Cmd` in volo che il ciclo principale usa per
/// decidere quanto attendere (vedi `app::run`); se un giorno il tipo di
/// ritorno diventasse `Vec<Msg>`, è quel conto il punto che si rompe.
pub(crate) fn run(cmd: Cmd, repo: &mut dyn Repo) -> Msg {
    match cmd {
        Cmd::LoadAll => Msg::Loaded {
            branches: repo.branches(),
            commits: repo.commits(),
            files: repo.files(),
        },
        Cmd::LoadCommitFiles(id) => {
            let result = repo.files_of_commit(&id);
            Msg::CommitFilesLoaded { id, result }
        }
        Cmd::LoadFileDiff(target) => {
            let result = repo.file_diff(&target);
            Msg::FileDiffLoaded { target, result }
        }
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
        } = msg
        else {
            panic!("expected Msg::Loaded");
        };
        assert_eq!(branches.len(), repo.branches().len());
        assert_eq!(commits.len(), repo.commits().len());
        assert_eq!(files.len(), repo.files().len());
    }

    #[test]
    fn load_file_diff_carries_the_requested_target_on_both_outcomes() {
        use crate::git::domain::{CommitId, DiffSource, DiffTarget, FileDiff};

        let mut repo = MockGitRepo::new();
        let known = DiffTarget {
            source: DiffSource::WorkingCopy,
            path: repo.files()[0].path.clone(),
        };
        let Msg::FileDiffLoaded { target, result } =
            run(Cmd::LoadFileDiff(known.clone()), &mut repo)
        else {
            panic!("expected Msg::FileDiffLoaded");
        };
        assert_eq!(target, known);
        assert!(matches!(result, Ok(FileDiff::Text(lines)) if !lines.is_empty()));

        let unknown = DiffTarget {
            source: DiffSource::Commit(CommitId("nope".to_string())),
            path: "x".to_string(),
        };
        let Msg::FileDiffLoaded { target, result } =
            run(Cmd::LoadFileDiff(unknown.clone()), &mut repo)
        else {
            panic!("expected Msg::FileDiffLoaded");
        };
        assert_eq!(target, unknown);
        assert!(result.is_err());
    }

    #[test]
    fn load_commit_files_carries_the_requested_id_on_both_outcomes() {
        let mut repo = MockGitRepo::new();
        let known = repo.commits()[1].id.clone();

        let msg = run(Cmd::LoadCommitFiles(known.clone()), &mut repo);
        let Msg::CommitFilesLoaded { id, result } = msg else {
            panic!("expected Msg::CommitFilesLoaded");
        };
        assert_eq!(id, known);
        assert_eq!(result.map(|f| f.len()).ok(), Some(2));

        let unknown = crate::git::domain::CommitId("nope".to_string());
        let msg = run(Cmd::LoadCommitFiles(unknown.clone()), &mut repo);
        let Msg::CommitFilesLoaded { id, result } = msg else {
            panic!("expected Msg::CommitFilesLoaded");
        };
        assert_eq!(id, unknown);
        assert!(result.is_err());
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
