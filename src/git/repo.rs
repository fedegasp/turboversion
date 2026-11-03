//
// src/git/repo.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Confine verso git: un solo trait, `Repo`, con tipi di ritorno di dominio
//! (mai tipi di un backend esterno). Oggi ci sono due implementazioni:
//! `git::mock::MockGitRepo` (dati finti in memoria, usata dai test) e
//! `git::git2::GitRepo` (repository reale, via `git2`, usata in
//! produzione); entrambe implementano lo stesso trait senza che `app`/`ui`
//! debbano saperne la differenza.

use std::fmt;

use super::domain::{Branch, Commit, FileEntry};

/// Errore di un'operazione `Repo`, con un messaggio già pronto per essere
/// mostrato in un dialog. Oggi un messaggio di testo semplice basta a
/// entrambi i backend; varianti più strutturate potranno arricchire questo
/// tipo quando ne emergerà il bisogno.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitError(pub(crate) String);

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Confine unico verso git: letture (branch/commit/file/diff) e le due
/// operazioni mutanti supportate da questo prototipo.
pub(crate) trait Repo {
    fn branches(&self) -> Vec<Branch>;
    fn commits(&self) -> Vec<Commit>;
    fn files(&self) -> Vec<FileEntry>;
    fn diff(&self) -> Vec<String>;
    fn create_branch(&mut self, name: &str) -> Result<Branch, GitError>;
    fn delete_branch(&mut self, name: &str) -> Result<(), GitError>;
}
