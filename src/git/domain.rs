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

/// Quanto delle modifiche di un file della working copy è nell'indice,
/// cioè finirebbe nel prossimo commit.
///
/// Tre stati e non un `bool` perché un file può essere in stage solo in
/// parte: messo in stage e poi modificato di nuovo, o messo in stage a
/// pezzi da un altro client (`git add -p`). Con un `bool` quel file
/// sarebbe "in stage" e la parte non ancora in stage sparirebbe dalla
/// lista, oppure "non in stage" e sparirebbe quella già pronta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Staging {
    /// Nessuna modifica nell'indice. È anche lo stato dei file di un
    /// commit, dove un indice non c'è.
    Unstaged,
    /// Una parte delle modifiche è nell'indice, un'altra solo su disco.
    Partly,
    /// Tutte le modifiche sono nell'indice.
    Staged,
}

#[derive(Clone, Debug)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) status: FileStatus,
    pub(crate) staging: Staging,
}

/// Rispetto a che cosa si guarda il diff di un file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffSource {
    /// Le modifiche non committate: dall'ultimo commit (HEAD) alla working
    /// copy, comprese quelle già nell'indice.
    WorkingCopy,
    /// Ciò che il commit ha cambiato rispetto al suo primo parent, o
    /// rispetto all'albero vuoto se non ha parent: lo stesso confronto con
    /// cui `Repo::files_of_commit` elenca i suoi file.
    Commit(CommitId),
}

/// Il diff di un file preciso, in un contesto preciso. È anche l'identità
/// di una lettura: un esito arrivato per un altro file, o per lo stesso
/// file in un altro contesto, non è quello da mostrare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiffTarget {
    pub(crate) source: DiffSource,
    /// Il percorso così come compare nella lista dei file, relativo alla
    /// radice del repository. Una directory non tracciata (`dir/`) copre
    /// tutti i file che contiene, a meno che non sia a sua volta un
    /// repository: i suoi file non appartengono a questo, e il diff non ha
    /// righe.
    pub(crate) path: String,
}

/// Il diff di un file, nella forma in cui si può mostrare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FileDiff {
    /// Righe della patch, intestazione compresa, come le stampa
    /// `git diff`: almeno un blocco di righe aggiunte, tolte o di contesto.
    Text(Vec<String>),
    /// Il file è cambiato, ma non come testo: un binario non ha righe da
    /// confrontare.
    Binary,
    /// Il percorso è (o contiene soltanto) un repository annidato non
    /// tracciato: i suoi file appartengono a un altro repository, e qui non
    /// hanno righe da confrontare.
    NestedRepository,
    /// Nessuna riga da mostrare: un file vuoto aggiunto, un cambio dei soli
    /// permessi, o un file che nel frattempo non ha più modifiche.
    NoTextChanges,
}
