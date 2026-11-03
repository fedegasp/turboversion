//
// src/ui/panels/statusbar.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Barra di stato: scorciatoie disponibili, che cambiano in base
//! all'overlay attivo (dialog > menu > pannello principale).

use std::borrow::Cow;

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::{Model, Panel};
use crate::ui::theme;

pub(crate) type Hints = Cow<'static, [(&'static str, &'static str)]>;

/// Scorciatoie del pannello principale; `Space Stage` compare solo quando il
/// focus è sul pannello Changes, dove quel tasto ha davvero effetto.
fn main_hints(focus: Panel) -> Vec<(&'static str, &'static str)> {
    let mut hints = vec![("Tab", "Panel"), ("↑↓", "Move")];
    if focus == Panel::Changes {
        hints.push(("Space", "Stage"));
    }
    hints.push(("F10", "Menu"));
    hints.push(("Alt-X", "Quit"));
    hints
}

pub(crate) fn hints(model: &Model) -> Hints {
    match (model.dialog(), model.menu()) {
        (Some(d), _) => Cow::Borrowed(d.hints()),
        (None, Some(_)) => Cow::Borrowed(super::menubar::MENU_KEYS),
        (None, None) => Cow::Owned(main_hints(model.focus)),
    }
}

pub(crate) fn render_statusbar(f: &mut Frame, area: Rect, keys: Hints) {
    let mut spans = Vec::new();
    for (k, label) in keys.iter() {
        spans.push(Span::styled(format!(" {k}"), theme::menubar_hotkey()));
        spans.push(Span::styled(format!(" {label} "), theme::menubar()));
    }
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::menubar()),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{model_for_tests, MenuState, Overlay};

    #[test]
    fn statusbar_hints_change_when_menu_opens() {
        let mut model = model_for_tests();
        let closed = crate::ui::render(80, 24, &mut model);
        assert!(closed.contains("Menu"));
        assert!(!closed.contains("Close"));

        model
            .overlays
            .push(Overlay::Menu(MenuState { menu: 0, item: 0 }));
        let open = crate::ui::render(80, 24, &mut model);
        assert!(open.contains("Close"));
    }

    #[test]
    fn space_stage_hint_only_shows_when_changes_panel_is_focused() {
        let mut model = model_for_tests();
        model.focus = Panel::History;
        assert!(!crate::ui::render(80, 24, &mut model).contains("Stage"));

        model.focus = Panel::Changes;
        assert!(crate::ui::render(80, 24, &mut model).contains("Stage"));
    }
}
