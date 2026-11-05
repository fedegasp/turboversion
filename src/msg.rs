//
// src/msg.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Eventi in ingresso (`Msg`) ed effetti richiesti (`Cmd`), lo scambio
//! comune tra `input` (produce `Msg`), `app::update` (consuma `Msg`,
//! produce `Cmd`) ed `effects` (esegue `Cmd` contro `Repo`, produce il
//! `Msg` di esito).

use crate::git::domain::{Branch, Commit, FileEntry};
use crate::git::repo::GitError;

/// Evento verso `update`. Le varianti "di tasto" (`Esc`, `Enter`, ...) sono
/// semantiche e indipendenti dal contesto: è `update` a decidere cosa
/// significano in base allo stato corrente del `Model` (dialog aperto, menu
/// aperto, nessuno dei due), vedi `dispatch_key` in `app/update.rs`.
#[derive(Debug)]
pub(crate) enum Msg {
    /// Scorciatoia globale (Ctrl+C o Alt+X): esce sempre, indipendentemente
    /// dagli overlay aperti.
    Quit,
    /// Alt+<lettera>: richiesta di aprire il menu con quella hotkey.
    /// Ignorata da `update` se c'è un dialog modale aperto.
    OpenMenuByHotkey(char),
    F10,
    Esc,
    Enter,
    Tab,
    BackTab,
    Left,
    Right,
    Up,
    Down,
    Backspace,
    /// Carattere stampabile digitato. `is_plain_ctrl` distingue una vera
    /// scorciatoia Ctrl+lettera (es. Ctrl+Z, da ignorare in un campo di
    /// testo) da combinazioni come AltGr (riportato da molte tastiere non-US
    /// come Ctrl+Alt) che devono restare testo digitabile: vedi
    /// `input::key_to_msg`.
    Char {
        c: char,
        is_plain_ctrl: bool,
    },
    /// Il pannello Diff è stato disegnato alto `0` o più righe, e
    /// l'altezza è cambiata rispetto a quella nota. Lo emette il ciclo
    /// principale confrontando l'altezza misurata da `ui::view` con quella
    /// registrata nel `Model`: così l'altezza del pannello entra nel
    /// `Model` per la stessa porta di ogni altro evento, e `update` resta
    /// l'unico a scriverlo.
    DiffViewport(u16),
    /// Esito di `Cmd::LoadAll`: i dati iniziali letti da `Repo`.
    Loaded {
        branches: Vec<Branch>,
        commits: Vec<Commit>,
        files: Vec<FileEntry>,
        diff: Vec<String>,
    },
    /// Esito di `Cmd::CreateBranch`; `name` è il nome digitato (già
    /// normalizzato), utile per ripopolare il dialog in caso di errore.
    BranchCreated {
        name: String,
        result: Result<Branch, GitError>,
    },
    /// Esito di `Cmd::DeleteBranch`.
    BranchDeleted {
        name: String,
        result: Result<(), GitError>,
    },
}

/// Effetto richiesto da `update`, da eseguire contro `Repo` (vedi
/// `effects::run`).
#[derive(Debug)]
pub(crate) enum Cmd {
    /// Carica branch/commit/file/diff iniziali.
    LoadAll,
    CreateBranch(String),
    DeleteBranch(String),
}
