//
// src/git/git2.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Backend `Repo` su un repository reale, via `git2` (bindings a
//! `libgit2`). Solo lettura per ora (vedi `create_branch`/`delete_branch`
//! sotto): la scrittura reale arriverà in un passo successivo.

use super::domain::{Branch, BranchKind, Commit, FileEntry, FileStatus};
use super::repo::{GitError, Repo};

/// Limite al numero di commit letti da `commits()`: una cronologia reale può
/// contenere centinaia di migliaia di commit, e caricarli tutti in memoria a
/// ogni `Cmd::LoadAll` non servirebbe a una UI che mostra solo una finestra
/// scorrevole. Semplificazione intenzionale, non un bug: un pannello History
/// con paginazione o caricamento incrementale è fuori scopo per ora.
const MAX_COMMITS: usize = 500;

/// Backend `Repo` su un repository git reale, via `git2`.
pub(crate) struct GitRepo {
    repo: git2::Repository,
}

impl GitRepo {
    /// Apre il repository che contiene `path` (anche se `path` è una
    /// sottodirectory del repository: usa `Repository::discover`, non
    /// `Repository::open`, cosa che conta per `main.rs` che parte dalla cwd).
    pub(crate) fn discover(path: &str) -> Result<Self, GitError> {
        git2::Repository::discover(path)
            .map(|repo| Self { repo })
            .map_err(from_git2)
    }
}

/// Mappa un errore `git2` (dettagli del backend) a `GitError` (dominio),
/// così che `app`/`ui` non debbano mai conoscere `git2::Error`.
fn from_git2(err: git2::Error) -> GitError {
    GitError(err.message().to_string())
}

impl Repo for GitRepo {
    fn branches(&self) -> Vec<Branch> {
        // Riferimento per marcare `current: true` sul ramo locale su cui ci
        // troviamo: lo shorthand di HEAD (es. "main"), assente se HEAD è
        // detached o se la lettura fallisce (repository vuoto).
        let current_name = self
            .repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().ok().map(str::to_string));

        let mut result = Vec::new();

        if let Ok(branches) = self.repo.branches(None) {
            for (branch, branch_type) in branches.flatten() {
                let Ok(Some(name)) = branch.name() else {
                    continue;
                };
                let kind = match branch_type {
                    git2::BranchType::Local => BranchKind::Local,
                    git2::BranchType::Remote => BranchKind::Remote,
                };
                let current = kind == BranchKind::Local && current_name.as_deref() == Some(name);
                result.push(Branch {
                    name: name.to_string(),
                    kind,
                    current,
                });
            }
        }

        if let Ok(tags) = self.repo.tag_names(None) {
            // Doppio `flatten`: il primo scarta le voci che `tag_names`
            // non è riuscito a leggere (`Result`), il secondo le voci che
            // non sono UTF-8 valido (`Option`), lasciando solo `&str`.
            for name in tags.iter().flatten().flatten() {
                result.push(Branch {
                    name: name.to_string(),
                    kind: BranchKind::Tag,
                    current: false,
                });
            }
        }

        result
    }

    fn commits(&self) -> Vec<Commit> {
        let Ok(mut revwalk) = self.repo.revwalk() else {
            return Vec::new();
        };
        if revwalk.push_head().is_err() {
            return Vec::new();
        }
        if revwalk.set_sorting(git2::Sort::TIME).is_err() {
            return Vec::new();
        }

        revwalk
            .take(MAX_COMMITS)
            .filter_map(Result::ok)
            .filter_map(|oid| self.repo.find_commit(oid).ok())
            .map(|commit| Commit {
                // Semplificazione intenzionale: nessun grafo ASCII dei merge
                // (è un problema a sé, fuori scopo per ora). Stesso simbolo
                // per ogni riga, niente `─╮`/`─╯` come nei dati mock.
                graph: "*".to_string(),
                hash: commit.id().to_string().chars().take(7).collect(),
                msg: commit
                    .summary()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_string(),
                author: commit.author().name().unwrap_or_default().to_string(),
            })
            .collect()
    }

    fn files(&self) -> Vec<FileEntry> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let Ok(statuses) = self.repo.statuses(Some(&mut opts)) else {
            return Vec::new();
        };

        statuses
            .iter()
            .filter_map(|entry| {
                let path = entry.path().ok()?.to_string();
                let status = entry.status();
                // Un file può avere flag sia di INDEX_* che di WT_* insieme
                // (es. staged e poi modificato di nuovo nel worktree):
                // scegliamo deterministicamente lo stato dell'indice, perché
                // è quello che finirebbe nel prossimo commit.
                let (file_status, staged) = if status.contains(git2::Status::INDEX_NEW) {
                    (FileStatus::Added, true)
                } else if status.contains(git2::Status::INDEX_MODIFIED) {
                    (FileStatus::Modified, true)
                } else if status.contains(git2::Status::INDEX_DELETED) {
                    (FileStatus::Deleted, true)
                } else if status.contains(git2::Status::WT_NEW) {
                    (FileStatus::Untracked, false)
                } else if status.contains(git2::Status::WT_MODIFIED) {
                    (FileStatus::Modified, false)
                } else if status.contains(git2::Status::WT_DELETED) {
                    (FileStatus::Deleted, false)
                } else {
                    return None;
                };
                Some(FileEntry {
                    path,
                    status: file_status,
                    staged,
                })
            })
            .collect()
    }

    fn diff(&self) -> Vec<String> {
        let mut opts = git2::DiffOptions::new();
        // `show_untracked_content` serve a includere il *contenuto* dei
        // file non tracciati nella patch testuale (`include_untracked` da
        // solo li elenca soltanto, con un header ma senza corpo).
        opts.include_untracked(true).show_untracked_content(true);

        // HEAD -> worktree passando per l'indice (concettualmente
        // `git diff HEAD`): include sia le modifiche già in stage (index vs
        // HEAD) sia quelle non in stage (worktree vs index), fuse in un
        // solo diff. `diff_index_to_workdir` da solo (usato in una versione
        // precedente di questo metodo) è l'equivalente di `git diff` senza
        // `--cached`, cioè solo index vs worktree: un file staged senza
        // ulteriori modifiche nel worktree risulterebbe assente dal diff,
        // pur comparendo come "staged" nel pannello Changes (`files()`,
        // che guarda giustamente prima l'indice) — un'incoerenza visibile
        // tra i due pannelli. `None` per l'albero quando HEAD non esiste
        // ancora (repository appena creato, nessun commit): tratta tutti i
        // file come nuovi rispetto a un albero vuoto.
        let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let Ok(diff) = self
            .repo
            .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))
        else {
            return Vec::new();
        };

        let mut patch = String::new();
        let printed = diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            // Antepone il carattere di origin (+/-/spazio) alle righe di
            // contenuto/contesto, come fa `git diff`; le righe di intestazione
            // (file, hunk, binary) non hanno un carattere di origin da
            // anteporre, il loro testo lo contiene già.
            if matches!(line.origin(), '+' | '-' | ' ') {
                patch.push(line.origin());
            }
            // `from_utf8_lossy`, non `from_utf8` scartato silenziosamente:
            // su contenuto non-UTF-8 valido, scartare la riga lascerebbe il
            // carattere di origin già accodato sopra "orfano" (senza
            // contenuto né newline), fondendosi con la riga successiva e
            // corrompendo l'output senza alcun errore visibile.
            patch.push_str(&String::from_utf8_lossy(line.content()));
            true
        });
        if printed.is_err() {
            return Vec::new();
        }

        patch.lines().map(str::to_string).collect()
    }

    fn create_branch(&mut self, _name: &str) -> Result<Branch, GitError> {
        Err(GitError(
            "Creazione di un ramo non ancora supportata da questo backend.".to_string(),
        ))
    }

    fn delete_branch(&mut self, _name: &str) -> Result<(), GitError> {
        Err(GitError(
            "Cancellazione di un ramo non ancora supportata da questo backend.".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Crea un repository temporaneo con un commit iniziale (un file
    /// tracciato) e una signature di test fissa, pronto per i test sotto.
    fn init_repo_with_a_commit(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init repo di test");
        {
            let mut config = repo.config().expect("config repo di test");
            config.set_str("user.name", "Test User").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }
        fs::write(dir.join("README.md"), "hello\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }
        repo
    }

    #[test]
    fn discover_finds_the_repo_from_a_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let real = GitRepo::discover(sub.to_str().unwrap()).unwrap();
        assert_eq!(real.commits().len(), 1);
    }

    #[test]
    fn discover_fails_outside_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        // Una directory temporanea vuota, senza un `.git` in nessun
        // antenato, non è detto in generale (dipende da dove gira il test
        // runner), ma lo è di sicuro dentro un tempdir appena creato.
        assert!(GitRepo::discover(dir.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn branches_lists_local_and_marks_current() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let head = repo.head().unwrap();
        let head_name = head.shorthand().unwrap().to_string();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature/x", &commit, false).unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let branches = real.branches();

        let current = branches.iter().find(|b| b.name == head_name).unwrap();
        assert!(current.current);
        assert_eq!(current.kind, BranchKind::Local);

        let feature = branches.iter().find(|b| b.name == "feature/x").unwrap();
        assert!(!feature.current);
        assert_eq!(feature.kind, BranchKind::Local);
    }

    #[test]
    fn commits_reads_history_from_head() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        fs::write(dir.path().join("README.md"), "hello again\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])
            .unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let commits = real.commits();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].msg, "Second commit");
        assert_eq!(commits[1].msg, "Initial commit");
        assert_eq!(commits[0].author, "Test User");
        assert_eq!(commits[0].hash.len(), 7);
    }

    #[test]
    fn files_reports_staged_and_untracked_entries() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());

        // Staged: modifica aggiunta all'indice.
        fs::write(dir.path().join("README.md"), "changed\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();

        // Untracked: un file nuovo, mai aggiunto.
        fs::write(dir.path().join("notes.txt"), "scratch\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let files = real.files();

        let readme = files.iter().find(|f| f.path == "README.md").unwrap();
        assert_eq!(readme.status, FileStatus::Modified);
        assert!(readme.staged);

        let notes = files.iter().find(|f| f.path == "notes.txt").unwrap();
        assert_eq!(notes.status, FileStatus::Untracked);
        assert!(!notes.staged);
    }

    #[test]
    fn diff_includes_untracked_and_modified_content() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        fs::write(dir.path().join("README.md"), "hello\nworld\n").unwrap();
        fs::write(dir.path().join("notes.txt"), "scratch\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let diff = real.diff();
        let text = diff.join("\n");

        assert!(text.contains("README.md"));
        assert!(text.contains("+world"));
        assert!(text.contains("notes.txt"));
    }

    #[test]
    fn diff_shows_staged_changes_even_without_further_worktree_edits() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());

        // Staged e basta: modifica scritta, aggiunta all'indice, e nessun
        // ulteriore tocco al file nel worktree dopo l'`add`. Un diff
        // index-vs-workdir puro (`git diff` senza `--cached`) qui
        // risulterebbe vuoto, pur comparendo il file come "staged" in
        // `files()`: un'incoerenza che questo test previene.
        fs::write(dir.path().join("README.md"), "hello\nstaged change\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let diff = real.diff();
        let text = diff.join("\n");

        assert!(
            text.contains("+staged change"),
            "il diff deve mostrare la modifica staged anche senza ulteriori \
             modifiche nel worktree; diff ottenuto: {text:?}"
        );
    }

    #[test]
    fn create_and_delete_branch_are_not_implemented_yet() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        assert!(real.create_branch("feature/x").is_err());
        assert!(real.delete_branch("main").is_err());
    }
}
