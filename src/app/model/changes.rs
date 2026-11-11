//
// src/app/model/changes.rs
// TurboVersion
//
// 2026-11-10 Federico Gasperini
//

//! Che cosa mostra il pannello Changes: la working copy, oppure i file di
//! un commit. La distinzione è un tipo e non un flag accanto alla lista,
//! perché da essa dipende quale lista si disegna, se `Space` ha senso, se
//! un esito arrivato dal worker va applicato o scartato, e in quale
//! contesto si legge il diff del file selezionato.

use crate::app::list_panel::ListPanel;
use crate::git::domain::{CommitId, FileEntry, Staging};

use super::reading::{Load, Reading};

/// La sorgente del pannello Changes.
///
/// I file della working copy non stanno qui ma in un campo del `Model`:
/// esistono anche mentre si guarda un commit (la riga "Uncommitted changes"
/// e lo stage dipendono da loro) e tornare sulla working copy deve
/// ritrovarli così com'erano, cursore compreso, senza rileggerli.
pub(crate) enum ChangesView {
    WorkingCopy,
    Commit(CommitChanges),
}

impl ChangesView {
    /// Se il pannello mostra la working copy, l'unico caso in cui ha senso
    /// mettere o togliere file dallo stage (vedi `Model::stage_toggle`).
    #[cfg(test)]
    pub(crate) fn is_working_copy(&self) -> bool {
        matches!(self, Self::WorkingCopy)
    }
}

/// Che cosa fa `Space` sul file selezionato della working copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageToggle {
    Stage,
    Unstage,
}

impl StageToggle {
    /// Un file tutto in stage ne esce; uno fuori dallo stage, o in stage
    /// solo in parte, vi entra per intero. Per il file in parte la scelta
    /// è la stessa di una casella a tre stati: il primo `Space` completa
    /// ciò che è stato iniziato, il secondo lo toglie tutto. L'opposto,
    /// togliere dallo stage la parte già pronta, disferebbe con un tasto
    /// un lavoro fatto a pezzi (`git add -p`) senza modo di rifarlo da qui.
    pub(crate) fn for_staging(staging: Staging) -> Self {
        match staging {
            Staging::Staged => Self::Unstage,
            Staging::Unstaged | Staging::Partly => Self::Stage,
        }
    }
}

/// I file di un commit, legati al commit da cui vengono: una lista letta per
/// un commit non può finire sotto l'identità di un altro.
pub(crate) type CommitChanges = Reading<CommitId, ListPanel<FileEntry>>;

/// A che punto è la lettura dei file di un commit. Un commit senza file è
/// un `Loaded` con la lista vuota, non un'attesa.
pub(crate) type CommitLoad = Load<ListPanel<FileEntry>>;

impl CommitChanges {
    pub(crate) fn id(&self) -> &CommitId {
        self.key()
    }

    /// I file arrivati, per i test; `None` in ogni altro stato.
    #[cfg(test)]
    pub(crate) fn files(&self) -> Option<&ListPanel<FileEntry>> {
        match self.load() {
            Load::Loaded(files) => Some(files),
            Load::Awaiting | Load::Failed(_) | Load::Unavailable => None,
        }
    }
}
