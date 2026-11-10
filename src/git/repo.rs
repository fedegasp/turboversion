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

use super::domain::{Branch, Commit, CommitId, FileEntry};

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
    /// I file della working copy con modifiche non committate.
    fn files(&self) -> Vec<FileEntry>;
    /// I file toccati dal commit `id`: il confronto con il suo primo
    /// parent, o con l'albero vuoto per un commit senza parent. Per un
    /// merge è quindi ciò che il merge ha portato nel ramo su cui è stato
    /// fatto. `staged` è sempre falso: in un commit non c'è un indice.
    ///
    /// A differenza delle altre letture restituisce un errore invece di
    /// una lista vuota: chi chiede un commit preciso deve poter distinguere
    /// "nessun file" da "commit illeggibile".
    fn files_of_commit(&self, id: &CommitId) -> Result<Vec<FileEntry>, GitError>;
    fn diff(&self) -> Vec<String>;
    fn create_branch(&mut self, name: &str) -> Result<Branch, GitError>;
    fn delete_branch(&mut self, name: &str) -> Result<(), GitError>;
}
