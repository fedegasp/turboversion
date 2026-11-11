//
// src/app/model/navigation.rs
// TurboVersion
//
// 2026-11-11 Federico Gasperini
//

//! Changes e Diff seguono la selezione: Changes la riga di History, Diff il
//! file selezionato in Changes, nel contesto di Changes (la working copy o
//! un commit). Qui si decide che cosa mostrano, che cosa serve leggere e
//! quando chiederlo.
//!
//! Le letture sono fuse: per ogni tipo (file di un commit, diff di un file)
//! al worker ce n'è al più una per volta. Scorrendo History in fretta la
//! selezione attraversa molte righe prima che la prima lettura torni; il
//! `Model` ricorda solo ciò che è *voluto* (la vista in attesa) e ciò che è
//! *in volo* (`InFlight`), e quando la lettura in volo torna chiede la
//! voluta, se nel frattempo è cambiata. Le righe attraversate e già
//! lasciate non vengono lette affatto.
//!
//! La fusione sta qui e non nel worker perché solo il `Model` sa quale
//! lettura ha superato quale, e perché così ogni `Cmd` continua a produrre
//! esattamente un `Msg`, su cui il ciclo principale conta.

use crate::app::list_panel::ListPanel;
use crate::git::domain::{CommitId, DiffSource, DiffTarget, FileDiff, FileEntry};
use crate::msg::Cmd;

use super::changes::{ChangesView, CommitChanges};
use super::reading::{Load, Reading};
use super::{HistoryEntry, Model};

/// Che cosa mostra il pannello Diff: il diff di un file in un contesto, a
/// qualunque punto sia la sua lettura, oppure nulla se in Changes non c'è
/// un file selezionato.
pub(crate) type DiffView = Option<Reading<DiffTarget, FileDiff>>;

/// Le letture partite verso il worker e non ancora tornate, una per tipo.
/// Si ricorda la chiave e non solo "ce n'è una": se la selezione torna su
/// ciò che è già in volo non serve chiederlo di nuovo, basta aspettarlo.
#[derive(Default)]
pub(in crate::app) struct InFlight {
    files: Option<CommitId>,
    diff: Option<DiffTarget>,
}

impl Model {
    /// Allinea Changes alla selezione di History e Diff alla selezione di
    /// Changes, e restituisce le letture da chiedere. Va chiamata dopo ogni
    /// cambiamento che può spostare una selezione: un tasto, una rilettura
    /// del repository, l'arrivo dei file di un commit.
    ///
    /// Ciò che non cambia non si tocca: se Changes mostra già la riga
    /// selezionata e Diff già il file selezionato, non si chiede nulla e
    /// i cursori restano dove sono, scorrimento del diff compreso.
    ///
    /// Non tocca History: è ciò che impedisce il ciclo selezione ->
    /// ricarico -> cursore in cima -> nuova selezione.
    pub(in crate::app) fn follow_selection(&mut self) -> Vec<Cmd> {
        self.sync_changes_with_history();
        self.sync_diff_with_changes();
        self.next_reads()
    }

    /// Consegna a Changes l'esito della lettura dei file del commit `id`.
    /// Un esito che non è atteso (la selezione è passata altrove prima che
    /// arrivasse, o è un doppione) viene scartato senza toccare nulla,
    /// errore compreso. In entrambi i casi la lettura non è più in volo, e
    /// può partire quella voluta nel frattempo.
    pub(in crate::app) fn receive_commit_files(
        &mut self,
        id: &CommitId,
        result: Result<Vec<FileEntry>, String>,
    ) -> Vec<Cmd> {
        if self.in_flight.files.as_ref() == Some(id) {
            self.in_flight.files = None;
        }
        if let ChangesView::Commit(shown) = &mut self.changes {
            shown.receive(id, result.map(ListPanel::new));
        }
        // I file arrivati hanno un file selezionato: ora c'è un diff da
        // chiedere.
        self.follow_selection()
    }

    /// Consegna a Diff l'esito della lettura di `target`, con le stesse
    /// regole di `receive_commit_files`.
    pub(in crate::app) fn receive_file_diff(
        &mut self,
        target: &DiffTarget,
        result: Result<FileDiff, String>,
    ) -> Vec<Cmd> {
        if self.in_flight.diff.as_ref() == Some(target) {
            self.in_flight.diff = None;
        }
        if let Some(shown) = &mut self.diff {
            shown.receive(target, result);
        }
        self.follow_selection()
    }

    /// Dimentica il diff della working copy a schermo, perché i file su
    /// disco possono essere cambiati: il prossimo `follow_selection` lo
    /// richiede anche se il file selezionato è lo stesso. Il diff di un
    /// file in un commit resta: un commit non cambia.
    pub(in crate::app) fn forget_working_copy_diff(&mut self) {
        if self
            .diff
            .as_ref()
            .is_some_and(|d| d.key().source == DiffSource::WorkingCopy)
        {
            self.diff = None;
        }
    }

    /// Porta Changes sulla riga selezionata in History. Un commit già
    /// guardato si riprende così com'era; uno mai letto si mette in attesa,
    /// o nasce già "non disponibile" se git non c'è più. Una History vuota
    /// mostra la working copy, che in quel caso è vuota anche lei.
    fn sync_changes_with_history(&mut self) {
        let wanted = match self.history.selected() {
            None | Some(HistoryEntry::WorkingCopy) => None,
            Some(HistoryEntry::Commit(c)) => Some(c.id.clone()),
        };
        let next = match (wanted, &self.changes) {
            (None, ChangesView::WorkingCopy) => return,
            (Some(id), ChangesView::Commit(shown)) if *shown.id() == id => return,
            (None, ChangesView::Commit(_)) => ChangesView::WorkingCopy,
            (Some(id), ChangesView::WorkingCopy | ChangesView::Commit(_)) => {
                let git = self.git_available;
                ChangesView::Commit(
                    self.kept_commits
                        .take(&id)
                        .unwrap_or_else(|| CommitChanges::wanted(id, git)),
                )
            }
        };
        if let ChangesView::Commit(left) = std::mem::replace(&mut self.changes, next) {
            self.kept_commits.keep(left);
        }
    }

    /// Il diff che il pannello Diff dovrebbe mostrare: quello del file
    /// selezionato in Changes, nel contesto di Changes. `None` se Changes
    /// non ha un file selezionato, perché la lista è vuota o perché i file
    /// del commit non ci sono (ancora, o più).
    fn wanted_diff(&self) -> Option<DiffTarget> {
        let (source, files) = match &self.changes {
            ChangesView::WorkingCopy => (DiffSource::WorkingCopy, &self.working_copy_files),
            ChangesView::Commit(shown) => match shown.load() {
                Load::Loaded(files) => (DiffSource::Commit(shown.id().clone()), files),
                Load::Awaiting | Load::Failed(_) | Load::Unavailable => return None,
            },
        };
        files.selected().map(|f| DiffTarget {
            source,
            path: f.path.clone(),
        })
    }

    /// Porta Diff sul file selezionato in Changes. Il diff di un file in un
    /// commit già guardato si riprende senza rileggerlo; quello lasciato si
    /// conserva, se è di un commit e se era arrivato. Cambiando diff lo
    /// scorrimento torna in cima: una riga del diff di prima non indica
    /// nulla in quello nuovo.
    fn sync_diff_with_changes(&mut self) {
        let wanted = self.wanted_diff();
        if self.diff.as_ref().map(Reading::key) == wanted.as_ref() {
            return;
        }
        let git = self.git_available;
        let next = wanted.map(|target| {
            self.kept_diffs
                .take(&target)
                .unwrap_or_else(|| Reading::wanted(target, git))
        });
        if let Some(left) = std::mem::replace(&mut self.diff, next) {
            if matches!(left.key().source, DiffSource::Commit(_)) {
                self.kept_diffs.keep(left);
            }
        }
        self.diff_scroll = 0;
    }

    /// Le letture da chiedere adesso: per ogni tipo, quella voluta, se c'è
    /// e se al worker non ne è già in volo una. Una lettura in volo per
    /// un'altra chiave è ormai inutile, ma il suo esito tornerà comunque e
    /// verrà scartato; è al suo ritorno che parte quella voluta.
    fn next_reads(&mut self) -> Vec<Cmd> {
        let mut cmds = Vec::new();
        if !self.git_available {
            // Nessuno le eseguirebbe; e una vista non nasce in attesa senza
            // git, quindi qui non ci si arriva con qualcosa da chiedere.
            return cmds;
        }
        if let ChangesView::Commit(shown) = &self.changes {
            if shown.is_awaiting() && self.in_flight.files.is_none() {
                self.in_flight.files = Some(shown.id().clone());
                cmds.push(Cmd::LoadCommitFiles(shown.id().clone()));
            }
        }
        if let Some(shown) = &self.diff {
            if shown.is_awaiting() && self.in_flight.diff.is_none() {
                self.in_flight.diff = Some(shown.key().clone());
                cmds.push(Cmd::LoadFileDiff(shown.key().clone()));
            }
        }
        cmds
    }

    /// Mostra `lines` come diff del file selezionato in Changes, per i test
    /// che scorrono il pannello Diff senza passare da una lettura. Con la
    /// stessa chiave che `follow_selection` vorrebbe, così un tasto non lo
    /// sostituisce con una lettura vera.
    #[cfg(test)]
    pub(crate) fn show_diff_for_tests(&mut self, lines: Vec<String>) {
        let target = self.wanted_diff().unwrap_or(DiffTarget {
            source: DiffSource::WorkingCopy,
            path: "test".to_string(),
        });
        let mut view = Reading::requested(target.clone());
        view.receive(&target, Ok(FileDiff::Text(lines)));
        self.diff = Some(view);
        self.diff_scroll = 0;
    }

    /// Quante viste di commit e di diff sono conservate, per i test.
    #[cfg(test)]
    pub(crate) fn kept_counts(&self) -> (usize, usize) {
        (self.kept_commits.len(), self.kept_diffs.len())
    }
}
