//
// src/app/model/mod.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use super::list_panel::ListPanel;
use super::panel::Panel;
use crate::dialog::Dialog;
use crate::git::domain::{Branch, Commit, CommitId, DiffTarget, FileDiff, FileEntry};

mod changes;
mod navigation;
mod reading;

pub(crate) use changes::{ChangesView, CommitLoad};
pub(crate) use navigation::DiffView;
use navigation::InFlight;
pub(crate) use reading::Load;
use reading::{Kept, Reading};

#[derive(Clone, Copy)]
pub(crate) struct MenuState {
    pub(crate) menu: usize,
    pub(crate) item: usize,
}

/// Overlay impilabili sopra i pannelli principali. Vivono in uno stack nel
/// `Model`: l'overlay in cima riceve per primo gli input (vedi
/// `update::dispatch_key`) ed è l'unico che il rendering disegna.
///
/// Di norma ce n'è al più uno: aprire un dialog da un menu chiude il
/// menu. Lo stack serve a chi si impila *sopra* senza sostituire, cioè
/// all'annuncio che le operazioni git non sono più possibili: può
/// arrivare in qualsiasi momento, anche mentre l'utente sta digitando in
/// un dialog, e chiuderlo riporta sotto quello che c'era.
pub(crate) enum Overlay {
    Menu(MenuState),
    Dialog(Dialog),
}

/// Una riga del pannello History. Il log non è fatto di soli commit: in
/// cima può esserci lo stato corrente della working copy, che un commit
/// non è — non ha hash né autore, e nel repository non esiste finché non
/// viene committato.
///
/// Sta qui e non in `git::domain` perché non è qualcosa che `Repo`
/// restituisce: `Repo::commits` continua a dare `Vec<Commit>`, ed è il
/// `Model` a comporre le due letture (commit e file modificati) nella
/// lista che il pannello percorre. Di conseguenza la selezione di History
/// non è più un indice in `Model::commits`.
#[derive(Debug)]
pub(crate) enum HistoryEntry {
    /// Le modifiche non ancora committate della working copy.
    WorkingCopy,
    Commit(Commit),
}

impl HistoryEntry {
    /// Se `self` e `other` sono la stessa riga, cioè la working copy
    /// entrambe o lo stesso commit, a prescindere dalla posizione: è ciò
    /// con cui la selezione sopravvive a una rilettura del log.
    fn is_same_row(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::WorkingCopy, Self::WorkingCopy) => true,
            (Self::Commit(a), Self::Commit(b)) => a.id == b.id,
            (Self::WorkingCopy, Self::Commit(_)) | (Self::Commit(_), Self::WorkingCopy) => false,
        }
    }
}

/// Il contenuto di un pannello: la lista che mostra, insieme al cursore
/// che la percorre, oppure le righe del diff con il suo scorrimento.
///
/// È l'unico posto in cui si stabilisce quale pannello mostra quale dato.
/// Prima la stessa tabella era scritta due volte — in `step`, per muovere
/// il cursore del pannello a fuoco, e in `ui::draw_panel`, per disegnarlo —
/// e le due copie potevano divergere in silenzio. "Branches" e "Remotes"
/// collassano nella stessa variante: elencano entrambi rami e si muovono e
/// si disegnano allo stesso modo, solo su liste diverse.
pub(crate) enum PanelContent<'a> {
    Branches(&'a mut ListPanel<Branch>),
    History(&'a mut ListPanel<HistoryEntry>),
    /// I file della working copy: gli unici che si possono mettere o
    /// togliere dallo stage.
    WorkingCopyFiles(&'a mut ListPanel<FileEntry>),
    /// I file toccati dal commit selezionato in History: si guardano e
    /// basta. Una lista vuota è un commit che non tocca alcun file.
    CommitFiles(&'a mut ListPanel<FileEntry>),
    /// Il pannello Changes è su un commit di cui non ci sono (ancora, o
    /// più) file da elencare: al posto della lista si mostra perché.
    ChangesNotice(ChangesNotice<'a>),
    /// Il pannello Diff non è una lista con selezione: scorre per righe.
    /// `viewport` è l'altezza con cui il pannello è stato disegnato
    /// l'ultima volta, e serve solo a fermare lo scorrimento quando
    /// l'ultima riga arriva in fondo; arriva per valore perché nessuno,
    /// oltre a `ui`, deve poterla cambiare.
    Diff {
        lines: &'a [String],
        scroll: &'a mut u16,
        viewport: u16,
    },
    /// Il pannello Diff non ha righe da mostrare: al posto del testo si
    /// dice perché.
    DiffNotice(DiffNotice<'a>),
}

/// Perché il pannello Diff non ha righe da scorrere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffNotice<'a> {
    /// In Changes non c'è un file selezionato perché la lista è vuota.
    NoFile,
    /// Il diff è stato chiesto e non è ancora arrivato; oppure Changes sta
    /// ancora aspettando i file del commit, da cui il diff dipende.
    Loading,
    /// I file del commit non si sono potuti leggere, quindi non c'è un
    /// file di cui mostrare il diff. L'errore lo dice già Changes: qui non
    /// si ripete.
    FilesNotRead,
    /// Il file è cambiato, ma è un binario: non ci sono righe da mostrare.
    Binary,
    /// Il file non ha righe cambiate da mostrare (vuoto, permessi, oppure
    /// non più modificato).
    NoTextChanges,
    /// La lettura è fallita; il messaggio è quello del backend.
    Failed(&'a str),
    /// Le operazioni git non sono più possibili: il diff non arriverà, o
    /// non arriveranno i file del commit da cui dipende.
    GitUnavailable,
}

/// Perché il pannello Changes, su un commit, non ha una lista da mostrare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangesNotice<'a> {
    /// I file sono stati chiesti e non sono ancora arrivati.
    Loading,
    /// La lettura è fallita; il messaggio è quello del backend.
    Failed(&'a str),
    /// Le operazioni git non sono più possibili: i file non arriveranno.
    GitUnavailable,
}

impl PanelContent<'_> {
    /// Muove di `delta` righe ciò che il pannello mostra: il cursore della
    /// lista, o lo scorrimento del diff.
    pub(super) fn step(&mut self, delta: i32) {
        match self {
            Self::Branches(list) => list.step(delta),
            Self::History(list) => list.step(delta),
            Self::WorkingCopyFiles(list) | Self::CommitFiles(list) => list.step(delta),
            // Nessuna riga da percorrere.
            Self::ChangesNotice(_) | Self::DiffNotice(_) => {}
            Self::Diff {
                lines,
                scroll,
                viewport,
            } => {
                let max = i32::from(max_diff_scroll(lines.len(), *viewport));
                **scroll = (i32::from(**scroll) + delta).clamp(0, max) as u16;
            }
        }
    }
}

/// Lo scorrimento massimo del pannello Diff: oltre questo valore l'ultima
/// riga del diff starebbe sopra il fondo del pannello, lasciando spazio
/// vuoto sotto.
///
/// `viewport` a 0 vuol dire "altezza non ancora nota": capita al primo
/// frame, prima che il pannello venga disegnato. In quel caso si conta una
/// riga visibile, cioè si ricade sul limite per riga (`len - 1`): scorrere
/// resta possibile e il valore non può comunque uscire dal diff: appena il
/// pannello viene disegnato arriva un `Msg::DiffViewport` e il limite vero
/// entra in vigore, riportando lo scorrimento nei ranghi.
pub(super) fn max_diff_scroll(lines: usize, viewport: u16) -> u16 {
    let visible = usize::from(viewport).max(1);
    // Il minimo con `u16::MAX` evita che un diff enorme (oltre 65 536
    // righe, un file generato ci arriva) tronchi il massimo a un valore
    // più piccolo dello scorrimento corrente, facendo saltare il diff in
    // cima; `u16` è anche il tipo che `Paragraph::scroll` accetta.
    lines.saturating_sub(visible).min(usize::from(u16::MAX)) as u16
}

pub(crate) struct Model {
    pub(crate) running: bool,
    pub(crate) focus: Panel,
    pub(crate) local_branches: ListPanel<Branch>,
    pub(crate) remote_branches: ListPanel<Branch>,
    /// Il log letto dal repository, nell'ordine in cui `Repo::commits`
    /// lo restituisce, e lo stato della working copy al momento della
    /// lettura: insieme determinano le righe di `history`.
    ///
    /// I tre campi sono privati e si cambiano solo da `set_history`, che
    /// ricompone `history`: se fossero scrivibili dall'esterno, tenerli
    /// coerenti tornerebbe a dipendere da chi scrive `update`, che è
    /// esattamente ciò che `ListPanel` ha tolto di mezzo per le liste.
    commits: Vec<Commit>,
    /// Se la working copy ha modifiche non committate. Deciso leggendo il
    /// repository, **non** guardando il pannello Changes: quel pannello
    /// mostra anche i file del commit selezionato, e la riga in cima a
    /// History non deve dipendere da cosa si sta guardando.
    working_copy_dirty: bool,
    /// Le righe del pannello History: i commit, preceduti dallo stato non
    /// committato quando c'è.
    history: ListPanel<HistoryEntry>,
    /// I file della working copy, come `Repo::files` li ha letti. Restano
    /// qui anche quando Changes mostra un commit: tornando sulla working
    /// copy si ritrovano così com'erano, cursore compreso.
    pub(crate) working_copy_files: ListPanel<FileEntry>,
    /// Che cosa mostra il pannello Changes. Segue la selezione di History
    /// e si cambia solo da `navigation` e da `lose_git`.
    changes: ChangesView,
    /// Le viste dei file di commit già arrivati, lasciate quando la
    /// selezione è passata altrove: tornarci le ritrova com'erano, cursore
    /// compreso, senza rileggerle. Un commit non cambia.
    kept_commits: Kept<CommitId, ListPanel<FileEntry>>,
    /// Che cosa mostra il pannello Diff: il diff del file selezionato in
    /// Changes, nel contesto di Changes. `None` se non c'è un file
    /// selezionato. Segue Changes e si cambia solo da `navigation` e da
    /// `lose_git`.
    diff: DiffView,
    /// I diff di file di commit già arrivati, lasciati quando la selezione
    /// è passata altrove. Quelli della working copy non si conservano: i
    /// file su disco cambiano, e tornarci li rilegge.
    kept_diffs: Kept<DiffTarget, FileDiff>,
    /// Le letture che seguono la navigazione partite e non ancora tornate:
    /// al più una per tipo (vedi `navigation`).
    in_flight: InFlight,
    pub(crate) diff_scroll: u16,
    /// Righe visibili nel pannello Diff nell'ultimo frame disegnato: il
    /// limite dello scorrimento dipende da quante righe ci stanno. Lo
    /// misura `ui`, che è l'unico a conoscere il layout, ma arriva qui come
    /// `Msg::DiffViewport` (vedi `run`), non scritto da chi disegna: il
    /// `Model` lo cambia solo `update`. Vale 0 finché il pannello non è
    /// stato disegnato nemmeno una volta.
    pub(crate) diff_viewport: u16,
    /// Se le operazioni git sono ancora possibili. Diventa falso quando il
    /// thread che le esegue non c'è più (vedi `Msg::WorkerLost`): da quel
    /// momento i dati già letti restano validi e navigabili, ma nessuna
    /// nuova lettura o scrittura arriverà mai al repository.
    ///
    /// È privato e si spegne solo da `lose_git`, che è anche ciò che
    /// rende l'annuncio all'utente irripetibile: tenere il conto fuori di
    /// qui vorrebbe dire fidarsi di chi chiama.
    git_available: bool,
    pub(crate) overlays: Vec<Overlay>,
}

impl Model {
    pub(crate) fn new() -> Self {
        Self {
            running: true,
            focus: Panel::History,
            local_branches: ListPanel::empty(),
            remote_branches: ListPanel::empty(),
            commits: Vec::new(),
            working_copy_dirty: false,
            history: ListPanel::empty(),
            working_copy_files: ListPanel::empty(),
            changes: ChangesView::WorkingCopy,
            kept_commits: Kept::new(),
            diff: None,
            kept_diffs: Kept::new(),
            in_flight: InFlight::default(),
            diff_scroll: 0,
            diff_viewport: 0,
            git_available: true,
            overlays: Vec::new(),
        }
    }

    /// Se le operazioni git sono ancora possibili. Lo legge la barra di
    /// stato, che è l'unico posto a ricordarlo dopo che il dialog
    /// d'annuncio è stato chiuso.
    pub(crate) fn git_available(&self) -> bool {
        self.git_available
    }

    /// Registra che le operazioni git non sono più possibili.
    /// Restituisce `true` solo la prima volta: le chiamate successive non
    /// cambiano nulla, così chi chiama può annunciare la perdita una
    /// volta sola senza tenere conti propri.
    ///
    /// Chiude anche le attese aperte, dei file di un commit e del diff di
    /// un file: il `Cmd` che li stava leggendo non avrà mai un esito, e i
    /// pannelli devono dirlo invece di restare in attesa per sempre.
    pub(super) fn lose_git(&mut self) -> bool {
        let first = self.git_available;
        self.git_available = false;
        if let ChangesView::Commit(shown) = &mut self.changes {
            shown.give_up_waiting();
        }
        if let Some(diff) = &mut self.diff {
            diff.give_up_waiting();
        }
        first
    }

    /// Il contenuto del pannello `panel`, prestato dal `Model`: vedi
    /// `PanelContent`. Lo usano sia `update` (per muovere il cursore del
    /// pannello a fuoco) sia il rendering (per sapere cosa disegnare), così
    /// la corrispondenza pannello -> dati resta scritta una volta sola. Il
    /// prestito è esclusivo perché il rendering di ratatui aggiorna il
    /// `ListState` (l'offset di scorrimento) mentre disegna.
    ///
    /// È `pub(crate)` perché `ui` ne ha bisogno, e chiamarla da lì non
    /// apre la strada a modificare le liste del `Model`: i metodi che
    /// cambiano elementi e selezione — e i costruttori con cui si potrebbe
    /// sovrascrivere la lista intera — sono visibili solo dentro
    /// `crate::app`, quindi il `&mut ListPanel` che ne esce serve a
    /// leggere e a prestare il cursore a ratatui, non ad altro. Fa
    /// eccezione lo scorrimento del Diff, un `&mut u16`.
    ///
    /// Vale per questa via, non per `ui` nel suo insieme: chi ha in mano un
    /// `&mut Model` — cioè `ui::view` e `ui::draw_panel`, non i pannelli —
    /// può comunque scrivere i campi `pub(crate)` del `Model`.
    pub(crate) fn content_mut(&mut self, panel: Panel) -> PanelContent<'_> {
        match panel {
            Panel::Branches => PanelContent::Branches(&mut self.local_branches),
            Panel::Remotes => PanelContent::Branches(&mut self.remote_branches),
            Panel::History => PanelContent::History(&mut self.history),
            Panel::Changes => match &mut self.changes {
                ChangesView::WorkingCopy => {
                    PanelContent::WorkingCopyFiles(&mut self.working_copy_files)
                }
                ChangesView::Commit(commit) => match commit.load_mut() {
                    CommitLoad::Loaded(files) => PanelContent::CommitFiles(files),
                    CommitLoad::Awaiting => PanelContent::ChangesNotice(ChangesNotice::Loading),
                    CommitLoad::Failed(message) => {
                        PanelContent::ChangesNotice(ChangesNotice::Failed(message))
                    }
                    CommitLoad::Unavailable => {
                        PanelContent::ChangesNotice(ChangesNotice::GitUnavailable)
                    }
                },
            },
            Panel::Diff => {
                let notice = match self.diff.as_ref().map(Reading::load) {
                    Some(Load::Loaded(FileDiff::Text(lines))) => {
                        return PanelContent::Diff {
                            lines,
                            scroll: &mut self.diff_scroll,
                            viewport: self.diff_viewport,
                        }
                    }
                    // Senza un file selezionato il motivo sta in Changes:
                    // sui file di un commit che non ci sono il diff non
                    // ha un file da cui partire, e lo dice con lo stesso
                    // stato della lista che manca.
                    None => match &self.changes {
                        ChangesView::Commit(shown) => match shown.load() {
                            Load::Awaiting => DiffNotice::Loading,
                            Load::Failed(_) => DiffNotice::FilesNotRead,
                            Load::Unavailable => DiffNotice::GitUnavailable,
                            Load::Loaded(_) => DiffNotice::NoFile,
                        },
                        ChangesView::WorkingCopy => DiffNotice::NoFile,
                    },
                    Some(Load::Awaiting) => DiffNotice::Loading,
                    Some(Load::Loaded(FileDiff::Binary)) => DiffNotice::Binary,
                    Some(Load::Loaded(FileDiff::NoTextChanges)) => DiffNotice::NoTextChanges,
                    Some(Load::Failed(message)) => DiffNotice::Failed(message),
                    Some(Load::Unavailable) => DiffNotice::GitUnavailable,
                };
                PanelContent::DiffNotice(notice)
            }
        }
    }

    /// Le righe del diff a schermo, zero se il pannello mostra un
    /// segnaposto: è su queste che si limita lo scorrimento.
    pub(super) fn diff_line_count(&self) -> usize {
        match self.diff.as_ref().map(Reading::load) {
            Some(Load::Loaded(FileDiff::Text(lines))) => lines.len(),
            Some(Load::Loaded(FileDiff::Binary | FileDiff::NoTextChanges))
            | Some(Load::Awaiting | Load::Failed(_) | Load::Unavailable)
            | None => 0,
        }
    }

    /// Il ramo su cui agisce un'azione sui rami che non lo nomina (una
    /// voce di menu, domani una scorciatoia): quello selezionato nel
    /// pannello a fuoco, se è uno dei due che elencano rami; quello
    /// selezionato in "Branches" se il focus è altrove, dove "Branches" è
    /// il pannello di riferimento. `None` se la lista da cui si pesca è
    /// vuota, cioè se non c'è alcun ramo puntato.
    ///
    /// Sta qui, accanto a `content_mut`, perché è di nuovo la domanda
    /// "quale pannello mostra quale dato", vista dal lato di chi agisce;
    /// il match è esaustivo, così un pannello nuovo costringe a dire da
    /// quale lista di rami pesca.
    ///
    /// Chi chiama decide poi *cosa* fare guardando il ramo restituito —
    /// il suo `kind`, se è il ramo corrente — e non il focus: il focus
    /// serve solo a scegliere il ramo.
    pub(super) fn target_branch(&self) -> Option<&Branch> {
        let branches = match self.focus {
            Panel::Remotes => &self.remote_branches,
            Panel::Branches | Panel::History | Panel::Changes | Panel::Diff => &self.local_branches,
        };
        branches.selected()
    }

    /// L'unico ingresso per ciò che il pannello History mostra: registra
    /// il log e lo stato della working copy, e ricompone le righe. I due
    /// dati arrivano da letture distinte e, più avanti, in momenti
    /// distinti; passando tutti da qui non esiste un modo di cambiarne uno
    /// lasciando le righe come stavano.
    pub(super) fn set_history(&mut self, commits: Vec<Commit>, working_copy_dirty: bool) {
        self.commits = commits;
        self.working_copy_dirty = working_copy_dirty;
        self.rebuild_history();
    }

    /// Ricompone le righe del pannello History: i commit del log,
    /// preceduti dalla riga dello stato non committato quando la working
    /// copy ha modifiche. Senza modifiche quella riga non esiste affatto,
    /// invece di comparire vuota.
    ///
    /// La selezione segue la riga, non l'indice: se la riga selezionata
    /// prima c'è ancora (la working copy, o lo stesso commit) il cursore la
    /// ritrova anche se si è spostata. Se non c'è più il cursore resta alla
    /// stessa altezza, limitato alla lista nuova: la riga della working copy
    /// che sparisce, perché è tornata pulita, lascia così il posto al commit
    /// più recente, e un commit riscritto (un amend) a quello che l'ha
    /// sostituito. Su una History che era vuota si parte dalla cima.
    fn rebuild_history(&mut self) {
        let mut entries = Vec::with_capacity(self.commits.len() + 1);
        if self.working_copy_dirty {
            entries.push(HistoryEntry::WorkingCopy);
        }
        // I commit vengono clonati: `commits` resta il log così com'è
        // stato letto, `history` è la lista derivata. Il backend reale ne
        // carica al più qualche centinaio, non l'intero repository.
        entries.extend(self.commits.iter().cloned().map(HistoryEntry::Commit));

        let old = std::mem::replace(&mut self.history, ListPanel::new(entries));
        let (Some(index), Some(anchor)) = (old.selected_index(), old.selected()) else {
            return;
        };
        if !self.history.select_first(|e| e.is_same_row(anchor)) {
            self.history.select_nearest(index);
        }
    }

    /// Che cosa mostra il pannello Changes.
    pub(crate) fn changes(&self) -> &ChangesView {
        &self.changes
    }

    /// Che cosa mostra il pannello Diff.
    #[cfg(test)]
    pub(crate) fn diff(&self) -> &DiffView {
        &self.diff
    }

    /// Le righe di History, per i test che le ispezionano da fuori: la
    /// lista è derivata, quindi si legge e non si scrive.
    #[cfg(test)]
    pub(crate) fn history(&self) -> &ListPanel<HistoryEntry> {
        &self.history
    }

    /// L'overlay in cima allo stack, se è un dialog.
    pub(crate) fn dialog(&self) -> Option<&Dialog> {
        match self.overlays.last() {
            Some(Overlay::Dialog(d)) => Some(d),
            _ => None,
        }
    }

    /// Come `dialog`, ma mutabile (serve ai test che manipolano il dialog
    /// aperto senza passare da `update`).
    #[cfg(test)]
    pub(crate) fn dialog_mut(&mut self) -> Option<&mut Dialog> {
        match self.overlays.last_mut() {
            Some(Overlay::Dialog(d)) => Some(d),
            _ => None,
        }
    }

    /// L'overlay in cima allo stack, se è un menu. `MenuState` è `Copy`.
    pub(crate) fn menu(&self) -> Option<MenuState> {
        match self.overlays.last() {
            Some(Overlay::Menu(m)) => Some(*m),
            _ => None,
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod content_tests;
#[cfg(test)]
mod history_tests;
