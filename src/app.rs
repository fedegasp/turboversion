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
pub enum Panel {
    Branches,
    History,
    Changes,
    Diff,
}

impl Panel {
    const ALL: [Panel; 4] = [Panel::Branches, Panel::History, Panel::Changes, Panel::Diff];

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|p| *p == self)
            .expect("Panel::ALL deve contenere tutte le varianti")
    }
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }
    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
    pub fn title(self) -> &'static str {
        match self {
            Panel::Branches => "Branches",
            Panel::History => "History",
            Panel::Changes => "Changes",
            Panel::Diff => "Diff",
        }
    }
}

pub const MENUS: &[(&str, &[&str])] = &[
    ("File", &["Open repository...", "Clone...", "Quit"]),
    ("Branch", &["New branch...", "Checkout", "Delete"]),
    ("Commit", &["Commit...", "Amend", "Stash"]),
    ("View", &["Zoom panel", "Refresh"]),
    ("Help", &["About"]),
];

#[derive(Clone, Copy)]
pub struct MenuState {
    pub menu: usize,
    pub item: usize,
}

pub struct App {
    pub running: bool,
    pub focus: Panel,
    pub branches: ListState,
    pub branch_list: Vec<mock::Branch>,
    pub history: ListState,
    pub changes: ListState,
    pub diff_scroll: u16,
    pub files: Vec<mock::FileEntry>,
    pub menu: Option<MenuState>,
    pub dialog: Option<Dialog>,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            focus: Panel::History,
            branches: ListState::default().with_selected(Some(0)),
            branch_list: mock::branches(),
            history: ListState::default().with_selected(Some(0)),
            changes: ListState::default().with_selected(Some(0)),
            diff_scroll: 0,
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
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
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
        // Alt+X chiude sempre; Alt+<lettera> apre il menu corrispondente
        // (tranne con un dialog modale aperto).
        if key.modifiers.contains(KeyModifiers::ALT) {
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
                self.activate(MENUS[m.menu].1[m.item]);
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

    fn activate(&mut self, item: &str) {
        match item {
            "Quit" => self.running = false,
            "About" => {
                self.dialog = Some(Dialog::info(
                    "About",
                    "TurboVersion\nTurbo Vision meets version control.",
                ))
            }
            "New branch..." => {
                self.dialog = Some(Dialog::input(
                    "New branch",
                    "Branch name:",
                    Action::NewBranch,
                ))
            }
            "Delete" => self.ask_delete_branch(),
            other => {
                self.dialog = Some(Dialog::info(
                    "Not implemented",
                    format!("\"{other}\" is not available yet."),
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
        let error = if name.is_empty() {
            Some("Branch name cannot be empty.".to_string())
        } else if name.chars().any(char::is_whitespace) {
            Some("Branch names cannot contain spaces.".to_string())
        } else if self.branch_list.iter().any(|b| b.name == name) {
            Some(format!("Branch '{name}' already exists."))
        } else {
            None
        };
        if let Some(text) = error {
            self.dialog = Some(Dialog::info("New branch", text));
            return;
        }
        self.branch_list.push(mock::Branch {
            name: name.to_string(),
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
            Panel::History => step_list(&mut self.history, mock::COMMITS.len(), delta),
            Panel::Changes => step_list(&mut self.changes, self.files.len(), delta),
            Panel::Diff => {
                let max = mock::DIFF.len().saturating_sub(1) as i32;
                self.diff_scroll = (self.diff_scroll as i32 + delta).clamp(0, max) as u16;
            }
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

    fn run_menu_item(app: &mut App, menu: usize, item: usize) {
        app.menu = Some(MenuState { menu, item });
        press(app, KeyCode::Enter);
    }

    #[test]
    fn about_opens_a_dialog_and_esc_closes_it() {
        let mut app = App::new();
        run_menu_item(&mut app, 4, 0);
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
        run_menu_item(&mut app, 4, 0);
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT));
        assert!(!app.running);
    }

    #[test]
    fn new_branch_is_created_and_selected() {
        let mut app = App::new();
        let n = app.branch_list.len();
        run_menu_item(&mut app, 1, 0);
        type_text(&mut app, "  feature/x ");
        press(&mut app, KeyCode::Enter);

        assert!(app.dialog.is_none());
        assert_eq!(app.branch_list.len(), n + 1);
        assert_eq!(app.branch_list[n].name, "feature/x");
        assert_eq!(app.branches.selected(), Some(n));
    }

    #[test]
    fn new_branch_cancel_and_validation() {
        let mut app = App::new();
        let n = app.branch_list.len();

        run_menu_item(&mut app, 1, 0);
        type_text(&mut app, "abc");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.branch_list.len(), n);

        for bad in ["", "two words", "develop"] {
            run_menu_item(&mut app, 1, 0);
            type_text(&mut app, bad);
            press(&mut app, KeyCode::Enter);
            assert!(app.dialog.is_some(), "errore atteso per {bad:?}");
            press(&mut app, KeyCode::Enter);
            assert!(app.dialog.is_none());
            assert_eq!(app.branch_list.len(), n);
        }
    }

    #[test]
    fn current_branch_cannot_be_deleted() {
        let mut app = App::new();
        let n = app.branch_list.len();
        run_menu_item(&mut app, 1, 2);
        assert_eq!(
            app.dialog.as_ref().unwrap().kind,
            crate::dialog::Kind::Message
        );
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.branch_list.len(), n);
    }

    #[test]
    fn delete_branch_asks_confirmation_defaulting_to_no() {
        let mut app = App::new();
        let n = app.branch_list.len();
        app.branches.select(Some(1)); // develop

        run_menu_item(&mut app, 1, 2);
        press(&mut app, KeyCode::Enter); // default: No
        assert_eq!(app.branch_list.len(), n);

        run_menu_item(&mut app, 1, 2);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.branch_list.len(), n - 1);
        assert!(app.branch_list.iter().all(|b| b.name != "develop"));
    }

    #[test]
    fn deleting_last_branch_keeps_selection_in_range() {
        let mut app = App::new();
        let last = app.branch_list.len() - 1;
        app.branches.select(Some(last));
        run_menu_item(&mut app, 1, 2);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.branches.selected(), Some(last - 1));
    }

    #[test]
    fn unimplemented_items_report_it() {
        let mut app = App::new();
        run_menu_item(&mut app, 0, 0);
        assert!(app.dialog.as_ref().unwrap().text.contains("not available"));
    }
}
