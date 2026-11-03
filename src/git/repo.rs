//
// src/git/repo.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Confine verso git: un solo trait, `GitRepo`, con tipi di ritorno di
//! dominio (mai tipi di un backend esterno). Oggi l'unica implementazione è
//! `git::mock::MockGitRepo`; un backend reale (es. basato su `git2`, vedi
//! roadmap in README.md) implementerebbe lo stesso trait senza che
//! `app`/`ui` debbano cambiare.

use std::fmt;

use super::domain::{Branch, Commit, FileEntry};

/// Errore di un'operazione `GitRepo`, con un messaggio già pronto per
/// essere mostrato in un dialog. Questo prototipo ha un solo backend
/// (mock) che non ha bisogno di varianti più strutturate; un backend reale
/// potrebbe arricchire questo tipo quando ne emergerà il bisogno.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitError(pub(crate) String);

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Confine unico verso git: letture (branch/commit/file/diff) e le due
/// operazioni mutanti supportate da questo prototipo.
pub(crate) trait GitRepo {
    fn branches(&self) -> Vec<Branch>;
    fn commits(&self) -> Vec<Commit>;
    fn files(&self) -> Vec<FileEntry>;
    fn diff(&self) -> Vec<String>;
    fn create_branch(&mut self, name: &str) -> Result<Branch, GitError>;
    fn delete_branch(&mut self, name: &str) -> Result<(), GitError>;
}
