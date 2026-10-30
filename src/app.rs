use std::io;

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    widgets::ListState,
    DefaultTerminal,
};

use crate::{mock, ui};

#[derive(Clone, Copy, PartialEq, Eq)]
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
    pub history: ListState,
    pub changes: ListState,
    pub diff_scroll: u16,
    pub files: Vec<mock::FileEntry>,
    pub menu: Option<MenuState>,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            focus: Panel::History,
            branches: ListState::default().with_selected(Some(0)),
            history: ListState::default().with_selected(Some(0)),
            changes: ListState::default().with_selected(Some(0)),
            diff_scroll: 0,
            files: mock::files(),
            menu: None,
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
        // Alt+X chiude sempre; Alt+<lettera> apre il menu corrispondente.
        if key.modifiers.contains(KeyModifiers::ALT) {
            if let KeyCode::Char(c) = key.code {
                if c.eq_ignore_ascii_case(&'x') {
                    self.running = false;
                } else if let Some(i) = MENUS
                    .iter()
                    .position(|(n, _)| n.starts_with(c.to_ascii_uppercase()))
                {
                    self.menu = Some(MenuState { menu: i, item: 0 });
                }
            }
            return;
        }
        if self.menu.is_some() {
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
                // Per ora l'unica voce attiva è File > Quit.
                if MENUS[m.menu].1[m.item] == "Quit" {
                    self.running = false;
                }
                self.menu = None;
                return;
            }
            _ => {}
        }
        self.menu = Some(m);
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
            Panel::Branches => step_list(&mut self.branches, mock::BRANCHES.len(), delta),
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
