//
// src/input/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Traduce i `KeyEvent` di crossterm in `Msg`, incluse le scorciatoie
//! globali (Ctrl+C, Alt+X, Alt+<lettera>). *Come* un `Msg` debba essere
//! interpretato in base allo stato corrente (dialog aperto, menu aperto, o
//! nessuno dei due) spetta a `app::update`: qui si traduce solo il tasto
//! premuto, senza leggere il `Model`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::msg::Msg;

/// Traduce un tasto premuto in un `Msg`, oppure `None` se il tasto non ha
/// alcun significato per l'app.
pub(crate) fn key_to_msg(key: KeyEvent) -> Option<Msg> {
    // Ctrl+C esce sempre: la raw mode disabilita la gestione standard di
    // SIGINT del terminale, quindi senza questo l'unica via di uscita
    // sarebbe Esc/Alt-X.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Msg::Quit);
    }

    // Alt+X chiude sempre; Alt+<lettera> è una richiesta di apertura menu
    // (è `update` a ignorarla se c'è un dialog modale aperto). I tasti che
    // combinano Alt con Ctrl (AltGr, su molte tastiere non-US) non sono
    // scorciatoie di menu: cadono nel ramo sotto, altrimenti i caratteri
    // speciali digitati in un campo di testo andrebbero persi.
    if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'x') => Some(Msg::Quit),
            KeyCode::Char(c) => Some(Msg::OpenMenuByHotkey(c)),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Esc => Some(Msg::Esc),
        KeyCode::Enter => Some(Msg::Enter),
        KeyCode::Tab => Some(Msg::Tab),
        KeyCode::BackTab => Some(Msg::BackTab),
        KeyCode::Left => Some(Msg::Left),
        KeyCode::Right => Some(Msg::Right),
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::Backspace => Some(Msg::Backspace),
        KeyCode::F(10) => Some(Msg::F10),
        KeyCode::Char(c) => {
            let is_plain_ctrl = key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT);
            Some(Msg::Char { c, is_plain_ctrl })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn ctrl_c_always_translates_to_quit() {
        assert!(matches!(
            key_to_msg(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Msg::Quit)
        ));
    }

    #[test]
    fn alt_x_translates_to_quit_case_insensitively() {
        assert!(matches!(
            key_to_msg(key(KeyCode::Char('x'), KeyModifiers::ALT)),
            Some(Msg::Quit)
        ));
        assert!(matches!(
            key_to_msg(key(KeyCode::Char('X'), KeyModifiers::ALT)),
            Some(Msg::Quit)
        ));
    }

    #[test]
    fn alt_letter_translates_to_open_menu_by_hotkey() {
        assert!(matches!(
            key_to_msg(key(KeyCode::Char('f'), KeyModifiers::ALT)),
            Some(Msg::OpenMenuByHotkey('f'))
        ));
    }

    #[test]
    fn altgr_like_combo_is_not_a_menu_shortcut_and_stays_plain_text() {
        let msg = key_to_msg(key(
            KeyCode::Char('e'),
            KeyModifiers::ALT | KeyModifiers::CONTROL,
        ));
        assert!(matches!(
            msg,
            Some(Msg::Char {
                c: 'e',
                is_plain_ctrl: false
            })
        ));
    }

    #[test]
    fn plain_ctrl_letter_is_flagged_as_such() {
        let msg = key_to_msg(key(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert!(matches!(
            msg,
            Some(Msg::Char {
                c: 'z',
                is_plain_ctrl: true
            })
        ));
    }

    #[test]
    fn navigation_keys_translate_to_their_semantic_msg() {
        assert!(matches!(
            key_to_msg(key(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Msg::Esc)
        ));
        assert!(matches!(
            key_to_msg(key(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Msg::Enter)
        ));
        assert!(matches!(
            key_to_msg(key(KeyCode::F(10), KeyModifiers::NONE)),
            Some(Msg::F10)
        ));
        assert!(key_to_msg(key(KeyCode::F(2), KeyModifiers::NONE)).is_none());
    }
}
