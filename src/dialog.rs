//
// src/dialog.rs
// TurboVersion
//
// 2026-10-30 Federico Gasperini
//

//! Stato e input dei dialog modali (nessun rendering qui, vedi `ui.rs`).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const MAX_INPUT: usize = 64;

type Hints = &'static [(&'static str, &'static str)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// Solo informazione: un pulsante OK.
    Message,
    /// Domanda Yes/No.
    Confirm,
    /// Campo di testo con OK/Cancel.
    Input,
}

/// Cosa fa l'app quando il dialog viene accettato.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    Info,
    NewBranch,
    DeleteBranch(String),
}

pub(crate) struct Dialog {
    pub(crate) title: String,
    pub(crate) text: String,
    pub(crate) kind: Kind,
    pub(crate) action: Action,
    pub(crate) input: String,
    /// Indice del pulsante con il focus; 0 è sempre quello che accetta.
    pub(crate) button: usize,
}

impl Dialog {
    fn new(kind: Kind, title: String, text: String, action: Action) -> Self {
        Self {
            title,
            text,
            kind,
            action,
            input: String::new(),
            button: 0,
        }
    }

    pub(crate) fn info(title: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(Kind::Message, title.into(), text.into(), Action::Info)
    }

    pub(crate) fn confirm(title: impl Into<String>, text: impl Into<String>, action: Action) -> Self {
        Self::new(Kind::Confirm, title.into(), text.into(), action)
    }

    pub(crate) fn input(title: impl Into<String>, prompt: impl Into<String>, action: Action) -> Self {
        Self::new(Kind::Input, title.into(), prompt.into(), action)
    }

    pub(crate) fn buttons(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::Message => &["OK"],
            Kind::Confirm => &["Yes", "No"],
            Kind::Input => &["OK", "Cancel"],
        }
    }

    pub(crate) fn hints(&self) -> Hints {
        match self.kind {
            Kind::Message => &[("Enter", "OK"), ("Esc", "Close")],
            Kind::Confirm => &[
                ("←→", "Choose"),
                ("Enter", "Select"),
                ("Y/N", "Yes/No"),
                ("Esc", "Cancel"),
            ],
            Kind::Input => &[
                ("Type", "Text"),
                ("Tab", "Button"),
                ("Enter", "OK"),
                ("Esc", "Cancel"),
            ],
        }
    }

    /// `Some(true)` = accettato, `Some(false)` = annullato, `None` = ancora aperto.
    pub(crate) fn on_key(&mut self, key: KeyEvent) -> Option<bool> {
        let n = self.buttons().len();
        match key.code {
            KeyCode::Esc => return Some(false),
            KeyCode::Enter => return Some(self.button == 0),
            KeyCode::Tab | KeyCode::Right => self.button = (self.button + 1) % n,
            KeyCode::BackTab | KeyCode::Left => self.button = (self.button + n - 1) % n,
            KeyCode::Backspace if self.kind == Kind::Input => {
                self.input.pop();
            }
            // Esclude i veri shortcut Ctrl+lettera, ma non AltGr (riportato da
            // molte piattaforme come Ctrl+Alt) che deve restare testo digitabile.
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                match self.kind {
                    Kind::Input => {
                        if !c.is_control() && self.input.chars().count() < MAX_INPUT {
                            self.input.push(c);
                        }
                    }
                    Kind::Confirm => match c.to_ascii_lowercase() {
                        'y' => return Some(true),
                        'n' => return Some(false),
                        _ => {}
                    },
                    Kind::Message => {}
                }
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn message_closes_with_enter_or_esc() {
        assert_eq!(
            Dialog::info("t", "x").on_key(key(KeyCode::Enter)),
            Some(true)
        );
        assert_eq!(
            Dialog::info("t", "x").on_key(key(KeyCode::Esc)),
            Some(false)
        );
    }

    #[test]
    fn confirm_buttons_and_hotkeys() {
        let mut d = Dialog::confirm("t", "x", Action::Info);
        assert_eq!(d.on_key(key(KeyCode::Right)), None);
        assert_eq!(d.button, 1);
        assert_eq!(d.on_key(key(KeyCode::Enter)), Some(false));
        assert_eq!(d.on_key(key(KeyCode::Left)), None);
        assert_eq!(d.on_key(key(KeyCode::Enter)), Some(true));
        assert_eq!(d.on_key(key(KeyCode::Char('N'))), Some(false));
        assert_eq!(d.on_key(key(KeyCode::Char('y'))), Some(true));
    }

    #[test]
    fn input_collects_text_and_ignores_controls() {
        let mut d = Dialog::input("t", "Name:", Action::NewBranch);
        for c in "ab".chars() {
            assert_eq!(d.on_key(key(KeyCode::Char(c))), None);
        }
        d.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(d.input, "ab");
        d.on_key(key(KeyCode::Backspace));
        assert_eq!(d.input, "a");
        d.on_key(key(KeyCode::Tab));
        assert_eq!(d.on_key(key(KeyCode::Enter)), Some(false));
    }

    #[test]
    fn input_accepts_altgr_like_combos_but_not_plain_control() {
        let mut d = Dialog::input("t", "Name:", Action::NewBranch);
        // AltGr è spesso riportato come Ctrl+Alt: deve restare testo.
        d.on_key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ));
        assert_eq!(d.input, "e");
        // Un vero Ctrl+lettera resta uno shortcut, non va inserito.
        d.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(d.input, "e");
    }

    #[test]
    fn input_is_length_limited() {
        let mut d = Dialog::input("t", "Name:", Action::NewBranch);
        for _ in 0..(MAX_INPUT + 10) {
            d.on_key(key(KeyCode::Char('a')));
        }
        assert_eq!(d.input.chars().count(), MAX_INPUT);
    }
}
