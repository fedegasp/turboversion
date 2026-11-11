//
// src/git/git2.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Backend `Repo` su un repository reale, via `git2` (bindings a
//! `libgit2`). Oltre alle letture gestisce la creazione e la cancellazione
//! di rami locali; le altre scritture (stage, commit, checkout) arriveranno
//! in passi successivi.

use super::domain::{
    Branch, BranchKind, Commit, CommitId, DiffSource, DiffTarget, FileDiff, FileEntry, FileStatus,
};
use super::repo::{GitError, Repo};
use super::validate::{
    branch_already_exists, branch_not_found, current_branch_not_deletable, validate_branch_name,
};

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

/// Come `from_git2`, ma antepone l'operazione tentata. Per le letture il
/// contesto è ovvio (sono tutte parte del caricamento iniziale), per le
/// scritture no: il solo testo di libgit2 non dice che cosa l'applicazione
/// stava provando a fare quando è fallita.
fn from_git2_while(context: &str, err: git2::Error) -> GitError {
    GitError(format!("{context}: {}", err.message()))
}

impl GitRepo {
    /// Ciò che il commit `id` ha cambiato: il confronto con il suo primo
    /// parent, come `git show --first-parent`, cioè per un merge ciò che ha
    /// portato nel ramo su cui è stato fatto. Senza parent (il primo
    /// commit, o la radice di un ramo orfano) il confronto è con l'albero
    /// vuoto, e ogni file del commit risulta aggiunto.
    ///
    /// È il confronto unico su cui si reggono sia l'elenco dei file di un
    /// commit sia il diff di uno di essi: un file elencato deve avere un
    /// diff, e viceversa.
    fn commit_diff(
        &self,
        id: &CommitId,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>, git2::Error> {
        let oid = git2::Oid::from_str(id.as_str())?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = match commit.parents().next() {
            Some(parent) => Some(parent.tree()?),
            None => None,
        };
        self.repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), opts)
    }

    /// Le modifiche non committate: dall'albero di HEAD alla working copy
    /// passando per l'indice, cioè `git diff HEAD`. Include sia ciò che è
    /// già in stage sia ciò che non lo è, fusi: il pannello Changes ha una
    /// riga sola per file, e un diff limitato a una delle due metà
    /// nasconderebbe l'altra di un file messo in stage solo in parte.
    ///
    /// Senza HEAD (repository appena creato, nessun commit) il confronto è
    /// con l'albero vuoto: ogni file risulta nuovo.
    fn working_copy_diff(
        &self,
        opts: &mut git2::DiffOptions,
    ) -> Result<git2::Diff<'_>, git2::Error> {
        let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        self.repo
            .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(opts))
    }
}

/// Le righe di una patch come le stampa `git diff`, e che cosa contiene:
/// righe di testo, un binario, oppure niente da mostrare oltre
/// l'intestazione.
fn patch_of(diff: &git2::Diff<'_>) -> Result<FileDiff, git2::Error> {
    let mut patch = String::new();
    let mut text = false;
    let mut binary = false;
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            // Le righe di contenuto e di contesto portano il carattere di
            // origine davanti, come in `git diff`; le intestazioni (file,
            // blocco) lo hanno già nel testo.
            origin @ ('+' | '-' | ' ') => {
                text = true;
                patch.push(origin);
            }
            // "Binary files a/x and b/x differ": un file cambiato che non
            // ha righe da confrontare.
            'B' => binary = true,
            _ => {}
        }
        // `from_utf8_lossy`, non `from_utf8` scartato in silenzio: su
        // contenuto non UTF-8 il carattere di origine già accodato sopra
        // resterebbe orfano, senza testo né a capo, e si fonderebbe con la
        // riga successiva.
        patch.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;
    Ok(if text {
        FileDiff::Text(patch.lines().map(str::to_string).collect())
    } else if binary {
        FileDiff::Binary
    } else {
        FileDiff::NoTextChanges
    })
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
                id: CommitId(commit.id().to_string()),
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

    fn files_of_commit(&self, id: &CommitId) -> Result<Vec<FileEntry>, GitError> {
        let short: String = id.as_str().chars().take(7).collect();
        let context = format!("Cannot read the files of commit {short}");
        let fail = |err: git2::Error| from_git2_while(&context, err);

        let diff = self.commit_diff(id, None).map_err(fail)?;

        Ok(diff
            .deltas()
            .filter_map(|delta| {
                // Senza rilevamento delle rinomine (`find_similar`) un file
                // spostato compare come un `Deleted` e un `Added`, che è
                // anche ciò che il pannello sa mostrare.
                let status = match delta.status() {
                    git2::Delta::Added | git2::Delta::Copied => FileStatus::Added,
                    git2::Delta::Deleted => FileStatus::Deleted,
                    git2::Delta::Modified | git2::Delta::Renamed | git2::Delta::Typechange => {
                        FileStatus::Modified
                    }
                    // Fra due alberi non compaiono: sono stati della working
                    // copy o dell'indice.
                    git2::Delta::Unmodified
                    | git2::Delta::Ignored
                    | git2::Delta::Untracked
                    | git2::Delta::Unreadable
                    | git2::Delta::Conflicted => return None,
                };
                // Un file cancellato esiste solo dal lato del parent, quindi
                // si legge da lì; per gli altri vale il lato del commit, con
                // il parent come ripiego. Fra due alberi ogni delta ha almeno
                // un lato con un percorso: un delta senza alcun percorso non
                // nominerebbe nessun file da elencare.
                let (first, second) = if status == FileStatus::Deleted {
                    (delta.old_file(), delta.new_file())
                } else {
                    (delta.new_file(), delta.old_file())
                };
                let path = first.path().or_else(|| second.path())?;
                Some(FileEntry {
                    path: path.to_string_lossy().into_owned(),
                    status,
                    staged: false,
                })
            })
            .collect())
    }

    fn file_diff(&self, target: &DiffTarget) -> Result<FileDiff, GitError> {
        let context = format!("Cannot read the diff of {}", target.path);
        let fail = |err: git2::Error| from_git2_while(&context, err);

        let mut opts = git2::DiffOptions::new();
        // Il percorso si confronta alla lettera, non come un glob: un nome
        // con `*` o `[` toccherebbe altri file. Una directory resta un
        // prefisso, quindi `dir/` (come lo elenca lo stato di una
        // directory non tracciata) copre i file che contiene.
        opts.pathspec(&target.path).disable_pathspec_match(true);
        let diff = match &target.source {
            DiffSource::WorkingCopy => {
                // Il contenuto dei file non tracciati, non solo il loro
                // nome: senza `show_untracked_content` la patch ha
                // l'intestazione e nessuna riga. `recurse_untracked_dirs`
                // apre una directory non tracciata nei file che contiene;
                // non apre un repository annidato, che resta senza righe.
                opts.include_untracked(true)
                    .show_untracked_content(true)
                    .recurse_untracked_dirs(true);
                self.working_copy_diff(&mut opts)
            }
            DiffSource::Commit(id) => self.commit_diff(id, Some(&mut opts)),
        }
        .map_err(fail)?;
        patch_of(&diff).map_err(fail)
    }

    fn create_branch(&mut self, name: &str) -> Result<Branch, GitError> {
        // La validazione viene prima di toccare il repository, così un nome
        // malformato o già preso non lascia nulla a metà; `branches()`
        // include anche remoti e tag, ma solo i nomi esatti collidono.
        if let Some(message) = validate_branch_name(name, &self.branches()) {
            return Err(GitError(message));
        }

        let context = format!("Cannot create branch '{name}'");

        // Il ramo nasce sul commit di HEAD, come `git branch <name>`. Che
        // HEAD non punti ancora a un commit è un caso normale (repository
        // appena inizializzato, o ramo orfano) e merita un messaggio
        // comprensibile al posto di quello interno di libgit2.
        let head = self.repo.head().map_err(|err| {
            if matches!(
                err.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) {
                GitError(format!("{context}: HEAD does not point to a commit yet."))
            } else {
                from_git2_while(&context, err)
            }
        })?;
        let commit = head
            .peel_to_commit()
            .map_err(|err| from_git2_while(&context, err))?;

        // `force = false`: senza sovrascritture silenziose di un ramo
        // esistente, la validazione sopra è l'unico punto che decide.
        let created = self.repo.branch(name, &commit, false).map_err(|err| {
            // Un conflitto può emergere solo qui e non in validazione: su un
            // filesystem case-insensitive `FEATURE` e `feature` sono lo
            // stesso riferimento, e il confronto sui nomi non lo vede.
            if err.code() == git2::ErrorCode::Exists {
                branch_already_exists(name)
            } else {
                from_git2_while(&context, err)
            }
        })?;

        // Il nome si rilegge dal riferimento creato invece di riecheggiare
        // quello richiesto: libgit2 può normalizzarlo, e il Model deve
        // contenere il nome che esiste davvero nel repository, altrimenti
        // la lista dei rami mostra un ramo inesistente. Il ripiego su
        // `name` copre il solo caso di un riferimento non UTF-8, dove il
        // nome digitato resta l'approssimazione migliore disponibile.
        let actual = created.name().ok().flatten().unwrap_or(name).to_string();

        Ok(Branch {
            name: actual,
            kind: BranchKind::Local,
            current: false,
        })
    }

    fn delete_branch(&mut self, name: &str) -> Result<(), GitError> {
        let context = format!("Cannot delete branch '{name}'");

        // Solo rami locali: un nome remoto o un tag non si cancella da qui.
        // `InvalidSpec` accanto a `NotFound` perché un nome malformato (es.
        // `a..b`) non è un riferimento che possa esistere: per chi guarda
        // la lista dei rami è lo stesso caso di "non c'è".
        let mut branch = self
            .repo
            .find_branch(name, git2::BranchType::Local)
            .map_err(|err| {
                if matches!(
                    err.code(),
                    git2::ErrorCode::NotFound | git2::ErrorCode::InvalidSpec
                ) {
                    branch_not_found(name)
                } else {
                    from_git2_while(&context, err)
                }
            })?;

        if branch.is_head() {
            return Err(current_branch_not_deletable(name));
        }

        branch
            .delete()
            .map_err(|err| from_git2_while(&context, err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::validate::branch_path_conflict;
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

    /// Committa su HEAD lo stato attuale dell'indice dopo aver scritto
    /// `write` e cancellato `remove`, con i `parents` dati. Restituisce
    /// l'identità del commit creato.
    fn commit_changes(
        repo: &git2::Repository,
        write: &[(&str, &str)],
        remove: &[&str],
        parents: &[&git2::Commit<'_>],
        message: &str,
    ) -> git2::Oid {
        let dir = repo.workdir().expect("repository con working copy");
        let mut index = repo.index().unwrap();
        for (path, content) in write {
            fs::write(dir.join(path), content).unwrap();
            index.add_path(Path::new(path)).unwrap();
        }
        for path in remove {
            fs::remove_file(dir.join(path)).unwrap();
            index.remove_path(Path::new(path)).unwrap();
        }
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, parents)
            .unwrap()
    }

    fn head_commit(repo: &git2::Repository) -> git2::Commit<'_> {
        repo.head().unwrap().peel_to_commit().unwrap()
    }

    /// I file di un commit come coppie (percorso, stato), ordinate per
    /// percorso: l'ordine dei delta non è parte del contratto.
    fn files_of(real: &GitRepo, oid: git2::Oid) -> Vec<(String, FileStatus)> {
        let mut files: Vec<_> = real
            .files_of_commit(&CommitId(oid.to_string()))
            .expect("il commit esiste")
            .into_iter()
            .map(|f| {
                assert!(!f.staged, "in un commit non c'è nulla in stage");
                (f.path, f.status)
            })
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    #[test]
    fn commits_carry_the_full_id_and_files_of_commit_accepts_it() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let head = head_commit(&repo).id();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let commits = real.commits();

        assert_eq!(commits[0].id, CommitId(head.to_string()));
        assert!(commits[0].id.as_str().starts_with(&commits[0].hash));
        assert!(real.files_of_commit(&commits[0].id).is_ok());
    }

    #[test]
    fn files_of_the_root_commit_are_all_added() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let root = head_commit(&repo).id();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            files_of(&real, root),
            [("README.md".to_string(), FileStatus::Added)]
        );
    }

    #[test]
    fn files_of_a_commit_are_its_changes_against_the_parent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let second = commit_changes(
            &repo,
            &[("keep.txt", "keep\n"), ("gone.txt", "gone\n")],
            &[],
            &[&head_commit(&repo)],
            "Second",
        );
        let third = commit_changes(
            &repo,
            &[("README.md", "changed\n"), ("new.txt", "new\n")],
            &["gone.txt"],
            &[&head_commit(&repo)],
            "Third",
        );
        // Modifiche lasciate nella working copy: non appartengono a nessun
        // commit e non devono comparire.
        fs::write(dir.path().join("keep.txt"), "dirty\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            files_of(&real, third),
            [
                ("README.md".to_string(), FileStatus::Modified),
                ("gone.txt".to_string(), FileStatus::Deleted),
                ("new.txt".to_string(), FileStatus::Added),
            ]
        );
        assert_eq!(
            files_of(&real, second),
            [
                ("gone.txt".to_string(), FileStatus::Added),
                ("keep.txt".to_string(), FileStatus::Added),
            ],
            "i file non toccati dal commit restano fuori"
        );
    }

    #[test]
    fn files_of_a_merge_are_compared_with_its_first_parent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let base = head_commit(&repo);

        // Due linee di sviluppo dalla stessa base, ognuna con un file suo.
        let side = commit_changes(&repo, &[("side.txt", "side\n")], &[], &[&base], "Side");
        let side = repo.find_commit(side).unwrap();
        repo.set_head_detached(base.id()).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .unwrap();
        let main = commit_changes(&repo, &[("main.txt", "main\n")], &[], &[&base], "Main");
        let main = repo.find_commit(main).unwrap();

        // Il merge contiene entrambi i file; rispetto al primo parent (la
        // linea "main") l'unica novità è quello arrivato dall'altra.
        let merge = commit_changes(
            &repo,
            &[("side.txt", "side\n")],
            &[],
            &[&main, &side],
            "Merge side",
        );

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            files_of(&real, merge),
            [("side.txt".to_string(), FileStatus::Added)]
        );
    }

    #[test]
    fn files_of_an_empty_commit_is_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        // Come `git commit --allow-empty`: stesso albero del parent.
        let empty = commit_changes(&repo, &[], &[], &[&head_commit(&repo)], "Empty");

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            real.files_of_commit(&CommitId(empty.to_string()))
                .expect("il commit esiste")
                .len(),
            0
        );
    }

    #[test]
    fn a_renamed_file_shows_up_as_deleted_and_added() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        // Stesso contenuto, percorso nuovo: per `git log --follow` è una
        // rinomina, qui due file distinti perché le rinomine non si cercano.
        let renamed = commit_changes(
            &repo,
            &[("MOVED.md", "hello\n")],
            &["README.md"],
            &[&head_commit(&repo)],
            "Rename",
        );

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            files_of(&real, renamed),
            [
                ("MOVED.md".to_string(), FileStatus::Added),
                ("README.md".to_string(), FileStatus::Deleted),
            ]
        );
    }

    #[test]
    fn files_of_an_unknown_or_malformed_commit_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        for id in ["0123456789012345678901234567890123456789", "not-a-hash"] {
            let err = real
                .files_of_commit(&CommitId(id.to_string()))
                .expect_err("nessun commit con questo id");
            assert!(
                err.to_string()
                    .starts_with("Cannot read the files of commit"),
                "il messaggio deve dire cosa si stava facendo: {err}"
            );
        }
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

    fn wc(path: &str) -> DiffTarget {
        DiffTarget {
            source: DiffSource::WorkingCopy,
            path: path.to_string(),
        }
    }

    fn of_commit(oid: git2::Oid, path: &str) -> DiffTarget {
        DiffTarget {
            source: DiffSource::Commit(CommitId(oid.to_string())),
            path: path.to_string(),
        }
    }

    /// Le righe di un diff che deve essere testuale.
    fn text_of(real: &GitRepo, target: &DiffTarget) -> String {
        match real.file_diff(target) {
            Ok(FileDiff::Text(lines)) => lines.join("\n"),
            other => panic!("{target:?}: atteso un diff testuale, non {other:?}"),
        }
    }

    #[test]
    fn the_working_copy_diff_of_a_file_leaves_the_other_files_out() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        commit_changes(
            &repo,
            &[("other.txt", "one\n")],
            &[],
            &[&head_commit(&repo)],
            "Other",
        );
        fs::write(dir.path().join("README.md"), "hello\nworld\n").unwrap();
        fs::write(dir.path().join("other.txt"), "one\ntwo\n").unwrap();
        fs::write(dir.path().join("notes.txt"), "scratch\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let readme = text_of(&real, &wc("README.md"));
        assert!(readme.contains("+world"), "{readme}");
        assert!(!readme.contains("other.txt") && !readme.contains("notes.txt"));

        // Un file non tracciato mostra il suo contenuto, non solo il nome.
        let notes = text_of(&real, &wc("notes.txt"));
        assert!(notes.contains("+scratch"), "{notes}");
        assert!(!notes.contains("README.md"));
    }

    /// Staged e non: il diff della working copy è `git diff HEAD`, quindi
    /// una modifica già nell'indice si vede anche senza altri ritocchi al
    /// file, e insieme a quelle fatte dopo.
    #[test]
    fn the_working_copy_diff_includes_what_is_already_staged() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        fs::write(dir.path().join("README.md"), "hello\nstaged\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert!(text_of(&real, &wc("README.md")).contains("+staged"));

        fs::write(dir.path().join("README.md"), "hello\nstaged\nlater\n").unwrap();
        let both = text_of(&real, &wc("README.md"));
        assert!(
            both.contains("+staged") && both.contains("+later"),
            "{both}"
        );
    }

    /// La directory non tracciata compare in Changes come `dir/`: il suo
    /// diff è quello dei file che contiene. Il percorso non è un glob, e
    /// non è nemmeno un prefisso qualsiasi: `READ` non è `README.md`.
    #[test]
    fn the_path_is_matched_literally_and_a_directory_covers_its_files() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        fs::create_dir(dir.path().join("newdir")).unwrap();
        fs::write(dir.path().join("newdir/inner.txt"), "inside\n").unwrap();
        fs::write(dir.path().join("README.md"), "changed\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let listed = real.files();
        assert!(listed.iter().any(|f| f.path == "newdir/"), "{listed:?}");
        assert!(text_of(&real, &wc("newdir/")).contains("+inside"));

        for not_a_file in ["READ", "*.md", "README.m?"] {
            assert_eq!(
                real.file_diff(&wc(not_a_file)),
                Ok(FileDiff::NoTextChanges),
                "{not_a_file:?}"
            );
        }
    }

    #[test]
    fn a_binary_file_and_an_empty_new_file_have_no_text_to_show() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let with_binary = commit_changes(
            &repo,
            &[("logo.bin", "\u{0}\u{1}\u{2}")],
            &[],
            &[&head_commit(&repo)],
            "Binary",
        );
        fs::write(dir.path().join("logo.bin"), b"\x00\x03\x04").unwrap();
        fs::write(dir.path().join("empty.txt"), "").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(real.file_diff(&wc("logo.bin")), Ok(FileDiff::Binary));
        assert_eq!(
            real.file_diff(&of_commit(with_binary, "logo.bin")),
            Ok(FileDiff::Binary)
        );
        assert_eq!(
            real.file_diff(&wc("empty.txt")),
            Ok(FileDiff::NoTextChanges)
        );
        // Un file senza modifiche non ha nulla da mostrare, e non è un
        // errore: la lista dei file può essere più vecchia del disco.
        assert_eq!(
            real.file_diff(&wc("README.md")),
            Ok(FileDiff::NoTextChanges)
        );
    }

    /// Il diff di un file in un commit è ciò che quel commit gli ha fatto
    /// rispetto al primo parent: né le modifiche della working copy né
    /// quelle di altri commit allo stesso file.
    #[test]
    fn the_diff_of_a_file_in_a_commit_is_against_the_first_parent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let second = commit_changes(
            &repo,
            &[("README.md", "hello\nsecond\n"), ("x.txt", "x\n")],
            &[],
            &[&head_commit(&repo)],
            "Second",
        );
        let third = commit_changes(
            &repo,
            &[("README.md", "hello\nsecond\nthird\n")],
            &["x.txt"],
            &[&head_commit(&repo)],
            "Third",
        );
        fs::write(dir.path().join("README.md"), "dirty\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let in_third = text_of(&real, &of_commit(third, "README.md"));
        assert!(in_third.contains("+third"), "{in_third}");
        assert!(!in_third.contains("+second") && !in_third.contains("dirty"));
        assert!(!in_third.contains("x.txt"), "un altro file del commit");

        // Il file cancellato dal commit ha il suo diff, dal lato del parent.
        assert!(text_of(&real, &of_commit(third, "x.txt")).contains("-x"));
        // Un file che il commit non tocca non ha differenze.
        assert_eq!(
            real.file_diff(&of_commit(second, "nope.txt")),
            Ok(FileDiff::NoTextChanges)
        );
    }

    #[test]
    fn the_diff_of_a_file_in_the_root_commit_is_against_the_empty_tree() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let root = head_commit(&repo).id();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let text = text_of(&real, &of_commit(root, "README.md"));
        assert!(text.contains("new file mode"), "{text}");
        assert!(text.contains("+hello"), "{text}");
    }

    #[test]
    fn the_diff_of_a_file_in_a_merge_is_against_its_first_parent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let base = head_commit(&repo);
        let side = commit_changes(&repo, &[("side.txt", "side\n")], &[], &[&base], "Side");
        let side = repo.find_commit(side).unwrap();
        repo.set_head_detached(base.id()).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .unwrap();
        let main = commit_changes(&repo, &[("main.txt", "main\n")], &[], &[&base], "Main");
        let main = repo.find_commit(main).unwrap();
        let merge = commit_changes(
            &repo,
            &[("side.txt", "side\n")],
            &[],
            &[&main, &side],
            "Merge side",
        );

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert!(text_of(&real, &of_commit(merge, "side.txt")).contains("+side"));
        assert_eq!(
            real.file_diff(&of_commit(merge, "main.txt")),
            Ok(FileDiff::NoTextChanges),
            "già nel primo parent"
        );
    }

    /// Senza alcun commit HEAD non esiste: il confronto parte dall'albero
    /// vuoto, e un file nuovo, in stage o no, mostra tutto il contenuto.
    #[test]
    fn the_working_copy_diff_works_before_the_first_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        fs::write(dir.path().join("staged.txt"), "in index\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("staged.txt")).unwrap();
        index.write().unwrap();
        fs::write(dir.path().join("loose.txt"), "not added\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert!(text_of(&real, &wc("staged.txt")).contains("+in index"));
        assert!(text_of(&real, &wc("loose.txt")).contains("+not added"));
    }

    /// Un cambio dei soli permessi è una modifica senza righe: c'è
    /// l'intestazione con i modi, nessun blocco di righe.
    #[cfg(unix)]
    #[test]
    fn a_change_of_permissions_only_has_no_text_changes() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let readme = dir.path().join("README.md");
        fs::set_permissions(&readme, fs::Permissions::from_mode(0o755)).unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        assert!(
            real.files().iter().any(|f| f.path == "README.md"),
            "il cambio di modo è una modifica della working copy"
        );
        assert_eq!(
            real.file_diff(&wc("README.md")),
            Ok(FileDiff::NoTextChanges)
        );

        // Lo stesso cambio, committato.
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        let chmod = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "chmod",
                &tree,
                &[&head_commit(&repo)],
            )
            .unwrap();
        assert_eq!(
            real.file_diff(&of_commit(chmod, "README.md")),
            Ok(FileDiff::NoTextChanges)
        );
    }

    /// Un nome che contiene caratteri da glob si prende alla lettera: il
    /// file con quel nome c'è, e solo lui.
    #[cfg(unix)]
    #[test]
    fn a_file_named_like_a_glob_is_found_by_its_own_name() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        for (name, content) in [
            ("a*b.txt", "star\n"),
            ("axb.txt", "plain\n"),
            ("c[1].txt", "bracket\n"),
            ("c1.txt", "one\n"),
        ] {
            fs::write(dir.path().join(name), content).unwrap();
        }

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let star = text_of(&real, &wc("a*b.txt"));
        assert!(star.contains("+star") && !star.contains("+plain"), "{star}");
        let bracket = text_of(&real, &wc("c[1].txt"));
        assert!(
            bracket.contains("+bracket") && !bracket.contains("+one"),
            "{bracket}"
        );
    }

    /// Una directory non tracciata che contiene un repository annidato non
    /// si apre: è un altro repository, e i suoi file non sono di questo.
    /// Compare in Changes come `dir/`, e il suo diff non ha righe.
    #[test]
    fn an_untracked_nested_repository_is_not_opened() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let nested = dir.path().join("nested");
        init_repo_with_a_commit(&nested);
        fs::write(nested.join("inner.txt"), "inner\n").unwrap();

        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let listed = real.files();
        assert!(listed.iter().any(|f| f.path == "nested/"), "{listed:?}");
        assert_eq!(real.file_diff(&wc("nested/")), Ok(FileDiff::NoTextChanges));
    }

    #[test]
    fn the_diff_of_an_unknown_commit_is_an_error_that_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        for id in ["0123456789012345678901234567890123456789", "not-a-hash"] {
            let err = real
                .file_diff(&DiffTarget {
                    source: DiffSource::Commit(CommitId(id.to_string())),
                    path: "README.md".to_string(),
                })
                .expect_err("nessun commit con questo id");
            assert!(
                err.to_string()
                    .starts_with("Cannot read the diff of README.md"),
                "{err}"
            );
        }
    }

    #[test]
    fn create_branch_adds_a_local_branch_on_head() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        let created = real.create_branch("feature/x").unwrap();

        assert_eq!(created.name, "feature/x");
        assert_eq!(created.kind, BranchKind::Local);
        assert!(!created.current);

        let listed = real
            .branches()
            .into_iter()
            .find(|b| b.name == created.name)
            .expect("il ramo creato deve comparire tra i branch del repository");
        assert_eq!(listed.kind, BranchKind::Local);
        assert!(!listed.current);
    }

    #[test]
    fn create_branch_rejects_duplicate_and_invalid_names() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let head_name = repo.head().unwrap().shorthand().unwrap().to_string();
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        let before = real.branches().len();

        assert!(real.create_branch(&head_name).is_err());
        for bad in ["", "two words", "-oops", "a..b", "trailing.lock"] {
            assert!(
                real.create_branch(bad).is_err(),
                "{bad:?} doveva essere rifiutato"
            );
        }

        assert_eq!(real.branches().len(), before);
    }

    #[test]
    fn create_branch_refuses_names_libgit2_would_rewrite() {
        // Senza la validazione, `repo.branch("a//b", ..)` riuscirebbe
        // creando `refs/heads/a/b`: un ramo con un nome diverso da quello
        // chiesto. Il rifiuto dev'essere pulito, senza riferimenti lasciati
        // indietro con il nome normalizzato.
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        assert!(real.create_branch("a//b").is_err());
        assert!(real.create_branch("/lead").is_err());

        let repo = git2::Repository::open(dir.path()).unwrap();
        for leftover in ["refs/heads/a/b", "refs/heads/lead", "refs/heads/a//b"] {
            assert!(
                repo.find_reference(leftover).is_err(),
                "{leftover} non doveva essere creato"
            );
        }
    }

    #[test]
    fn create_branch_refuses_a_name_that_collides_with_an_existing_branch_path() {
        // `a` e `a/b` non possono coesistere. Lasciato a libgit2, il verso
        // `a` -> `a/b` fallisce con un testo che cita un percorso interno
        // di `.git`, e il verso opposto con un altro ancora: il controllo
        // di dominio dà un messaggio solo, prima di toccare il disco.
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        real.create_branch("a").unwrap();
        real.create_branch("x/y").unwrap();

        assert_eq!(
            real.create_branch("a/b").unwrap_err(),
            branch_path_conflict("a/b", "a")
        );
        assert_eq!(
            real.create_branch("x").unwrap_err(),
            branch_path_conflict("x", "x/y")
        );

        let names: Vec<String> = real.branches().into_iter().map(|b| b.name).collect();
        assert!(!names.iter().any(|n| n == "a/b" || n == "x"));
    }

    #[test]
    fn create_branch_returns_the_name_the_repository_actually_has() {
        // Invariante centrale: il nome nel `Branch` restituito (che finisce
        // nel Model) deve essere uno di quelli elencati dal repository, mai
        // una semplice eco di ciò che è stato digitato.
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        for name in ["feature/x", "release-1.0", "a/b/c"] {
            let created = real.create_branch(name).unwrap();
            assert!(
                real.branches().iter().any(|b| b.name == created.name),
                "{:?} non esiste nel repository",
                created.name
            );
        }
    }

    #[test]
    fn create_branch_reports_head_without_a_commit() {
        let dir = tempfile::tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        let err = real.create_branch("feature/x").unwrap_err();

        assert!(
            err.to_string().contains("HEAD does not point to a commit"),
            "messaggio poco chiaro per un repository senza commit: {err}"
        );
    }

    #[test]
    fn create_branch_maps_a_name_clash_found_only_on_write() {
        // Su filesystem case-insensitive (APFS, NTFS) `feature` e `FEATURE`
        // sono lo stesso riferimento e il conflitto emerge solo alla
        // scrittura: il messaggio dev'essere quello di dominio, non il
        // testo interno di libgit2. Su filesystem case-sensitive non c'è
        // conflitto e la creazione riesce: entrambi gli esiti sono
        // corretti, quello che non deve succedere è un errore grezzo.
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();
        real.create_branch("feature").unwrap();

        match real.create_branch("FEATURE") {
            Ok(created) => assert_eq!(created.name, "FEATURE"),
            Err(err) => assert_eq!(err, branch_already_exists("FEATURE")),
        }
    }

    #[test]
    fn delete_branch_removes_a_local_branch() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature/x", &commit, false).unwrap();
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        real.delete_branch("feature/x").unwrap();

        assert!(real.branches().iter().all(|b| b.name != "feature/x"));
    }

    #[test]
    fn delete_branch_refuses_an_unknown_or_malformed_name() {
        let dir = tempfile::tempdir().unwrap();
        init_repo_with_a_commit(dir.path());
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        // Stesso errore di dominio del mock, non il testo di libgit2: un
        // nome malformato non è un riferimento che possa esistere, quindi
        // per chi guarda la lista dei rami è il caso "non c'è".
        assert_eq!(
            real.delete_branch("does-not-exist").unwrap_err(),
            branch_not_found("does-not-exist")
        );
        assert_eq!(
            real.delete_branch("a..b").unwrap_err(),
            branch_not_found("a..b")
        );
    }

    #[test]
    fn delete_branch_refuses_remote_branches_and_tags_and_keeps_them() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.reference("refs/remotes/origin/x", commit.id(), false, "test")
            .unwrap();
        repo.tag_lightweight("v0.1.0", commit.as_object(), false)
            .unwrap();
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        for name in ["origin/x", "v0.1.0"] {
            assert_eq!(
                real.delete_branch(name).unwrap_err(),
                branch_not_found(name)
            );
            assert!(
                real.branches().iter().any(|b| b.name == name),
                "{name} doveva restare nel repository"
            );
        }
    }

    #[test]
    fn delete_branch_refuses_the_current_branch_and_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_a_commit(dir.path());
        let head_name = repo.head().unwrap().shorthand().unwrap().to_string();
        let mut real = GitRepo::discover(dir.path().to_str().unwrap()).unwrap();

        assert_eq!(
            real.delete_branch(&head_name).unwrap_err(),
            current_branch_not_deletable(&head_name)
        );

        let still_there = real
            .branches()
            .into_iter()
            .find(|b| b.name == head_name)
            .expect("il ramo corrente deve restare");
        assert!(still_there.current);
    }
}
