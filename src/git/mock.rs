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

use super::domain::{
    Branch, BranchKind, Commit, CommitId, DiffSource, DiffTarget, FileDiff, FileEntry, FileStatus,
    Staging,
};
use super::repo::{GitError, Repo};
use super::validate::{
    branch_not_found, current_branch_not_deletable, nothing_to_commit, validate_branch_name,
    validate_commit_message,
};

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
        ("src/main.rs", FileStatus::Modified, Staging::Staged),
        ("src/theme.rs", FileStatus::Modified, Staging::Unstaged),
        ("src/ui.rs", FileStatus::Added, Staging::Unstaged),
        ("README.md", FileStatus::Deleted, Staging::Unstaged),
        ("notes.txt", FileStatus::Untracked, Staging::Unstaged),
    ]
    .into_iter()
    .map(|(path, status, staging)| FileEntry {
        path: path.to_string(),
        status,
        staging,
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
                    staging: Staging::Unstaged,
                })
                .collect();
            (CommitId(hash.to_string()), files)
        })
        .collect()
}

/// Il diff di `src/theme.rs` nella working copy: l'unico scritto a mano,
/// perché è quello che si vede appena aperta l'app sui dati di prova.
fn theme_diff() -> Vec<String> {
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

/// Un diff plausibile per `file`, con il nome di `context` nelle righe:
/// così un test riconosce non solo di quale file è il diff mostrato, ma
/// anche se viene dalla working copy o da quale commit.
fn synthetic_diff(file: &FileEntry, context: &str) -> Vec<String> {
    let path = &file.path;
    let mut lines = vec![format!("diff --git a/{path} b/{path}")];
    let body = match file.status {
        FileStatus::Added | FileStatus::Untracked => {
            vec![
                "@@ -0,0 +1,2 @@".to_string(),
                format!("+{path} ({context})"),
                "+".to_string(),
            ]
        }
        FileStatus::Deleted => vec![
            "@@ -1,2 +0,0 @@".to_string(),
            format!("-{path} ({context})"),
            "-".to_string(),
        ],
        FileStatus::Modified => vec![
            "@@ -1,2 +1,2 @@".to_string(),
            format!(" {path}"),
            format!("-old ({context})"),
            format!("+new ({context})"),
        ],
    };
    lines.extend(body);
    lines
}

/// Backend git "finto": tiene i dati in memoria, per sviluppare la TUI senza
/// dipendere da un repository reale (vedi roadmap in README.md).
pub(crate) struct MockGitRepo {
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    commit_files: Vec<(CommitId, Vec<FileEntry>)>,
    files: Vec<FileEntry>,
    /// Le directory non tracciate fra `files`, con i file che contengono:
    /// lo stage le sostituisce con quei file.
    untracked_dirs: Vec<(String, Vec<String>)>,
}

impl MockGitRepo {
    pub(crate) fn new() -> Self {
        Self {
            branches: initial_branches(),
            commits: initial_commits(),
            commit_files: initial_commit_files(),
            files: initial_files(),
            untracked_dirs: Vec::new(),
        }
    }

    /// Lo stesso backend con in più una directory non tracciata `dir`
    /// (con la `/` finale, come la elenca lo stato) che contiene `files`,
    /// inserita nella working copy alla posizione `at`.
    pub(crate) fn with_untracked_dir(mut self, at: usize, dir: &str, files: &[&str]) -> Self {
        self.files.insert(
            at,
            FileEntry {
                path: dir.to_string(),
                status: FileStatus::Untracked,
                staging: Staging::Unstaged,
            },
        );
        self.untracked_dirs.push((
            dir.to_string(),
            files.iter().map(|f| f.to_string()).collect(),
        ));
        self
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

    /// Un file che non compare fra quelli del contesto non ha differenze,
    /// come per il backend reale; un commit sconosciuto è un errore.
    fn file_diff(&self, target: &DiffTarget) -> Result<FileDiff, GitError> {
        let (files, context) = match &target.source {
            DiffSource::WorkingCopy => (self.files.as_slice(), "working copy".to_string()),
            DiffSource::Commit(id) => {
                let files = self
                    .commit_files
                    .iter()
                    .find(|(c, _)| c == id)
                    .map(|(_, files)| files.as_slice())
                    .ok_or_else(|| {
                        GitError(format!(
                            "Cannot read the diff of {}: commit {} not found",
                            target.path,
                            id.as_str()
                        ))
                    })?;
                (files, id.as_str().to_string())
            }
        };
        let Some(file) = files.iter().find(|f| f.path == target.path) else {
            return Ok(FileDiff::NoTextChanges);
        };
        if target.source == DiffSource::WorkingCopy && file.path == "src/theme.rs" {
            return Ok(FileDiff::Text(theme_diff()));
        }
        Ok(FileDiff::Text(synthetic_diff(file, &context)))
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

    /// Lo stage di un file in memoria: un file non tracciato diventa
    /// aggiunto, come nel backend reale, e una directory non tracciata
    /// diventa i file che contiene, nella stessa posizione. Un percorso
    /// senza modifiche non cambia nulla, come nel backend reale.
    fn stage_file(&mut self, path: &str) -> Result<(), GitError> {
        if let Some(at) = self.untracked_dirs.iter().position(|(dir, _)| dir == path) {
            let (_, contents) = self.untracked_dirs.remove(at);
            if let Some(index) = self.files.iter().position(|f| f.path == path) {
                let staged = contents.into_iter().map(|path| FileEntry {
                    path,
                    status: FileStatus::Added,
                    staging: Staging::Staged,
                });
                self.files.splice(index..=index, staged);
            }
            return Ok(());
        }
        let Some(file) = self.files.iter_mut().find(|f| f.path == path) else {
            return Ok(());
        };
        if file.status == FileStatus::Untracked {
            file.status = FileStatus::Added;
        }
        file.staging = Staging::Staged;
        Ok(())
    }

    /// L'unstage in memoria: un file aggiunto torna non tracciato, come
    /// nel backend reale; un percorso senza modifiche non cambia nulla.
    fn unstage_file(&mut self, path: &str) -> Result<(), GitError> {
        let Some(file) = self.files.iter_mut().find(|f| f.path == path) else {
            return Ok(());
        };
        if file.status == FileStatus::Added {
            file.status = FileStatus::Untracked;
        }
        file.staging = Staging::Unstaged;
        Ok(())
    }

    /// Il commit in memoria: i file in stage escono dalla working copy ed
    /// entrano in un commit nuovo in cima al log; quelli in stage solo in
    /// parte vi entrano e restano nella working copy con il resto. Stesse
    /// regole del backend reale su messaggio e stage vuoto.
    fn commit(&mut self, message: &str) -> Result<CommitId, GitError> {
        if let Some(err) = validate_commit_message(message) {
            return Err(err);
        }
        let committed: Vec<FileEntry> = self
            .files
            .iter()
            .filter(|f| f.staging != Staging::Unstaged)
            .map(|f| FileEntry {
                path: f.path.clone(),
                status: f.status,
                staging: Staging::Unstaged,
            })
            .collect();
        if committed.is_empty() {
            return Err(nothing_to_commit());
        }
        self.files.retain(|f| f.staging != Staging::Staged);
        for f in &mut self.files {
            f.staging = Staging::Unstaged;
        }
        // Un hash finto ma univoco fra i commit in memoria.
        let hash = format!("c{:06}", self.commits.len());
        let id = CommitId(hash.clone());
        self.commits.insert(
            0,
            Commit {
                id: id.clone(),
                graph: "●  ".to_string(),
                hash,
                msg: message
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                author: "Mario".to_string(),
            },
        );
        self.commit_files.push((id.clone(), committed));
        Ok(id)
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
            assert!(files.iter().all(|f| f.staging == Staging::Unstaged));
        }
        assert!(repo
            .files_of_commit(&CommitId("does-not-exist".to_string()))
            .is_err());
    }

    /// Ogni file elencato ha un diff, diverso per contesto: la working copy
    /// e ciascun commit danno righe diverse anche per lo stesso percorso.
    #[test]
    fn every_listed_file_has_a_diff_of_its_own_context() {
        let repo = MockGitRepo::new();
        let text = |source: DiffSource, path: &str| match repo.file_diff(&DiffTarget {
            source,
            path: path.to_string(),
        }) {
            Ok(FileDiff::Text(lines)) => lines,
            other => panic!("{path}: atteso un diff testuale, non {other:?}"),
        };
        for f in repo.files() {
            assert!(!text(DiffSource::WorkingCopy, &f.path).is_empty());
        }
        for c in repo.commits() {
            for f in repo.files_of_commit(&c.id).expect("commit noto") {
                let lines = text(DiffSource::Commit(c.id.clone()), &f.path);
                assert!(lines.join("\n").contains(&c.hash), "{lines:?}");
            }
        }
        assert_eq!(
            repo.file_diff(&DiffTarget {
                source: DiffSource::WorkingCopy,
                path: "not/changed.rs".to_string(),
            }),
            Ok(FileDiff::NoTextChanges)
        );
        assert!(repo
            .file_diff(&DiffTarget {
                source: DiffSource::Commit(CommitId("does-not-exist".to_string())),
                path: "x".to_string(),
            })
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

    #[test]
    fn stage_unstage_and_commit_follow_the_rules_of_the_real_backend() {
        let mut repo = MockGitRepo::new();
        let state = |repo: &MockGitRepo, path: &str| {
            repo.files()
                .into_iter()
                .find(|f| f.path == path)
                .map(|f| (f.status, f.staging))
        };

        repo.stage_file("notes.txt").unwrap();
        assert_eq!(
            state(&repo, "notes.txt"),
            Some((FileStatus::Added, Staging::Staged))
        );
        repo.unstage_file("notes.txt").unwrap();
        assert_eq!(
            state(&repo, "notes.txt"),
            Some((FileStatus::Untracked, Staging::Unstaged))
        );
        repo.stage_file("nope").unwrap();
        repo.unstage_file("nope").unwrap();
        assert_eq!(repo.files().len(), initial_files().len(), "nessun effetto");

        assert!(repo.commit("  ").is_err());
        let n = repo.commits().len();
        let id = repo.commit("Commit main.rs").unwrap();
        assert_eq!(repo.commits().len(), n + 1);
        assert_eq!(repo.commits()[0].id, id);
        assert!(state(&repo, "src/main.rs").is_none(), "committato");
        assert_eq!(
            repo.files_of_commit(&id).unwrap()[0].path,
            "src/main.rs".to_string()
        );
        assert_eq!(repo.commit("Again"), Err(nothing_to_commit()));
    }
}
