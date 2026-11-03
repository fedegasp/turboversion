//
// src/mock.rs
// TurboVersion
//
// 2026-10-30 Federico Gasperini
//

//! Dati finti per sviluppare la TUI senza dipendere da git.

/// Tipo di riferimento: un ramo locale, un ramo remote-tracking o un tag.
/// Solo i rami locali possono essere cancellati dall'app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BranchKind {
    Local,
    Remote,
    Tag,
}

pub(crate) struct Branch {
    pub(crate) name: String,
    pub(crate) kind: BranchKind,
    pub(crate) current: bool,
}

pub(crate) fn branches() -> Vec<Branch> {
    [
        ("main", BranchKind::Local, true),
        ("develop", BranchKind::Local, false),
        ("feature/menu", BranchKind::Local, false),
        ("origin/main", BranchKind::Remote, false),
        ("origin/develop", BranchKind::Remote, false),
        ("v0.1.0", BranchKind::Tag, false),
    ]
    .into_iter()
    .map(|(name, kind, current)| Branch {
        name: name.to_string(),
        kind,
        current,
    })
    .collect()
}

#[derive(Clone, Copy)]
pub(crate) struct Commit {
    pub(crate) graph: &'static str,
    pub(crate) hash: &'static str,
    pub(crate) msg: &'static str,
    pub(crate) author: &'static str,
}

pub(crate) const COMMITS: &[Commit] = &[
    Commit {
        graph: "●─╮",
        hash: "a1b2c3d",
        msg: "Merge feature/menu",
        author: "Mario",
    },
    Commit {
        graph: "│ ●",
        hash: "d4e5f6a",
        msg: "Add dropdown menu",
        author: "Luca",
    },
    Commit {
        graph: "│ ●",
        hash: "9f8e7d6",
        msg: "Add menubar widget",
        author: "Luca",
    },
    Commit {
        graph: "●─╯",
        hash: "c0ffee1",
        msg: "Fix status bar",
        author: "Mario",
    },
    Commit {
        graph: "●  ",
        hash: "b4dc0de",
        msg: "Window shadow",
        author: "Mario",
    },
    Commit {
        graph: "●  ",
        hash: "1234abc",
        msg: "Initial theme",
        author: "Mario",
    },
];

pub(crate) fn commits() -> Vec<Commit> {
    COMMITS.to_vec()
}

#[derive(Clone, Copy, PartialEq, Eq)]
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

pub(crate) struct FileEntry {
    pub(crate) path: &'static str,
    pub(crate) status: FileStatus,
    pub(crate) staged: bool,
}

pub(crate) fn files() -> Vec<FileEntry> {
    vec![
        FileEntry {
            path: "src/main.rs",
            status: FileStatus::Modified,
            staged: true,
        },
        FileEntry {
            path: "src/theme.rs",
            status: FileStatus::Modified,
            staged: false,
        },
        FileEntry {
            path: "src/ui.rs",
            status: FileStatus::Added,
            staged: false,
        },
        FileEntry {
            path: "README.md",
            status: FileStatus::Deleted,
            staged: false,
        },
        FileEntry {
            path: "notes.txt",
            status: FileStatus::Untracked,
            staged: false,
        },
    ]
}

pub(crate) const DIFF: &[&str] = &[
    "diff --git a/src/theme.rs b/src/theme.rs",
    "@@ -1,6 +1,9 @@",
    " use ratatui::style::{Color, Style};",
    " ",
    "-pub fn window() -> Style {",
    "-    Style::new().fg(Color::White)",
    "+/// Testo giallo su blu, come Turbo C.",
    "+pub fn window() -> Style {",
    "+    Style::new().fg(Color::LightYellow).bg(Color::Blue)",
    " }",
    " ",
    "@@ -20,3 +23,7 @@",
    " pub fn shadow() -> Style {",
    "-    Style::new()",
    "+    Style::new().fg(Color::DarkGray).bg(Color::Black)",
    " }",
];

pub(crate) fn diff() -> Vec<&'static str> {
    DIFF.to_vec()
}
