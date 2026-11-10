//
// src/git/mock.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Implementazione di `Repo` con dati finti in memoria, per esercitare la
//! TUI senza dipendere da un repository reale. Le operazioni mutanti usano
//! le stesse regole sui nomi e gli stessi messaggi d'errore del backend
//! reale (`git::validate`), così un test che passa qui non descrive un
//! comportamento inesistente in produzione.
//!
//! Resta fuori portata ciò che solo un refdb vero può rilevare: una
//! collisione fra nomi che differiscono per sole maiuscole su un filesystem
//! case-insensitive, o un ramo `a/b` quando esiste già `a`. Sul backend
//! reale sono errori, qui no.

use super::domain::{Branch, BranchKind, Commit, CommitId, FileEntry, FileStatus};
use super::repo::{GitError, Repo};
use super::validate::{branch_not_found, current_branch_not_deletable, validate_branch_name};

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
        // In memoria l'hash abbreviato basta come identità: è univoco fra
        // questi pochi commit.
        id: CommitId(hash.to_string()),
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

/// I file toccati da ciascun commit di `initial_commits`, rispetto al suo
/// primo parent. Insiemi diversi fra loro e diversi dalla working copy, così
/// un test vede subito quale lista sta mostrando il pannello Changes.
fn initial_commit_files() -> Vec<(CommitId, Vec<FileEntry>)> {
    let table: [(&str, &[(&str, FileStatus)]); 6] = [
        ("a1b2c3d", &[("src/ui/menubar.rs", FileStatus::Added)]),
        (
            "d4e5f6a",
            &[
                ("src/ui/menubar.rs", FileStatus::Modified),
                ("src/ui/dropdown.rs", FileStatus::Added),
            ],
        ),
        ("9f8e7d6", &[("src/ui/menubar.rs", FileStatus::Added)]),
        ("c0ffee1", &[("src/ui/statusbar.rs", FileStatus::Modified)]),
        (
            "b4dc0de",
            &[
                ("src/ui/window.rs", FileStatus::Modified),
                ("src/ui/old_window.rs", FileStatus::Deleted),
            ],
        ),
        (
            "1234abc",
            &[
                ("src/theme.rs", FileStatus::Added),
                ("Cargo.toml", FileStatus::Added),
            ],
        ),
    ];
    table
        .into_iter()
        .map(|(hash, files)| {
            let files = files
                .iter()
                .map(|&(path, status)| FileEntry {
                    path: path.to_string(),
                    status,
                    staged: false,
                })
                .collect();
            (CommitId(hash.to_string()), files)
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

/// Backend git "finto": tiene i dati in memoria, per sviluppare la TUI senza
/// dipendere da un repository reale (vedi roadmap in README.md).
pub(crate) struct MockGitRepo {
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    commit_files: Vec<(CommitId, Vec<FileEntry>)>,
    files: Vec<FileEntry>,
    diff: Vec<String>,
}

impl MockGitRepo {
    pub(crate) fn new() -> Self {
        Self {
            branches: initial_branches(),
            commits: initial_commits(),
            commit_files: initial_commit_files(),
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

impl Repo for MockGitRepo {
    fn branches(&self) -> Vec<Branch> {
        self.branches.clone()
    }

    fn commits(&self) -> Vec<Commit> {
        self.commits.clone()
    }

    fn files(&self) -> Vec<FileEntry> {
        self.files.clone()
    }

    fn files_of_commit(&self, id: &CommitId) -> Result<Vec<FileEntry>, GitError> {
        self.commit_files
            .iter()
            .find(|(c, _)| c == id)
            .map(|(_, files)| files.clone())
            .ok_or_else(|| {
                GitError(format!(
                    "Cannot read the files of commit {}: not found",
                    id.as_str()
                ))
            })
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
        // Stesse condizioni del backend reale: solo rami locali (un nome
        // remoto o un tag semplicemente "non esiste" come ramo locale) e
        // mai quello corrente.
        let Some(index) = self
            .branches
            .iter()
            .position(|b| b.name == name && b.kind == BranchKind::Local)
        else {
            return Err(branch_not_found(name));
        };
        if self.branches[index].current {
            return Err(current_branch_not_deletable(name));
        }
        self.branches.remove(index);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::validate::{branch_already_exists, branch_path_conflict};

    #[test]
    fn every_commit_has_its_own_files_and_an_unknown_one_is_an_error() {
        let repo = MockGitRepo::new();
        for c in repo.commits() {
            let files = repo.files_of_commit(&c.id).expect("commit noto");
            assert!(!files.is_empty(), "{} senza file", c.hash);
            assert!(files.iter().all(|f| !f.staged));
        }
        assert!(repo
            .files_of_commit(&CommitId("does-not-exist".to_string()))
            .is_err());
    }

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
        // Stesso messaggio del backend reale, non un errore qualsiasi.
        assert_eq!(
            repo.create_branch("develop").unwrap_err(),
            branch_already_exists("develop")
        );
        assert_eq!(
            repo.create_branch("feature").unwrap_err(),
            branch_path_conflict("feature", "feature/menu")
        );
        assert_eq!(
            repo.create_branch("main/x").unwrap_err(),
            branch_path_conflict("main/x", "main")
        );
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
    fn delete_branch_fails_for_an_unknown_name() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        assert_eq!(
            repo.delete_branch("does-not-exist").unwrap_err(),
            branch_not_found("does-not-exist")
        );

        assert_eq!(repo.branches().len(), n);
    }

    #[test]
    fn delete_branch_refuses_the_current_branch_and_non_local_refs() {
        let mut repo = MockGitRepo::new();
        let n = repo.branches().len();

        assert_eq!(
            repo.delete_branch("main").unwrap_err(),
            current_branch_not_deletable("main")
        );
        for name in ["origin/main", "v0.1.0"] {
            assert_eq!(
                repo.delete_branch(name).unwrap_err(),
                branch_not_found(name)
            );
        }
        assert_eq!(repo.branches().len(), n);
    }
}
