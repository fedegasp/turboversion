//
// src/app/model/changes.rs
// TurboVersion
//
// 2026-11-10 Federico Gasperini
//

//! Che cosa mostra il pannello Changes: la working copy, oppure i file di
//! un commit. La distinzione è un tipo e non un flag accanto alla lista,
//! perché da essa dipende quale lista si disegna, se `Space` ha senso e se
//! un esito arrivato dal worker va applicato o scartato.

use crate::app::list_panel::ListPanel;
use crate::git::domain::{CommitId, FileEntry};

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
    /// mettere o togliere file dallo stage.
    pub(crate) fn is_working_copy(&self) -> bool {
        matches!(self, Self::WorkingCopy)
    }
}

/// I file di un commit, legati al commit da cui vengono: una lista letta per
/// un commit non può finire sotto l'identità di un altro.
pub(crate) struct CommitChanges {
    id: CommitId,
    load: CommitLoad,
}

/// A che punto è la lettura dei file di un commit. Ogni stato si disegna in
/// modo diverso: una lista vuota da sola non distinguerebbe "in arrivo",
/// "illeggibile", "nessuno potrà leggerla" e "commit senza file".
pub(crate) enum CommitLoad {
    /// Chiesti al worker, non ancora arrivati.
    Awaiting,
    /// Arrivati: la lista, eventualmente vuota, con il suo cursore.
    Loaded(ListPanel<FileEntry>),
    /// Il backend non è riuscito a leggerli; il messaggio è il suo.
    Failed(String),
    /// Nessuno li leggerà: il thread che esegue le operazioni git non c'è
    /// più, prima che arrivassero o prima che venissero chiesti.
    Unavailable,
}

impl CommitChanges {
    /// La vista di un commit di cui si sono appena chiesti i file: in
    /// attesa finché non arrivano, invece di mostrare quelli del commit di
    /// prima sotto un'identità che non è la loro.
    pub(super) fn requested(id: CommitId) -> Self {
        Self {
            id,
            load: CommitLoad::Awaiting,
        }
    }

    /// La vista di un commit i cui file non si possono più chiedere.
    pub(super) fn unavailable(id: CommitId) -> Self {
        Self {
            id,
            load: CommitLoad::Unavailable,
        }
    }

    pub(crate) fn id(&self) -> &CommitId {
        &self.id
    }

    pub(super) fn load_mut(&mut self) -> &mut CommitLoad {
        &mut self.load
    }

    pub(super) fn is_loaded(&self) -> bool {
        matches!(self.load, CommitLoad::Loaded(_))
    }

    /// Registra l'esito della lettura per `id`, se è quello che questa vista
    /// sta aspettando, e dice se lo era. Un esito per un altro commit è
    /// obsoleto (la selezione si è spostata prima che arrivasse); un
    /// secondo esito per lo stesso commit, già arrivato, riporterebbe il
    /// cursore in cima senza mostrare nulla di nuovo. In entrambi i casi la
    /// vista resta com'è.
    ///
    /// L'identità della richiesta è l'id del commit e non un contatore: un
    /// commit non cambia, quindi un esito per l'id atteso è corretto
    /// qualunque delle richieste per quell'id l'abbia prodotto.
    pub(super) fn receive(
        &mut self,
        id: &CommitId,
        result: Result<Vec<FileEntry>, String>,
    ) -> bool {
        if !matches!(self.load, CommitLoad::Awaiting) || self.id != *id {
            return false;
        }
        self.load = match result {
            Ok(files) => CommitLoad::Loaded(ListPanel::new(files)),
            Err(message) => CommitLoad::Failed(message),
        };
        true
    }

    /// Chiude un'attesa che nessun esito verrà più a chiudere. Una vista già
    /// caricata, o già fallita, resta com'è: quello che mostra è ancora vero.
    pub(super) fn give_up_waiting(&mut self) {
        if matches!(self.load, CommitLoad::Awaiting) {
            self.load = CommitLoad::Unavailable;
        }
    }

    #[cfg(test)]
    pub(crate) fn load(&self) -> &CommitLoad {
        &self.load
    }

    /// I file arrivati, per i test; `None` in ogni altro stato.
    #[cfg(test)]
    pub(crate) fn files(&self) -> Option<&ListPanel<FileEntry>> {
        match &self.load {
            CommitLoad::Loaded(files) => Some(files),
            CommitLoad::Awaiting | CommitLoad::Failed(_) | CommitLoad::Unavailable => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_awaiting(&self) -> bool {
        matches!(self.load, CommitLoad::Awaiting)
    }
}
