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

#[derive(Clone, Debug)]
pub(crate) struct Commit {
    pub(crate) graph: String,
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
