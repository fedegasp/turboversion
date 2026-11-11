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

use super::domain::{Branch, Commit, CommitId, DiffTarget, FileDiff, FileEntry};

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

/// Confine unico verso git: letture (rami, commit, file, diff di un file)
/// e le operazioni mutanti supportate da questo prototipo (rami, stage,
/// commit).
pub(crate) trait Repo {
    fn branches(&self) -> Vec<Branch>;
    fn commits(&self) -> Vec<Commit>;
    /// I file della working copy con modifiche non committate.
    fn files(&self) -> Vec<FileEntry>;
    /// I file toccati dal commit `id`: il confronto con il suo primo
    /// parent, o con l'albero vuoto per un commit senza parent. Per un
    /// merge è quindi ciò che il merge ha portato nel ramo su cui è stato
    /// fatto. `staging` è sempre `Staging::Unstaged`: in un commit non c'è
    /// un indice.
    ///
    /// A differenza delle altre letture restituisce un errore invece di
    /// una lista vuota: chi chiede un commit preciso deve poter distinguere
    /// "nessun file" da "commit illeggibile".
    fn files_of_commit(&self, id: &CommitId) -> Result<Vec<FileEntry>, GitError>;
    /// Il diff del solo file `target.path`, nel contesto `target.source`:
    /// per la working copy dall'ultimo commit ai file su disco passando per
    /// l'indice (come `git diff HEAD`), per un commit verso il suo primo
    /// parent (o l'albero vuoto), come `files_of_commit`.
    ///
    /// Come `files_of_commit` restituisce un errore invece di un diff
    /// vuoto: "niente da mostrare" e "illeggibile" vanno distinti.
    fn file_diff(&self, target: &DiffTarget) -> Result<FileDiff, GitError>;
    fn create_branch(&mut self, name: &str) -> Result<Branch, GitError>;
    fn delete_branch(&mut self, name: &str) -> Result<(), GitError>;
    /// Mette nell'indice lo stato su disco di `path`, come `git add`: un
    /// file modificato o non tracciato entra com'è, una directory non
    /// tracciata (`dir/`) con tutti i file che contiene, un file cancellato
    /// esce dall'indice (cioè la cancellazione entra nel prossimo commit).
    /// Un file messo in stage solo in parte vi entra per intero.
    fn stage_file(&mut self, path: &str) -> Result<(), GitError>;
    /// Riporta `path` nell'indice com'è nell'ultimo commit, come
    /// `git restore --staged`: le modifiche su disco restano. Un file che
    /// nell'ultimo commit non c'è (aggiunto, o qualunque file prima del
    /// primo commit) esce dall'indice e torna non tracciato.
    fn unstage_file(&mut self, path: &str) -> Result<(), GitError>;
    /// Crea un commit con il contenuto dell'indice e il messaggio dato, su
    /// HEAD; prima del primo commit crea il commit radice. Rifiuta un
    /// messaggio vuoto e un indice uguale all'ultimo commit, invece di
    /// creare un commit senza modifiche.
    fn commit(&mut self, message: &str) -> Result<CommitId, GitError>;
}
