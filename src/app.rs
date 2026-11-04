//
// src/app.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! `Model` (stato dell'applicazione) e `update` (`Model`, `Msg` -> `Vec<Cmd>`).
//! `update` è puro: nessun I/O, nessun rendering, nessun `unwrap`. Le
//! operazioni verso git passano da un `Cmd` restituito qui, inviato al
//! `Worker` (vedi `worker.rs`) il cui `Msg` di esito torna, più tardi, a un
//! giro successivo di `run`.

use std::io;
use std::time::Duration;

use ratatui::{
    crossterm::event::{self, Event, KeyEventKind},
    DefaultTerminal,
};

use crate::dialog::{Action, Dialog};
#[cfg(test)]
use crate::effects;
use crate::git::domain::{Branch, BranchKind, Commit, FileEntry};
use crate::git::repo::GitError;
use crate::input;
use crate::list_panel::ListPanel;
use crate::msg::{Cmd, Msg};
use crate::ui;
use crate::worker::Worker;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Panel {
    Branches,
    Remotes,
    History,
    Changes,
    Diff,
}

impl Panel {
    const ALL: [Panel; 5] = [
        Panel::Branches,
        Panel::Remotes,
        Panel::History,
        Panel::Changes,
        Panel::Diff,
    ];

    fn index(self) -> usize {
        self as usize
    }
    pub(crate) fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }
    pub(crate) fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
    pub(crate) fn title(self) -> &'static str {
        match self {
            Panel::Branches => "Branches",
            Panel::Remotes => "Remotes",
            Panel::History => "History",
            Panel::Changes => "Changes",
            Panel::Diff => "Diff",
        }
    }
}

/// Azione associata a una voce di menu. Il dispatch in `activate` avviene su
/// questo tipo, non sul testo della label: rinominare una label non cambia
/// più silenziosamente il comportamento (vedi `activate`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MenuAction {
    OpenRepository,
    CloneRepository,
    Quit,
    NewBranch,
    Checkout,
    DeleteBranch,
    Commit,
    Amend,
    Stash,
    ZoomPanel,
    Refresh,
    About,
}

pub(crate) const MENUS: &[(&str, &[(&str, MenuAction)])] = &[
    (
        "File",
        &[
            ("Open repository...", MenuAction::OpenRepository),
            ("Clone...", MenuAction::CloneRepository),
            ("Quit", MenuAction::Quit),
        ],
    ),
    (
        "Branch",
        &[
            ("New branch...", MenuAction::NewBranch),
            ("Checkout", MenuAction::Checkout),
            ("Delete", MenuAction::DeleteBranch),
        ],
    ),
    (
        "Commit",
        &[
            ("Commit...", MenuAction::Commit),
            ("Amend", MenuAction::Amend),
            ("Stash", MenuAction::Stash),
        ],
    ),
    (
        "View",
        &[
            ("Zoom panel", MenuAction::ZoomPanel),
            ("Refresh", MenuAction::Refresh),
        ],
    ),
    ("Help", &[("About", MenuAction::About)]),
];

#[derive(Clone, Copy)]
pub(crate) struct MenuState {
    pub(crate) menu: usize,
    pub(crate) item: usize,
}

/// Overlay impilabili sopra i pannelli principali. Vivono in uno stack nel
/// `Model`: l'overlay in cima riceve per primo gli input (vedi
/// `dispatch_key`). Oggi lo stack non supera mai un elemento, ma la lettura
/// guarda comunque solo la cima.
pub(crate) enum Overlay {
    Menu(MenuState),
    Dialog(Dialog),
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
    Commits(&'a mut ListPanel<Commit>),
    Files(&'a mut ListPanel<FileEntry>),
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
}

impl PanelContent<'_> {
    /// Muove di `delta` righe ciò che il pannello mostra: il cursore della
    /// lista, o lo scorrimento del diff.
    fn step(&mut self, delta: i32) {
        match self {
            Self::Branches(list) => list.step(delta),
            Self::Commits(list) => list.step(delta),
            Self::Files(list) => list.step(delta),
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
fn max_diff_scroll(lines: usize, viewport: u16) -> u16 {
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
    pub(crate) commits: ListPanel<Commit>,
    pub(crate) files: ListPanel<FileEntry>,
    pub(crate) diff: Vec<String>,
    pub(crate) diff_scroll: u16,
    /// Righe visibili nel pannello Diff nell'ultimo frame disegnato: il
    /// limite dello scorrimento dipende da quante righe ci stanno. Lo
    /// misura `ui`, che è l'unico a conoscere il layout, ma arriva qui come
    /// `Msg::DiffViewport` (vedi `run`), non scritto da chi disegna: il
    /// `Model` lo cambia solo `update`. Vale 0 finché il pannello non è
    /// stato disegnato nemmeno una volta.
    pub(crate) diff_viewport: u16,
    pub(crate) overlays: Vec<Overlay>,
}

impl Model {
    pub(crate) fn new() -> Self {
        Self {
            running: true,
            focus: Panel::History,
            local_branches: ListPanel::default(),
            remote_branches: ListPanel::default(),
            commits: ListPanel::default(),
            files: ListPanel::default(),
            diff: Vec::new(),
            diff_scroll: 0,
            diff_viewport: 0,
            overlays: Vec::new(),
        }
    }

    /// Il contenuto del pannello `panel`, prestato dal `Model`: vedi
    /// `PanelContent`. Lo usano sia `update` (per muovere il cursore del
    /// pannello a fuoco) sia `ui::view_model` (per sapere cosa disegnare),
    /// così la corrispondenza pannello -> dati resta scritta una volta
    /// sola. Il prestito è esclusivo perché il rendering di ratatui
    /// aggiorna il `ListState` (l'offset di scorrimento) mentre disegna.
    ///
    /// È `pub(crate)` perché `ui` ne ha bisogno, ma da lì l'unico chiamante
    /// ammesso è `ui::view_model`: un pannello che lo invocasse
    /// direttamente otterrebbe un `&mut ListPanel` e potrebbe modificare il
    /// `Model` mentre disegna.
    pub(crate) fn content_mut(&mut self, panel: Panel) -> PanelContent<'_> {
        match panel {
            Panel::Branches => PanelContent::Branches(&mut self.local_branches),
            Panel::Remotes => PanelContent::Branches(&mut self.remote_branches),
            Panel::History => PanelContent::Commits(&mut self.commits),
            Panel::Changes => PanelContent::Files(&mut self.files),
            Panel::Diff => PanelContent::Diff {
                lines: &self.diff,
                scroll: &mut self.diff_scroll,
                viewport: self.diff_viewport,
            },
        }
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

/// `Cmd` da eseguire all'avvio, prima del primo frame (vedi `main.rs`).
pub(crate) fn init_cmds() -> Vec<Cmd> {
    vec![Cmd::LoadAll]
}

/// Intervallo di `event::poll`: abbastanza breve da non introdurre un
/// ritardo percepibile nel drenare i `Msg` del worker tra un tasto e
/// l'altro, abbastanza lungo da non consumare CPU in un loop stretto da
/// ferma.
const POLL_TIMEOUT: Duration = Duration::from_millis(50);

/// Ciclo principale: disegna, aspetta fino a `POLL_TIMEOUT` un tasto (non
/// bloccante: un backend git reale può impiegare più di un giro a
/// rispondere, vedi `worker.rs`), lo traduce in `Msg` (vedi
/// `input::key_to_msg`) e lo passa a `dispatch`; poi, con o senza tasto,
/// drena i `Msg` già pronti dal worker e li applica con `update`.
pub(crate) fn run(
    model: &mut Model,
    worker: &Worker,
    terminal: &mut DefaultTerminal,
) -> io::Result<()> {
    // L'altezza del pannello Diff la conosce solo chi calcola il layout.
    // Vive qui, fuori dal `Model`, e rientra come `Msg` quando cambia:
    // disegnare non scrive nel `Model`.
    let mut diff_viewport = model.diff_viewport;

    while model.running {
        terminal.draw(|f| ui::view(f, model, &mut diff_viewport))?;
        if diff_viewport != model.diff_viewport {
            dispatch(model, worker, Msg::DiffViewport(diff_viewport));
        }

        if event::poll(POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(msg) = input::key_to_msg(key) {
                        dispatch(model, worker, msg);
                    }
                }
            }
            // Gli eventi Resize fanno semplicemente ridisegnare al giro dopo.
        }

        while let Some(msg) = worker.try_recv() {
            update(model, msg);
        }
    }
    Ok(())
}

/// Applica `msg` e inoltra al worker ogni `Cmd` richiesto da `update`. A
/// differenza del vecchio `dispatch` sincrono (sostituito da questo), il
/// `Msg` di esito non torna qui: arriva più tardi, drenato da `run` tramite
/// `Worker::try_recv`.
fn dispatch(model: &mut Model, worker: &Worker, msg: Msg) {
    for cmd in update(model, msg) {
        worker.send(cmd);
    }
}

pub(crate) fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Quit => {
            model.running = false;
            Vec::new()
        }
        Msg::OpenMenuByHotkey(c) => {
            open_menu_by_hotkey(model, c);
            Vec::new()
        }
        Msg::DiffViewport(height) => {
            apply_diff_viewport(model, height);
            Vec::new()
        }
        Msg::Loaded {
            branches,
            commits,
            files,
            diff,
        } => {
            apply_loaded(model, branches, commits, files, diff);
            Vec::new()
        }
        Msg::BranchCreated { name, result } => {
            apply_branch_created(model, name, result);
            Vec::new()
        }
        Msg::BranchDeleted { name, result } => {
            apply_branch_deleted(model, name, result);
            Vec::new()
        }
        // Le varianti restanti sono scorciatoie "di tasto" generiche: il
        // loro significato dipende da quale overlay è in cima (dialog >
        // menu > pannelli principali), vedi `dispatch_key`.
        key_msg => dispatch_key(model, key_msg),
    }
}

fn dispatch_key(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    // Oggi un solo overlay è mai attivo: se questo smette di essere vero,
    // "cima dello stack" e "overlay attivo" divergerebbero silenziosamente.
    debug_assert!(model.overlays.len() <= 1, "più di un overlay impilato");
    match model.overlays.last() {
        Some(Overlay::Dialog(_)) => update_dialog(model, msg),
        Some(Overlay::Menu(_)) => update_menu(model, msg),
        None => update_main(model, msg),
    }
}

fn open_menu_by_hotkey(model: &mut Model, c: char) {
    if model.dialog().is_some() {
        return;
    }
    if let Some(i) = MENUS
        .iter()
        .position(|(n, _)| n.starts_with(c.to_ascii_uppercase()))
    {
        let state = MenuState { menu: i, item: 0 };
        match model.overlays.last_mut() {
            Some(Overlay::Menu(m)) => *m = state,
            _ => model.overlays.push(Overlay::Menu(state)),
        }
    }
}

/// Registra l'altezza del pannello Diff e riporta subito lo scorrimento
/// entro il nuovo limite: ingrandendo il terminale, uno scorrimento che
/// prima era al fondo lascerebbe altrimenti righe vuote sotto l'ultima riga
/// del diff fino al tasto successivo.
fn apply_diff_viewport(model: &mut Model, height: u16) {
    model.diff_viewport = height;
    model.diff_scroll = model
        .diff_scroll
        .min(max_diff_scroll(model.diff.len(), height));
}

fn apply_loaded(
    model: &mut Model,
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    files: Vec<FileEntry>,
    diff: Vec<String>,
) {
    let (local, remote) = split_branches(branches);
    model.local_branches.set(local);
    model.remote_branches.set(remote);
    model.commits.set(commits);
    model.files.set(files);
    model.diff = diff;
    model.diff_scroll = 0;
}

/// Divide i riferimenti restituiti da `Repo::branches` nelle due liste che
/// la UI mostra. I tag esistono nel dominio e i backend continuano a
/// restituirli (la validazione dei nomi duplicati si appoggia alla lista
/// completa), ma non hanno un pannello: qui vengono scartati.
fn split_branches(branches: Vec<Branch>) -> (Vec<Branch>, Vec<Branch>) {
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for b in branches {
        match b.kind {
            BranchKind::Local => local.push(b),
            BranchKind::Remote => remote.push(b),
            BranchKind::Tag => {}
        }
    }
    (local, remote)
}

fn apply_branch_created(model: &mut Model, name: String, result: Result<Branch, GitError>) {
    match result {
        Ok(branch) => {
            // Aggiornamento locale invece di una ricarica dal
            // repository: il ramo finisce in coda ai locali, mentre una
            // rilettura lo collocherebbe nell'ordine in cui il repository
            // elenca i suoi riferimenti. È una divergenza solo di
            // ordinamento, che sparirà quando l'esito Ok potrà chiedere una
            // ricarica completa.
            model.local_branches.push_selected(branch);
        }
        Err(err) => {
            // Riapre il dialog con il testo già digitato, invece di un
            // messaggio d'errore che lo scarterebbe e costringerebbe a
            // ridigitarlo da capo.
            let mut d = Dialog::input(
                "New branch",
                format!("Branch name:\n{err}"),
                Action::NewBranch,
            );
            d.input = name;
            model.overlays.push(Overlay::Dialog(d));
        }
    }
}

fn apply_branch_deleted(model: &mut Model, name: String, result: Result<(), GitError>) {
    match result {
        Ok(()) => {
            // Aggiornamento locale invece di una ricarica: la lista
            // contiene solo rami locali, e i loro nomi sono unici, quindi
            // il confronto per nome toglie esattamente il ramo cancellato.
            model.local_branches.retain(|b| b.name != name);
        }
        Err(err) => {
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "Delete branch",
                err.to_string(),
            )));
        }
    }
}

fn update_menu(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let Some(&Overlay::Menu(mut m)) = model.overlays.last() else {
        return Vec::new();
    };
    let n_menus = MENUS.len();
    let n_items = MENUS[m.menu].1.len();
    match msg {
        Msg::Esc | Msg::F10 => {
            model.overlays.pop();
            return Vec::new();
        }
        Msg::Left => {
            m.menu = (m.menu + n_menus - 1) % n_menus;
            m.item = 0;
        }
        Msg::Right => {
            m.menu = (m.menu + 1) % n_menus;
            m.item = 0;
        }
        Msg::Up => m.item = (m.item + n_items - 1) % n_items,
        Msg::Down => m.item = (m.item + 1) % n_items,
        Msg::Enter => {
            model.overlays.pop();
            let (label, action) = MENUS[m.menu].1[m.item];
            activate(model, label, action);
            return Vec::new();
        }
        _ => {}
    }
    *model
        .overlays
        .last_mut()
        .expect("il top era un Menu un istante fa") = Overlay::Menu(m);
    Vec::new()
}

fn update_dialog(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let Some(Overlay::Dialog(dialog)) = model.overlays.last_mut() else {
        return Vec::new();
    };
    let Some(accepted) = dialog.on_msg(msg) else {
        return Vec::new();
    };
    let Some(Overlay::Dialog(dialog)) = model.overlays.pop() else {
        unreachable!("appena verificato Some(Overlay::Dialog(_)) poche righe sopra")
    };
    if accepted {
        apply_dialog(dialog)
    } else {
        Vec::new()
    }
}

fn activate(model: &mut Model, label: &str, action: MenuAction) {
    match action {
        MenuAction::Quit => model.running = false,
        MenuAction::About => {
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "About",
                "TurboVersion\nTurbo Vision meets version control.",
            )));
        }
        MenuAction::NewBranch => {
            model.overlays.push(Overlay::Dialog(Dialog::input(
                "New branch",
                "Branch name:",
                Action::NewBranch,
            )));
        }
        MenuAction::DeleteBranch => ask_delete_branch(model),
        MenuAction::OpenRepository
        | MenuAction::CloneRepository
        | MenuAction::Checkout
        | MenuAction::Commit
        | MenuAction::Amend
        | MenuAction::Stash
        | MenuAction::ZoomPanel
        | MenuAction::Refresh => {
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "Not implemented",
                format!("\"{label}\" is not available yet."),
            )));
        }
    }
}

/// Chiede conferma per cancellare il ramo selezionato nel pannello
/// "Branches". L'operazione guarda sempre quella selezione, anche con il
/// focus altrove, come è normale per una voce di menu. L'eccezione è
/// "Remotes": è l'altro pannello che elenca rami, quindi è l'unico in cui
/// "Delete" sarebbe ambiguo su quale dei due elenchi stia agendo. Lì si
/// rifiuta invece di cancellare un ramo a cui l'utente non stava pensando.
fn ask_delete_branch(model: &mut Model) {
    if model.focus == Panel::Remotes {
        model.overlays.push(Overlay::Dialog(Dialog::info(
            "Delete branch",
            "Remote branches cannot be deleted here.",
        )));
        return;
    }
    let Some(b) = model.local_branches.selected() else {
        model.overlays.push(Overlay::Dialog(Dialog::info(
            "Delete branch",
            "No branch selected.",
        )));
        return;
    };
    let dialog = if b.current {
        Dialog::info(
            "Delete branch",
            format!("Cannot delete the current branch '{}'.", b.name),
        )
    } else {
        let mut d = Dialog::confirm(
            "Delete branch",
            format!("Delete branch '{}'?", b.name),
            Action::DeleteBranch(b.name.clone()),
        );
        d.button = 1; // azione distruttiva: default su "No"
        d
    };
    model.overlays.push(Overlay::Dialog(dialog));
}

/// Traduce l'accettazione di un dialog nell'effetto da richiedere, se c'è.
fn apply_dialog(dialog: Dialog) -> Vec<Cmd> {
    match dialog.action {
        Action::Info => Vec::new(),
        Action::NewBranch => vec![Cmd::CreateBranch(dialog.input.trim().to_string())],
        Action::DeleteBranch(name) => vec![Cmd::DeleteBranch(name)],
    }
}

fn update_main(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::F10 => model
            .overlays
            .push(Overlay::Menu(MenuState { menu: 0, item: 0 })),
        Msg::Tab => model.focus = model.focus.next(),
        Msg::BackTab => model.focus = model.focus.prev(),
        Msg::Up | Msg::Char { c: 'k', .. } => step(model, -1),
        Msg::Down | Msg::Char { c: 'j', .. } => step(model, 1),
        Msg::Char { c: ' ', .. } if model.focus == Panel::Changes => {
            // Su una lista vuota non c'è alcun file selezionato (vedi
            // l'invariante di `ListPanel`): niente da commutare.
            if let Some(file) = model.files.selected_mut() {
                file.staged = !file.staged;
            }
        }
        _ => {}
    }
    Vec::new()
}

/// Muove di `delta` righe il pannello a fuoco, qualunque sia il suo
/// contenuto: quale lista tocchi lo decide `Model::content_mut`, non questa
/// funzione.
fn step(model: &mut Model, delta: i32) {
    let focus = model.focus;
    if focus == Panel::History {
        // Una nuova selezione mostra un commit diverso: lo scroll del diff
        // precedente non ha più senso.
        model.diff_scroll = 0;
    }
    model.content_mut(focus).step(delta);
}

/// Un `Model` popolato con gli stessi dati di un avvio reale (passati
/// attraverso `init_cmds` + `effects::run`, come farebbe `main.rs`), per i
/// test di `app` e di `ui` che hanno bisogno di dati non vuoti.
#[cfg(test)]
pub(crate) fn model_for_tests() -> Model {
    let mut model = Model::new();
    let mut repo = crate::git::mock::MockGitRepo::new();
    for cmd in init_cmds() {
        let msg = effects::run(cmd, &mut repo);
        update(&mut model, msg);
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::mock::MockGitRepo;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::ops::{Deref, DerefMut};

    /// Replica il ciclo di `run` (traduzione del tasto, `update`, eventuali
    /// effetti contro un `MockGitRepo`) per i test, esponendo il `Model`
    /// tramite `Deref`/`DerefMut`: un wrapper di comodo solo per i test, non
    /// un tipo pubblico dell'app.
    ///
    /// A differenza di `run`/`dispatch` di produzione, qui non passa da un
    /// `Worker`: comporrebbe `update()` + `effects::run()` contro un vero
    /// thread in background, rendendo i test dipendenti dal timing di quel
    /// thread. Compone invece `update()` + `effects::run()` direttamente
    /// contro `MockGitRepo`, esattamente come `model_for_tests` sopra: resta
    /// deterministico e veloce, e non duplica la logica di smistamento (che
    /// vive comunque solo in `update`).
    struct TestApp {
        model: Model,
        repo: MockGitRepo,
    }

    impl TestApp {
        fn new() -> Self {
            let mut model = Model::new();
            let mut repo = MockGitRepo::new();
            for cmd in init_cmds() {
                let msg = effects::run(cmd, &mut repo);
                update(&mut model, msg);
            }
            Self { model, repo }
        }

        fn send(&mut self, msg: Msg) {
            for cmd in update(&mut self.model, msg) {
                let result = effects::run(cmd, &mut self.repo);
                // Un Cmd produce un solo Msg di esito, mai altri Cmd: nessun
                // rischio di ricorsione.
                update(&mut self.model, result);
            }
        }
    }

    impl Deref for TestApp {
        type Target = Model;
        fn deref(&self) -> &Model {
            &self.model
        }
    }

    impl DerefMut for TestApp {
        fn deref_mut(&mut self) -> &mut Model {
            &mut self.model
        }
    }

    fn press_mods(app: &mut TestApp, code: KeyCode, mods: KeyModifiers) {
        if let Some(msg) = input::key_to_msg(KeyEvent::new(code, mods)) {
            app.send(msg);
        }
    }

    fn press(app: &mut TestApp, code: KeyCode) {
        press_mods(app, code, KeyModifiers::NONE);
    }

    fn type_text(app: &mut TestApp, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    /// Trova (menu, item) per un'azione, cercando in `MENUS` invece di usare
    /// indici magici nei test: una voce che cambia posizione non rompe i test.
    fn menu_position(action: MenuAction) -> (usize, usize) {
        for (mi, (_, items)) in MENUS.iter().enumerate() {
            if let Some(ii) = items.iter().position(|(_, a)| *a == action) {
                return (mi, ii);
            }
        }
        panic!("azione {action:?} non trovata in MENUS");
    }

    /// Indice di un ramo locale cercato per nome: i test non dipendono
    /// così dall'ordine dei dati di partenza.
    fn local_position(app: &TestApp, name: &str) -> usize {
        app.local_branches
            .items()
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("ramo locale {name:?} non trovato"))
    }

    fn run_menu_action(app: &mut TestApp, action: MenuAction) {
        let (menu, item) = menu_position(action);
        app.overlays.push(Overlay::Menu(MenuState { menu, item }));
        press(app, KeyCode::Enter);
    }

    #[test]
    fn about_opens_a_dialog_and_esc_closes_it() {
        let mut app = TestApp::new();
        run_menu_action(&mut app, MenuAction::About);
        assert!(app.dialog().is_some());
        assert!(app.menu().is_none());
        press(&mut app, KeyCode::Esc);
        assert!(app.dialog().is_none());
    }

    #[test]
    fn dialog_is_modal() {
        let mut app = TestApp::new();
        app.overlays
            .push(Overlay::Dialog(Dialog::input("t", "x", Action::NewBranch)));
        let focus = app.focus;

        type_text(&mut app, "q"); // finisce nel campo, non esce
        press(&mut app, KeyCode::F(10)); // non apre il menu
        press_mods(&mut app, KeyCode::Char('f'), KeyModifiers::ALT);

        assert!(app.running);
        assert!(app.menu().is_none());
        assert_eq!(app.focus, focus);
        assert_eq!(app.dialog().unwrap().input, "q");
    }

    #[test]
    fn q_in_main_panel_no_longer_quits() {
        let mut app = TestApp::new();
        press(&mut app, KeyCode::Char('q'));
        assert!(app.running);
    }

    #[test]
    fn alt_x_quits_even_with_dialog_open() {
        let mut app = TestApp::new();
        run_menu_action(&mut app, MenuAction::About);
        press_mods(&mut app, KeyCode::Char('x'), KeyModifiers::ALT);
        assert!(!app.running);
    }

    #[test]
    fn ctrl_c_quits_even_with_dialog_or_menu_open() {
        let mut app = TestApp::new();
        press_mods(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!app.running);

        let mut app = TestApp::new();
        run_menu_action(&mut app, MenuAction::About);
        press_mods(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!app.running);
    }

    #[test]
    fn altgr_like_combo_is_not_swallowed_as_a_menu_shortcut() {
        // Alt+Ctrl è come molte piattaforme riportano AltGr: non deve aprire
        // un menu, e il carattere deve comunque raggiungere il campo di testo.
        let mut app = TestApp::new();
        app.overlays
            .push(Overlay::Dialog(Dialog::input("t", "x", Action::NewBranch)));
        press_mods(
            &mut app,
            KeyCode::Char('e'),
            KeyModifiers::ALT | KeyModifiers::CONTROL,
        );
        assert!(app.menu().is_none());
        assert_eq!(app.dialog().unwrap().input, "e");
    }

    #[test]
    fn new_branch_is_created_and_selected() {
        let mut app = TestApp::new();
        let n = app.local_branches.len();
        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, "  feature/x ");
        press(&mut app, KeyCode::Enter);

        assert!(app.dialog().is_none());
        assert_eq!(app.local_branches.len(), n + 1);
        assert_eq!(app.local_branches.items()[n].name, "feature/x");
        assert_eq!(app.local_branches.items()[n].kind, BranchKind::Local);
        assert_eq!(app.local_branches.selected_index(), Some(n));
    }

    #[test]
    fn new_branch_cancel_and_validation_keeps_typed_text_for_correction() {
        let mut app = TestApp::new();
        let n = app.local_branches.len();

        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, "abc");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.local_branches.len(), n);

        for bad in [
            "",
            "two words",
            "develop",
            "-oops",
            "a..b",
            "a@{b",
            "trailing/",
            "trailing.lock",
        ] {
            run_menu_action(&mut app, MenuAction::NewBranch);
            type_text(&mut app, bad);
            press(&mut app, KeyCode::Enter);

            assert!(app.dialog().is_some(), "errore atteso per {bad:?}");
            let d = app.dialog().unwrap();
            assert_eq!(
                d.kind,
                crate::dialog::Kind::Input,
                "deve restare modificabile per correggere il testo, per {bad:?}"
            );
            assert_eq!(
                d.input, bad,
                "il testo digitato non deve andare perso per {bad:?}"
            );

            press(&mut app, KeyCode::Esc);
            assert!(app.dialog().is_none());
            assert_eq!(app.local_branches.len(), n);
        }
    }

    #[test]
    fn current_branch_cannot_be_deleted() {
        let mut app = TestApp::new();
        let n = app.local_branches.len();
        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.local_branches.len(), n);
    }

    #[test]
    fn loaded_branches_are_split_by_kind_and_tags_are_dropped() {
        let app = TestApp::new();
        assert!(app
            .local_branches
            .items()
            .iter()
            .all(|b| b.kind == BranchKind::Local));
        assert!(app
            .remote_branches
            .items()
            .iter()
            .all(|b| b.kind == BranchKind::Remote));
        assert!(app
            .local_branches
            .items()
            .iter()
            .any(|b| b.name == "develop"));
        assert!(app
            .remote_branches
            .items()
            .iter()
            .any(|b| b.name == "origin/main"));
        assert!(app
            .local_branches
            .items()
            .iter()
            .chain(app.remote_branches.items())
            .all(|b| b.name != "v0.1.0"));
    }

    #[test]
    fn delete_from_the_remotes_panel_is_refused_instead_of_deleting_a_local_branch() {
        let mut app = TestApp::new();
        let n = app.local_branches.len();
        app.focus = Panel::Remotes;
        // Un locale cancellabile resta selezionato nell'altro pannello: la
        // richiesta non deve ricadere su di lui.
        let i = local_position(&app, "develop");
        app.local_branches.select(i);

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
        assert!(app
            .dialog()
            .unwrap()
            .text
            .contains("Remote branches cannot be deleted"));
        press(&mut app, KeyCode::Enter);

        assert_eq!(app.local_branches.len(), n);
        assert!(app
            .local_branches
            .items()
            .iter()
            .any(|b| b.name == "develop"));
    }

    #[test]
    fn deleting_without_any_local_branch_shows_a_message_instead_of_doing_nothing_silently() {
        let mut app = TestApp::new();
        // "Nessun ramo selezionato" e "nessun ramo" sono ormai la stessa
        // cosa: `ListPanel` non ammette una lista piena senza selezione.
        update(
            &mut app.model,
            Msg::Loaded {
                branches: Vec::new(),
                commits: Vec::new(),
                files: Vec::new(),
                diff: Vec::new(),
            },
        );
        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
        assert!(app.dialog().unwrap().text.contains("No branch selected"));
    }

    #[test]
    fn delete_branch_asks_confirmation_defaulting_to_no() {
        let mut app = TestApp::new();
        let n = app.local_branches.len();
        let i = local_position(&app, "develop");
        app.local_branches.select(i);

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        press(&mut app, KeyCode::Enter); // default: No
        assert_eq!(app.local_branches.len(), n);

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.local_branches.len(), n - 1);
        assert!(app
            .local_branches
            .items()
            .iter()
            .all(|b| b.name != "develop"));
    }

    #[test]
    fn deleting_last_branch_keeps_selection_in_range() {
        let mut app = TestApp::new();
        // Aggiunge un ramo locale in coda (gli unici cancellabili), lo
        // seleziona e lo cancella: la selezione deve restare nei limiti
        // della lista ridotta.
        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, "temp");
        press(&mut app, KeyCode::Enter);
        let last = app.local_branches.len() - 1;
        assert_eq!(app.local_branches.selected_index(), Some(last));

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.local_branches.selected_index(), Some(last - 1));
    }

    #[test]
    fn space_on_an_empty_changes_list_does_not_panic() {
        let mut app = TestApp::new();
        update(
            &mut app.model,
            Msg::Loaded {
                branches: Vec::new(),
                commits: Vec::new(),
                files: Vec::new(),
                diff: Vec::new(),
            },
        );
        assert_eq!(
            app.files.selected_index(),
            None,
            "lista vuota, nessuna riga"
        );

        app.focus = Panel::Changes;
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Char(' '));

        assert!(app.files.is_empty());
    }

    #[test]
    fn each_branch_panel_moves_only_its_own_selection() {
        let mut app = TestApp::new();
        assert!(app.remote_branches.len() > 1, "servono almeno due remoti");

        app.focus = Panel::Remotes;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.remote_branches.selected_index(), Some(1));
        assert_eq!(
            app.local_branches.selected_index(),
            Some(0),
            "i locali non si muovono"
        );

        app.focus = Panel::Branches;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.local_branches.selected_index(), Some(1));
        assert_eq!(
            app.remote_branches.selected_index(),
            Some(1),
            "i remoti non si muovono"
        );
    }

    #[test]
    fn unimplemented_items_report_it() {
        let mut app = TestApp::new();
        run_menu_action(&mut app, MenuAction::OpenRepository);
        assert!(app.dialog().unwrap().text.contains("not available"));
    }

    #[test]
    fn panel_next_and_prev_wrap_around() {
        assert_eq!(Panel::Diff.next(), Panel::Branches);
        assert_eq!(Panel::Branches.prev(), Panel::Diff);
        assert_eq!(Panel::Branches.next(), Panel::Remotes);
        assert_eq!(Panel::Remotes.next(), Panel::History);
        assert_eq!(Panel::History.prev(), Panel::Remotes);
        assert_eq!(Panel::Changes.prev(), Panel::History);
    }

    /// Il pannello a fuoco è l'unico a muoversi, anche quando il contenuto
    /// non è una lista: `Model::content_mut` sceglie il cursore, `step` non
    /// sa più quale sia.
    #[test]
    fn only_the_focused_panel_moves() {
        let mut app = TestApp::new();
        app.focus = Panel::Diff;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.diff_scroll, 1);
        assert_eq!(app.commits.selected_index(), Some(0));
        assert_eq!(app.files.selected_index(), Some(0));

        app.focus = Panel::Changes;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.files.selected_index(), Some(1));
        assert_eq!(app.commits.selected_index(), Some(0));
    }

    #[test]
    fn diff_scroll_resets_when_history_selection_changes() {
        let mut app = TestApp::new();
        app.focus = Panel::Diff;
        app.diff_scroll = 3;
        app.focus = Panel::History;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.diff_scroll, 0);
    }

    /// Un `TestApp` con il focus sul Diff, `lines` righe di diff e un
    /// pannello alto `viewport` righe, annunciato come lo annuncerebbe un
    /// frame appena disegnato (`viewport` a 0 vuol dire "mai disegnato").
    fn app_showing_diff(lines: usize, viewport: u16) -> TestApp {
        let mut app = TestApp::new();
        app.focus = Panel::Diff;
        app.diff = (0..lines).map(|i| format!("line {i}")).collect();
        app.diff_scroll = 0;
        app.send(Msg::DiffViewport(viewport));
        app
    }

    #[test]
    fn diff_scroll_stops_with_the_last_line_at_the_bottom_of_the_panel() {
        let mut app = app_showing_diff(10, 4);
        for _ in 0..20 {
            press(&mut app, KeyCode::Down);
        }
        // Righe 6..9 visibili: l'ultima è in fondo al pannello, non in cima.
        assert_eq!(app.diff_scroll, 6);
    }

    #[test]
    fn a_diff_shorter_than_the_panel_does_not_scroll() {
        let mut app = app_showing_diff(3, 10);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.diff_scroll, 0);
    }

    /// Ingrandire il terminale non deve lasciare righe vuote sotto
    /// l'ultima riga del diff: il limite si riapplica quando arriva la
    /// nuova altezza, non al tasto successivo.
    #[test]
    fn a_taller_panel_pulls_the_scroll_back_to_the_bottom() {
        let mut app = app_showing_diff(10, 4);
        for _ in 0..20 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.diff_scroll, 6);

        app.send(Msg::DiffViewport(19));
        assert_eq!(app.diff_viewport, 19);
        assert_eq!(app.diff_scroll, 0, "il diff ci sta tutto");

        app.send(Msg::DiffViewport(7));
        assert_eq!(app.diff_scroll, 0, "rimpicciolire non muove lo scroll");
    }

    /// L'anello completo: il pannello viene disegnato, la sua altezza
    /// rientra nel `Model` e lo scorrimento si ferma dove il pannello
    /// finisce. Gli altri test di scorrimento fissano l'altezza a mano e
    /// non passano da `ui`.
    #[test]
    fn scrolling_a_drawn_panel_stops_at_its_real_bottom() {
        let mut app = app_showing_diff(100, 0);
        ui::render(100, 30, &mut app.model);
        let visible = app.diff_viewport;
        assert!(visible > 0, "il pannello è stato disegnato");

        for _ in 0..200 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.diff_scroll, 100 - visible);

        // L'ultima riga del diff è davvero l'ultima riga del pannello.
        let out = ui::render(100, 30, &mut app.model);
        assert!(out.contains("line 99"), "{out}");
        assert!(!out.contains("line 85"), "{out}");
    }

    #[test]
    fn an_empty_diff_or_an_unknown_height_scrolls_without_panicking() {
        let mut app = app_showing_diff(0, 0);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.diff_scroll, 0);

        app.send(Msg::DiffViewport(10));
        press(&mut app, KeyCode::Down);
        assert_eq!(app.diff_scroll, 0);

        // Altezza ancora sconosciuta su un diff non vuoto: lo scorrimento
        // non si blocca, e resta comunque dentro il diff.
        let mut app = app_showing_diff(3, 0);
        for _ in 0..10 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.diff_scroll, 2);
    }

    /// Un diff più lungo di quanto `u16` possa contare non fa saltare lo
    /// scorrimento in cima né panicare: il limite si ferma a `u16::MAX`.
    #[test]
    fn a_diff_longer_than_u16_clamps_instead_of_wrapping_around() {
        assert_eq!(max_diff_scroll(usize::MAX, 10), u16::MAX);
        assert_eq!(max_diff_scroll(70_000, 20), u16::MAX);
        assert_eq!(max_diff_scroll(0, 0), 0);
        assert_eq!(max_diff_scroll(0, 40), 0);
    }
}
