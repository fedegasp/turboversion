//! Dati finti per sviluppare la TUI senza dipendere da git.

pub const BRANCHES: &[&str] = &[
    "* main",
    "  develop",
    "  feature/menu",
    "  origin/main",
    "  origin/develop",
    "  v0.1.0",
];

pub struct Commit {
    pub graph: &'static str,
    pub hash: &'static str,
    pub msg: &'static str,
    pub author: &'static str,
}

pub const COMMITS: &[Commit] = &[
    Commit { graph: "●─╮", hash: "a1b2c3d", msg: "Merge feature/menu", author: "Mario" },
    Commit { graph: "│ ●", hash: "d4e5f6a", msg: "Add dropdown menu", author: "Luca" },
    Commit { graph: "│ ●", hash: "9f8e7d6", msg: "Add menubar widget", author: "Luca" },
    Commit { graph: "●─╯", hash: "c0ffee1", msg: "Fix status bar", author: "Mario" },
    Commit { graph: "●  ", hash: "b4dc0de", msg: "Window shadow", author: "Mario" },
    Commit { graph: "●  ", hash: "1234abc", msg: "Initial theme", author: "Mario" },
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
}

impl FileStatus {
    /// Lettera di stato in stile `git status --short`.
    pub const fn label(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Untracked => '?',
        }
    }
}

pub struct FileEntry {
    pub path: &'static str,
    pub status: FileStatus,
    pub staged: bool,
}

pub fn files() -> Vec<FileEntry> {
    vec![
        FileEntry { path: "src/main.rs", status: FileStatus::Modified, staged: true },
        FileEntry { path: "src/theme.rs", status: FileStatus::Modified, staged: false },
        FileEntry { path: "src/ui.rs", status: FileStatus::Added, staged: false },
        FileEntry { path: "README.md", status: FileStatus::Deleted, staged: false },
        FileEntry { path: "notes.txt", status: FileStatus::Untracked, staged: false },
    ]
}

pub const DIFF: &[&str] = &[
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
