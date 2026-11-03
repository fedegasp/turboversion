//
// src/app.rs
// TurboVersion
//
// 2026-10-30 Federico Gasperini
//

use std::io;

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    widgets::ListState,
    DefaultTerminal,
};

use crate::{
    dialog::{Action, Dialog},
    mock, ui,
};

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

pub(crate) struct App {
    pub(crate) running: bool,
    pub(crate) focus: Panel,
    pub(crate) branches: ListState,
    pub(crate) branch_list: Vec<mock::Branch>,
    pub(crate) history: ListState,
    pub(crate) commits: Vec<mock::Commit>,
    pub(crate) changes: ListState,
    pub(crate) diff_scroll: u16,
    pub(crate) diff: Vec<&'static str>,
    pub(crate) files: Vec<mock::FileEntry>,
    pub(crate) menu: Option<MenuState>,
    pub(crate) dialog: Option<Dialog>,
}

impl App {
    pub(crate) fn new() -> Self {
        Self {
            running: true,
            focus: Panel::History,
            branches: ListState::default().with_selected(Some(0)),
            branch_list: mock::branches(),
            history: ListState::default().with_selected(Some(0)),
            commits: mock::commits(),
            changes: ListState::default().with_selected(Some(0)),
            diff_scroll: 0,
            diff: mock::diff(),
            files: mock::files(),
            menu: None,
            dialog: None,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub(crate) fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while self.running {
            terminal.draw(|f| ui::draw(f, self))?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.on_key(key);
                }
            }
            // Gli eventi Resize fanno semplicemente ridisegnare al giro dopo.
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) {
        // Ctrl+C esce sempre, indipendentemente da dialog/menu aperti: la
        // raw mode disabilita la gestione standard di SIGINT del terminale,
        // quindi senza questo l'unica via di uscita sarebbe Esc/Alt-X.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.running = false;
            return;
        }
        // Alt+X chiude sempre; Alt+<lettera> apre il menu corrispondente
        // (tranne con un dialog modale aperto). I tasti che combinano Alt
        // con Ctrl (AltGr, su molte tastiere non-US) non sono scorciatoie
        // di menu: vengono inoltrati alla gestione normale, altrimenti i
        // caratteri speciali digitati in un campo di testo andrebbero persi.
        if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            if let KeyCode::Char(c) = key.code {
                if c.eq_ignore_ascii_case(&'x') {
                    self.running = false;
                } else if self.dialog.is_some() {
                    // modale: ignora
                } else if let Some(i) = MENUS
                    .iter()
                    .position(|(n, _)| n.starts_with(c.to_ascii_uppercase()))
                {
                    self.menu = Some(MenuState { menu: i, item: 0 });
                }
            }
            return;
        }
        if self.dialog.is_some() {
            self.on_dialog_key(key);
        } else if self.menu.is_some() {
            self.on_menu_key(key);
        } else {
            self.on_main_key(key);
        }
    }

    fn on_menu_key(&mut self, key: KeyEvent) {
        let Some(mut m) = self.menu else { return };
        let n_menus = MENUS.len();
        let n_items = MENUS[m.menu].1.len();
        match key.code {
            KeyCode::Esc | KeyCode::F(10) => {
                self.menu = None;
                return;
            }
            KeyCode::Left => {
                m.menu = (m.menu + n_menus - 1) % n_menus;
                m.item = 0;
            }
            KeyCode::Right => {
                m.menu = (m.menu + 1) % n_menus;
                m.item = 0;
            }
            KeyCode::Up => m.item = (m.item + n_items - 1) % n_items,
            KeyCode::Down => m.item = (m.item + 1) % n_items,
            KeyCode::Enter => {
                self.menu = None;
                let (label, action) = MENUS[m.menu].1[m.item];
                self.activate(label, action);
                return;
            }
            _ => {}
        }
        self.menu = Some(m);
    }

    fn on_dialog_key(&mut self, key: KeyEvent) {
        let Some(accepted) = self.dialog.as_mut().and_then(|d| d.on_key(key)) else {
            return;
        };
        if let Some(d) = self.dialog.take() {
            if accepted {
                self.apply(d);
            }
        }
    }

    fn activate(&mut self, label: &str, action: MenuAction) {
        match action {
            MenuAction::Quit => self.running = false,
            MenuAction::About => {
                self.dialog = Some(Dialog::info(
                    "About",
                    "TurboVersion\nTurbo Vision meets version control.",
                ))
            }
            MenuAction::NewBranch => {
                self.dialog = Some(Dialog::input(
                    "New branch",
                    "Branch name:",
                    Action::NewBranch,
                ))
            }
            MenuAction::DeleteBranch => self.ask_delete_branch(),
            MenuAction::OpenRepository
            | MenuAction::CloneRepository
            | MenuAction::Checkout
            | MenuAction::Commit
            | MenuAction::Amend
            | MenuAction::Stash
            | MenuAction::ZoomPanel
            | MenuAction::Refresh => {
                self.dialog = Some(Dialog::info(
                    "Not implemented",
                    format!("\"{label}\" is not available yet."),
                ))
            }
        }
    }

    fn ask_delete_branch(&mut self) {
        let Some(b) = self
            .branches
            .selected()
            .and_then(|i| self.branch_list.get(i))
        else {
            self.dialog = Some(Dialog::info("Delete branch", "No branch selected."));
            return;
        };
        let dialog = if b.current {
            Dialog::info(
                "Delete branch",
                format!("Cannot delete the current branch '{}'.", b.name),
            )
        } else if b.kind != mock::BranchKind::Local {
            Dialog::info(
                "Delete branch",
                format!("'{}' is not a local branch and cannot be deleted here.", b.name),
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
        self.dialog = Some(dialog);
    }

    fn apply(&mut self, d: Dialog) {
        match d.action {
            Action::Info => {}
            Action::NewBranch => self.create_branch(d.input.trim()),
            Action::DeleteBranch(name) => self.delete_branch(&name),
        }
    }

    fn create_branch(&mut self, name: &str) {
        if let Some(text) = validate_branch_name(name, &self.branch_list) {
            // Riapre il dialog con il testo già digitato, invece di un
            // messaggio d'errore che lo scarterebbe e costringerebbe a
            // ridigitarlo da capo.
            let mut d = Dialog::input("New branch", format!("Branch name:\n{text}"), Action::NewBranch);
            d.input = name.to_string();
            self.dialog = Some(d);
            return;
        }
        self.branch_list.push(mock::Branch {
            name: name.to_string(),
            kind: mock::BranchKind::Local,
            current: false,
        });
        self.branches.select(Some(self.branch_list.len() - 1));
    }

    fn delete_branch(&mut self, name: &str) {
        self.branch_list.retain(|b| b.name != name);
        let last = self.branch_list.len().checked_sub(1);
        let sel = self.branches.selected().zip(last).map(|(s, l)| s.min(l));
        self.branches.select(sel);
    }

    fn on_main_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::F(10) => self.menu = Some(MenuState { menu: 0, item: 0 }),
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::Char(' ') if self.focus == Panel::Changes => {
                if let Some(i) = self.changes.selected() {
                    self.files[i].staged = !self.files[i].staged;
                }
            }
            _ => {}
        }
    }

    fn step(&mut self, delta: i32) {
        match self.focus {
            Panel::Branches => step_list(&mut self.branches, self.branch_list.len(), delta),
            Panel::History => {
                step_list(&mut self.history, self.commits.len(), delta);
                // Una nuova selezione mostra un commit diverso: lo scroll
                // del diff precedente non ha più senso.
                self.diff_scroll = 0;
            }
            Panel::Changes => step_list(&mut self.changes, self.files.len(), delta),
            Panel::Diff => {
                let max = self.diff.len().saturating_sub(1) as i32;
                self.diff_scroll = (self.diff_scroll as i32 + delta).clamp(0, max) as u16;
            }
        }
    }
}

/// Valida un nome di ramo in modo vicino a `git check-ref-format`: niente
/// spazi/controlli, niente `-` iniziale (eviterebbe che un domani, passato a
/// `git` come argomento, venga scambiato per un'opzione), niente sequenze o
/// terminazioni che git stesso rifiuta.
fn validate_branch_name(name: &str, existing: &[mock::Branch]) -> Option<String> {
    const INVALID_CHARS: &[char] = &['~', '^', ':', '?', '*', '[', '\\', ' '];
    if name.is_empty() {
        return Some("Branch name cannot be empty.".to_string());
    }
    if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("Branch names cannot contain spaces or control characters.".to_string());
    }
    if name.starts_with('-') {
        return Some("Branch names cannot start with '-'.".to_string());
    }
    if name.ends_with('/') || name.ends_with('.') || name.ends_with(".lock") {
        return Some("Branch names cannot end with '/', '.' or '.lock'.".to_string());
    }
    if name.contains("..") || name.contains("@{") || name.chars().any(|c| INVALID_CHARS.contains(&c))
    {
        return Some("Branch names cannot contain '..', '@{', or any of ~ ^ : ? * [ \\.".to_string());
    }
    if existing.iter().any(|b| b.name == name) {
        return Some(format!("Branch '{name}' already exists."));
    }
    None
}

fn step_list(state: &mut ListState, len: usize, delta: i32) {
    if len == 0 {
        return;
    }
    let cur = state.selected().unwrap_or(0) as i32;
    state.select(Some((cur + delta).clamp(0, len as i32 - 1) as usize));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn type_text(app: &mut App, text: &str) {
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

    fn run_menu_action(app: &mut App, action: MenuAction) {
        let (menu, item) = menu_position(action);
        app.menu = Some(MenuState { menu, item });
        press(app, KeyCode::Enter);
    }

    #[test]
    fn about_opens_a_dialog_and_esc_closes_it() {
        let mut app = App::new();
        run_menu_action(&mut app, MenuAction::About);
        assert!(app.dialog.is_some());
        assert!(app.menu.is_none());
        press(&mut app, KeyCode::Esc);
        assert!(app.dialog.is_none());
    }

    #[test]
    fn dialog_is_modal() {
        let mut app = App::new();
        app.dialog = Some(Dialog::input("t", "x", Action::NewBranch));
        let focus = app.focus;

        type_text(&mut app, "q"); // finisce nel campo, non esce
        press(&mut app, KeyCode::F(10)); // non apre il menu
        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT));

        assert!(app.running);
        assert!(app.menu.is_none());
        assert_eq!(app.focus, focus);
        assert_eq!(app.dialog.as_ref().unwrap().input, "q");
    }

    #[test]
    fn alt_x_quits_even_with_dialog_open() {
        let mut app = App::new();
        run_menu_action(&mut app, MenuAction::About);
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT));
        assert!(!app.running);
    }

    #[test]
    fn ctrl_c_quits_even_with_dialog_or_menu_open() {
        let mut app = App::new();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.running);

        let mut app = App::new();
        run_menu_action(&mut app, MenuAction::About);
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.running);
    }

    #[test]
    fn altgr_like_combo_is_not_swallowed_as_a_menu_shortcut() {
        // Alt+Ctrl è come molte piattaforme riportano AltGr: non deve aprire
        // un menu, e il carattere deve comunque raggiungere il campo di testo.
        let mut app = App::new();
        app.dialog = Some(Dialog::input("t", "x", Action::NewBranch));
        app.on_key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::ALT | KeyModifiers::CONTROL,
        ));
        assert!(app.menu.is_none());
        assert_eq!(app.dialog.as_ref().unwrap().input, "e");
    }

    #[test]
    fn new_branch_is_created_and_selected() {
        let mut app = App::new();
        let n = app.branch_list.len();
        run_menu_action(&mut app, MenuAction::NewBranch);
        type_text(&mut app, "  feature/x ");
        press(&mut app, KeyCode::Enter);

        assert!(app.dialog.is_none());
        assert_eq!(app.branch_list.len(), n + 1);
        assert_eq!(app.branch_list[n].name, "feature/x");
        assert_eq!(app.branch_list[n].kind, mock::BranchKind::Local);
        assert_eq!(app.branches.selected(), Some(n));
    }

    #[test]
    fn new_branch_cancel_and_validation_keeps_typed_text_for_correction() {
        let mut app = App::new();
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

            assert!(app.dialog.is_some(), "errore atteso per {bad:?}");
            let d = app.dialog.as_ref().unwrap();
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
            assert!(app.dialog.is_none());
            assert_eq!(app.branch_list.len(), n);
        }
    }

    #[test]
    fn current_branch_cannot_be_deleted() {
        let mut app = App::new();
        let n = app.branch_list.len();
        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(
            app.dialog.as_ref().unwrap().kind,
            crate::dialog::Kind::Message
        );
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.branch_list.len(), n);
    }

    #[test]
    fn remote_branches_and_tags_cannot_be_deleted() {
        let mut app = App::new();
        let n = app.branch_list.len();
        for name in ["origin/main", "v0.1.0"] {
            let i = app.branch_list.iter().position(|b| b.name == name).unwrap();
            app.branches.select(Some(i));
            run_menu_action(&mut app, MenuAction::DeleteBranch);
            assert_eq!(
                app.dialog.as_ref().unwrap().kind,
                crate::dialog::Kind::Message,
                "{name} non dovrebbe essere cancellabile"
            );
            press(&mut app, KeyCode::Enter);
            assert_eq!(app.branch_list.len(), n);
        }
    }

    #[test]
    fn deleting_with_nothing_selected_shows_a_message_instead_of_doing_nothing_silently() {
        let mut app = App::new();
        app.branches.select(None);
        run_menu_action(&mut app, MenuAction::DeleteBranch);
        assert_eq!(
            app.dialog.as_ref().unwrap().kind,
            crate::dialog::Kind::Message
        );
        assert!(app.dialog.as_ref().unwrap().text.contains("No branch selected"));
    }

    #[test]
    fn delete_branch_asks_confirmation_defaulting_to_no() {
        let mut app = App::new();
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
        let mut app = App::new();
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
        let mut app = App::new();
        run_menu_action(&mut app, MenuAction::OpenRepository);
        assert!(app.dialog.as_ref().unwrap().text.contains("not available"));
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
        let mut app = App::new();
        app.focus = Panel::Diff;
        app.diff_scroll = 3;
        app.focus = Panel::History;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.diff_scroll, 0);
    }
}
