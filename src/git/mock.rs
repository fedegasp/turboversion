//
// src/git/mock.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Implementazione di `GitRepo` che riproduce in memoria i dati finti e il
//! comportamento (inclusa la validazione dei nomi di ramo) che nel
//! prototipo iniziale vivevano sparsi tra `app.rs` e `mock.rs`. Serve a
//! sviluppare la TUI senza dipendere da un repository reale.

use super::domain::{Branch, BranchKind, Commit, FileEntry, FileStatus};
use super::repo::{GitError, GitRepo};

fn initial_branches() -> Vec<Branch> {
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

fn initial_commits() -> Vec<Commit> {
    [
        ("●─╮", "a1b2c3d", "Merge feature/menu", "Mario"),
        ("│ ●", "d4e5f6a", "Add dropdown menu", "Luca"),
        ("│ ●", "9f8e7d6", "Add menubar widget", "Luca"),
        ("●─╯", "c0ffee1", "Fix status bar", "Mario"),
        ("●  ", "b4dc0de", "Window shadow", "Mario"),
        ("●  ", "1234abc", "Initial theme", "Mario"),
    ]
    .into_iter()
    .map(|(graph, hash, msg, author)| Commit {
        graph: graph.to_string(),
        hash: hash.to_string(),
        msg: msg.to_string(),
        author: author.to_string(),
    })
    .collect()
}

fn initial_files() -> Vec<FileEntry> {
    [
        ("src/main.rs", FileStatus::Modified, true),
        ("src/theme.rs", FileStatus::Modified, false),
        ("src/ui.rs", FileStatus::Added, false),
        ("README.md", FileStatus::Deleted, false),
        ("notes.txt", FileStatus::Untracked, false),
    ]
    .into_iter()
    .map(|(path, status, staged)| FileEntry {
        path: path.to_string(),
        status,
        staged,
    })
    .collect()
}

fn initial_diff() -> Vec<String> {
    [
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
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// Valida un nome di ramo in modo vicino a `git check-ref-format`: niente
/// spazi/controlli, niente `-` iniziale (eviterebbe che un domani, passato a
/// `git` come argomento, venga scambiato per un'opzione), niente sequenze o
/// terminazioni che git stesso rifiuta.
fn validate_branch_name(name: &str, existing: &[Branch]) -> Option<String> {
    const INVALID_CHARS: &[char] = &['~', '^', ':', '?', '*', '[', '\\', ' '];
    if name.is_empty() {
        return Some("Branch name cannot be empty.".to_string());
    }
    if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("Branch names cannot contain spaces or control characters.".to_string());
    }
    if name.starts_with('-') {
        return Some("Branch names cannot start with '-'.".to_string());
    }
    if name.ends_with('/') || name.ends_with('.') || name.ends_with(".lock") {
        return Some("Branch names cannot end with '/', '.' or '.lock'.".to_string());
    }
    if name.contains("..")
        || name.contains("@{")
        || name.chars().any(|c| INVALID_CHARS.contains(&c))
    {
        return Some(
            "Branch names cannot contain '..', '@{', or any of ~ ^ : ? * [ \\.".to_string(),
        );
    }
    if existing.iter().any(|b| b.name == name) {
        return Some(format!("Branch '{name}' already exists."));
    }
    None
}

/// Backend git "finto": tiene i dati in memoria, per sviluppare la TUI senza
/// dipendere da un repository reale (vedi roadmap in README.md).
pub(crate) struct MockGitRepo {
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    files: Vec<FileEntry>,
    diff: Vec<String>,
}

impl MockGitRepo {
    pub(crate) fn new() -> Self {
        Self {
            branches: initial_branches(),
            commits: initial_commits(),
            files: initial_files(),
            diff: initial_diff(),
        }
    }
}

impl Default for MockGitRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl GitRepo for MockGitRepo {
    fn branches(&self) -> Vec<Branch> {
        self.branches.clone()
    }

    fn commits(&self) -> Vec<Commit> {
        self.commits.clone()
    }

    fn files(&self) -> Vec<FileEntry> {
        self.files.clone()
    }

    fn diff(&self) -> Vec<String> {
        self.diff.clone()
    }

    fn create_branch(&mut self, name: &str) -> Result<Branch, GitError> {
        if let Some(message) = validate_branch_name(name, &self.branches) {
            return Err(GitError(message));
        }
        let branch = Branch {
            name: name.to_string(),
            kind: BranchKind::Local,
            current: false,
        };
        self.branches.push(branch.clone());
        Ok(branch)
    }

    fn delete_branch(&mut self, name: &str) -> Result<(), GitError> {
        self.branches.retain(|b| b.name != name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_branch_adds_a_local_branch_and_returns_it() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        let branch = repo.create_branch("feature/x").unwrap();

        assert_eq!(branch.name, "feature/x");
        assert_eq!(branch.kind, BranchKind::Local);
        assert!(!branch.current);
        assert_eq!(repo.branches().len(), n + 1);
    }

    #[test]
    fn create_branch_rejects_invalid_or_duplicate_names() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        for bad in [
            "",
            "two words",
            "develop",
            "-oops",
            "a..b",
            "a@{b",
            "trailing/",
            "trailing.lock",
        ] {
            assert!(
                repo.create_branch(bad).is_err(),
                "{bad:?} doveva essere rifiutato"
            );
        }
        assert_eq!(repo.branches().len(), n);
    }

    #[test]
    fn delete_branch_removes_the_matching_branch_by_name() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        repo.delete_branch("develop").unwrap();

        assert_eq!(repo.branches().len(), n - 1);
        assert!(repo.branches().iter().all(|b| b.name != "develop"));
    }

    #[test]
    fn delete_branch_is_a_noop_for_an_unknown_name() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        repo.delete_branch("does-not-exist").unwrap();

        assert_eq!(repo.branches().len(), n);
    }
}
