//
// src/dialog.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Stato e input dei dialog modali (nessun rendering qui, vedi
//! `ui::panels::dialog`). Il dialog è un overlay del `Model` (vedi
//! `app::Model::dialog`): riceve i `Msg` prima del menu e dei pannelli
//! principali.

use crate::msg::Msg;

const MAX_INPUT: usize = 64;

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

    pub(crate) fn confirm(
        title: impl Into<String>,
        text: impl Into<String>,
        action: Action,
    ) -> Self {
        Self::new(Kind::Confirm, title.into(), text.into(), action)
    }

    pub(crate) fn input(
        title: impl Into<String>,
        prompt: impl Into<String>,
        action: Action,
    ) -> Self {
        Self::new(Kind::Input, title.into(), prompt.into(), action)
    }

    pub(crate) fn buttons(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::Message => &["OK"],
            Kind::Confirm => &["Yes", "No"],
            Kind::Input => &["OK", "Cancel"],
        }
    }

    /// Le scorciatoie da mostrare mentre il dialog è aperto. Il tipo è
    /// scritto per esteso invece che dietro un alias locale: un alias
    /// `Hints` qui sarebbe un secondo tipo con lo stesso nome di quello
    /// della barra di stato, e questo modulo non può importare quello
    /// (è stato dell'app, non rendering).
    pub(crate) fn hints(&self) -> &'static [(&'static str, &'static str)] {
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
    pub(crate) fn on_msg(&mut self, msg: Msg) -> Option<bool> {
        let n = self.buttons().len();
        match msg {
            Msg::Esc => return Some(false),
            Msg::Enter => return Some(self.button == 0),
            Msg::Tab | Msg::Right => self.button = (self.button + 1) % n,
            Msg::BackTab | Msg::Left => self.button = (self.button + n - 1) % n,
            Msg::Backspace if self.kind == Kind::Input => {
                self.input.pop();
            }
            // Esclude i veri shortcut Ctrl+lettera, ma non AltGr (riportato da
            // molte piattaforme come Ctrl+Alt) che deve restare testo digitabile.
            Msg::Char { c, is_plain_ctrl } if !is_plain_ctrl => match self.kind {
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
            },
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn char_msg(c: char) -> Msg {
        Msg::Char {
            c,
            is_plain_ctrl: false,
        }
    }

    fn plain_ctrl_msg(c: char) -> Msg {
        Msg::Char {
            c,
            is_plain_ctrl: true,
        }
    }

    #[test]
    fn message_closes_with_enter_or_esc() {
        assert_eq!(Dialog::info("t", "x").on_msg(Msg::Enter), Some(true));
        assert_eq!(Dialog::info("t", "x").on_msg(Msg::Esc), Some(false));
    }

    #[test]
    fn confirm_buttons_and_hotkeys() {
        let mut d = Dialog::confirm("t", "x", Action::Info);
        assert_eq!(d.on_msg(Msg::Right), None);
        assert_eq!(d.button, 1);
        assert_eq!(d.on_msg(Msg::Enter), Some(false));
        assert_eq!(d.on_msg(Msg::Left), None);
        assert_eq!(d.on_msg(Msg::Enter), Some(true));
        assert_eq!(d.on_msg(char_msg('N')), Some(false));
        assert_eq!(d.on_msg(char_msg('y')), Some(true));
    }

    #[test]
    fn input_collects_text_and_ignores_controls() {
        let mut d = Dialog::input("t", "Name:", Action::NewBranch);
        for c in "ab".chars() {
            assert_eq!(d.on_msg(char_msg(c)), None);
        }
        d.on_msg(plain_ctrl_msg('z'));
        assert_eq!(d.input, "ab");
        d.on_msg(Msg::Backspace);
        assert_eq!(d.input, "a");
        d.on_msg(Msg::Tab);
        assert_eq!(d.on_msg(Msg::Enter), Some(false));
    }

    #[test]
    fn input_accepts_altgr_like_combos_but_not_plain_control() {
        let mut d = Dialog::input("t", "Name:", Action::NewBranch);
        // AltGr è spesso riportato come Ctrl+Alt: deve restare testo.
        d.on_msg(char_msg('e'));
        assert_eq!(d.input, "e");
        // Un vero Ctrl+lettera resta uno shortcut, non va inserito.
        d.on_msg(plain_ctrl_msg('z'));
        assert_eq!(d.input, "e");
    }

    #[test]
    fn input_is_length_limited() {
        let mut d = Dialog::input("t", "Name:", Action::NewBranch);
        for _ in 0..(MAX_INPUT + 10) {
            d.on_msg(char_msg('a'));
        }
        assert_eq!(d.input.chars().count(), MAX_INPUT);
    }
}
