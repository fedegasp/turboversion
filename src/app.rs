//
// src/app.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! `Model` (stato dell'applicazione) e `update` (`Model`, `Msg` -> `Vec<Cmd>`).
//! `update` è puro: nessun I/O, nessun rendering, nessun `unwrap`. Le
//! operazioni verso git passano da un `Cmd` restituito qui, eseguito da
//! `effects::run` e il cui esito torna come un nuovo `Msg` (vedi `run`).

use std::io;

use ratatui::{
    crossterm::event::{self, Event, KeyEventKind},
    widgets::ListState,
    DefaultTerminal,
};

use crate::dialog::{Action, Dialog};
use crate::git::domain::{Branch, BranchKind, Commit, FileEntry};
use crate::git::repo::{GitError, GitRepo};
use crate::input;
use crate::msg::{Cmd, Msg};
use crate::{effects, ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Panel {
    Branches,
    History,
    Changes,
    Diff,
}

impl Panel {
    const ALL: [Panel; 4] = [Panel::Branches, Panel::History, Panel::Changes, Panel::Diff];

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
/// `Model` (regola 5 di `architettura-tui`): l'overlay in cima riceve per
/// primo gli input (vedi `dispatch_key`). Oggi lo stack non supera mai un
/// elemento, ma la lettura guarda comunque solo la cima.
pub(crate) enum Overlay {
    Menu(MenuState),
    Dialog(Dialog),
}

pub(crate) struct Model {
    pub(crate) running: bool,
    pub(crate) focus: Panel,
    pub(crate) branches: ListState,
    pub(crate) branch_list: Vec<Branch>,
    pub(crate) history: ListState,
    pub(crate) commits: Vec<Commit>,
    pub(crate) changes: ListState,
    pub(crate) diff_scroll: u16,
    pub(crate) diff: Vec<String>,
    pub(crate) files: Vec<FileEntry>,
    pub(crate) overlays: Vec<Overlay>,
}

impl Model {
    pub(crate) fn new() -> Self {
        Self {
            running: true,
            focus: Panel::History,
            branches: ListState::default(),
            branch_list: Vec::new(),
            history: ListState::default(),
            commits: Vec::new(),
            changes: ListState::default(),
            diff_scroll: 0,
            diff: Vec::new(),
            files: Vec::new(),
            overlays: Vec::new(),
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

/// Ciclo principale: disegna, legge un tasto, lo traduce in `Msg` (vedi
/// `input::key_to_msg`), applica `update` e, se richiede effetti, li esegue
/// subito tramite `effects::run` rialimentando il loro `Msg` di esito in
/// `update` (vedi il commento in `effects.rs` sul perché è sincrono).
pub(crate) fn run(
    model: &mut Model,
    repo: &mut dyn GitRepo,
    terminal: &mut DefaultTerminal,
) -> io::Result<()> {
    while model.running {
        terminal.draw(|f| ui::view(f, model))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let Some(msg) = input::key_to_msg(key) {
                    dispatch(model, repo, msg);
                }
            }
        }
        // Gli eventi Resize fanno semplicemente ridisegnare al giro dopo.
    }
    Ok(())
}

/// Applica `msg` e, se `update` ha richiesto effetti, li esegue
/// immediatamente rialimentando il loro esito.
fn dispatch(model: &mut Model, repo: &mut dyn GitRepo, msg: Msg) {
    for cmd in update(model, msg) {
        let result = effects::run(cmd, repo);
        // Un Cmd produce un solo Msg di esito, mai altri Cmd: nessun rischio
        // di ricorsione.
        update(model, result);
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

fn apply_loaded(
    model: &mut Model,
    branches: Vec<Branch>,
    commits: Vec<Commit>,
    files: Vec<FileEntry>,
    diff: Vec<String>,
) {
    model.branch_list = branches;
    model.branches = ListState::default().with_selected(Some(0));
    model.commits = commits;
    model.history = ListState::default().with_selected(Some(0));
    model.files = files;
    model.changes = ListState::default().with_selected(Some(0));
    model.diff = diff;
    model.diff_scroll = 0;
}

fn apply_branch_created(model: &mut Model, name: String, result: Result<Branch, GitError>) {
    match result {
        Ok(branch) => {
            model.branch_list.push(branch);
            model.branches.select(Some(model.branch_list.len() - 1));
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
            model.branch_list.retain(|b| b.name != name);
            let last = model.branch_list.len().checked_sub(1);
            let sel = model.branches.selected().zip(last).map(|(s, l)| s.min(l));
            model.branches.select(sel);
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

fn ask_delete_branch(model: &mut Model) {
    let Some(b) = model
        .branches
        .selected()
        .and_then(|i| model.branch_list.get(i))
    else {
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
    } else if b.kind != BranchKind::Local {
        Dialog::info(
            "Delete branch",
            format!(
                "'{}' is not a local branch and cannot be deleted here.",
                b.name
            ),
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
            if let Some(i) = model.changes.selected() {
                model.files[i].staged = !model.files[i].staged;
            }
        }
        _ => {}
    }
    Vec::new()
}

fn step(model: &mut Model, delta: i32) {
    match model.focus {
        Panel::Branches => step_list(&mut model.branches, model.branch_list.len(), delta),
        Panel::History => {
            step_list(&mut model.history, model.commits.len(), delta);
            // Una nuova selezione mostra un commit diverso: lo scroll
            // del diff precedente non ha più senso.
            model.diff_scroll = 0;
        }
        Panel::Changes => step_list(&mut model.changes, model.files.len(), delta),
        Panel::Diff => {
            let max = model.diff.len().saturating_sub(1) as i32;
            model.diff_scroll = (model.diff_scroll as i32 + delta).clamp(0, max) as u16;
        }
    }
}

fn step_list(state: &mut ListState, len: usize, delta: i32) {
    if len == 0 {
        return;
    }
    let cur = state.selected().unwrap_or(0) as i32;
    state.select(Some((cur + delta).clamp(0, len as i32 - 1) as usize));
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
            dispatch(&mut self.model, &mut self.repo, msg);
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
        let n = app.branch_list.len();
        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, "  feature/x ");
        press(&mut app, KeyCode::Enter);

        assert!(app.dialog().is_none());
        assert_eq!(app.branch_list.len(), n + 1);
        assert_eq!(app.branch_list[n].name, "feature/x");
        assert_eq!(app.branch_list[n].kind, BranchKind::Local);
        assert_eq!(app.branches.selected(), Some(n));
    }

    #[test]
    fn new_branch_cancel_and_validation_keeps_typed_text_for_correction() {
        let mut app = TestApp::new();
        let n = app.branch_list.len();

        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, "abc");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.branch_list.len(), n);

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
            assert_eq!(app.branch_list.len(), n);
        }
    }

    #[test]
    fn current_branch_cannot_be_deleted() {
        let mut app = TestApp::new();
        let n = app.branch_list.len();
        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.branch_list.len(), n);
    }

    #[test]
    fn remote_branches_and_tags_cannot_be_deleted() {
        let mut app = TestApp::new();
        let n = app.branch_list.len();
        for name in ["origin/main", "v0.1.0"] {
            let i = app.branch_list.iter().position(|b| b.name == name).unwrap();
            app.branches.select(Some(i));
            run_menu_action(&mut app, MenuAction::DeleteBranch);
            assert_eq!(
                app.dialog().unwrap().kind,
                crate::dialog::Kind::Message,
                "{name} non dovrebbe essere cancellabile"
            );
            press(&mut app, KeyCode::Enter);
            assert_eq!(app.branch_list.len(), n);
        }
    }

    #[test]
    fn deleting_with_nothing_selected_shows_a_message_instead_of_doing_nothing_silently() {
        let mut app = TestApp::new();
        app.branches.select(None);
        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(app.dialog().unwrap().kind, crate::dialog::Kind::Message);
        assert!(app.dialog().unwrap().text.contains("No branch selected"));
    }

    #[test]
    fn delete_branch_asks_confirmation_defaulting_to_no() {
        let mut app = TestApp::new();
        let n = app.branch_list.len();
        app.branches.select(Some(1)); // develop

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        press(&mut app, KeyCode::Enter); // default: No
        assert_eq!(app.branch_list.len(), n);

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.branch_list.len(), n - 1);
        assert!(app.branch_list.iter().all(|b| b.name != "develop"));
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
        let last = app.branch_list.len() - 1;
        assert_eq!(app.branches.selected(), Some(last));

        run_menu_action(&mut app, MenuAction::DeleteBranch);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.branches.selected(), Some(last - 1));
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
        assert_eq!(Panel::History.next(), Panel::Changes);
        assert_eq!(Panel::Changes.prev(), Panel::History);
    }

    #[test]
    fn step_list_clamps_at_bounds_and_tolerates_empty_lists() {
        let mut state = ListState::default().with_selected(Some(0));
        step_list(&mut state, 3, -1);
        assert_eq!(state.selected(), Some(0));
        step_list(&mut state, 3, 10);
        assert_eq!(state.selected(), Some(2));
        step_list(&mut state, 0, 1); // lista vuota: niente panico, selezione invariata
        assert_eq!(state.selected(), Some(2));
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
}
