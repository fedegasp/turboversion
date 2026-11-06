//
// src/ui/panels/statusbar.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Barra di stato: disegna le scorciatoie attive, scelte da
//! `ui::hints::active` in base all'overlay in cima e al pannello a
//! fuoco.

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::ui::hints::Hints;
use crate::ui::theme;

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
    use crate::app::{model_for_tests, update, MenuState, Overlay, Panel};
    use crate::msg::Msg;

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

    #[test]
    fn git_unavailable_is_still_said_after_the_dialog_is_dismissed() {
        let mut model = model_for_tests();
        assert!(!crate::ui::render(80, 24, &mut model).contains("Git unavailable"));

        update(&mut model, Msg::WorkerLost);
        // Il dialog d'annuncio si chiude; la barra di stato è ciò che
        // resta a dire che le operazioni git non si fanno più.
        model.overlays.pop();
        assert!(crate::ui::render(80, 24, &mut model).contains("Git unavailable"));
    }
}
