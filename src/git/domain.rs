//
// src/git/domain.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Tipi di dominio: la vista "git" che il resto dell'app conosce, mai
//! legata a un backend particolare (oggi `git::mock::MockGitRepo` e
//! `git::git2::GitRepo`, entrambi dietro il trait `git::repo::Repo`).

/// Tipo di riferimento: un ramo locale, un ramo remote-tracking o un tag.
/// Solo i rami locali possono essere cancellati dall'app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BranchKind {
    Local,
    Remote,
    Tag,
}

#[derive(Clone, Debug)]
pub(crate) struct Branch {
    pub(crate) name: String,
    pub(crate) kind: BranchKind,
    pub(crate) current: bool,
}

/// L'identità di un commit: l'hash completo, così come il backend lo
/// conosce. È ciò con cui si torna a chiedere qualcosa su quel commit
/// (`Repo::files_of_commit`) e ciò con cui lo si ritrova in una lista
/// riletta; l'hash abbreviato di `Commit::hash` serve solo a mostrarlo, e
/// in un repository grande non è garantito che sia univoco.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CommitId(pub(crate) String);

impl CommitId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Commit {
    pub(crate) id: CommitId,
    pub(crate) graph: String,
    /// L'hash abbreviato, da mostrare. Per ritrovare il commit c'è `id`.
    pub(crate) hash: String,
    pub(crate) msg: String,
    pub(crate) author: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FileStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
}

impl FileStatus {
    /// Lettera di stato in stile `git status --short`.
    pub(crate) const fn label(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Untracked => '?',
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) status: FileStatus,
    pub(crate) staged: bool,
}
