//
// src/ui/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Orchestrazione generale del rendering: layout dei pannelli e dispatch
//! verso `ui::panels`. Legge il `Model` e disegna soltanto — non chiama mai
//! nulla in `git/`.

pub(crate) mod panels;
pub(crate) mod text;
pub(crate) mod theme;
pub(crate) mod widgets;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::Line,
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::{Model, Panel};
use widgets::window::Window;

/// Sotto questa larghezza si mostra un solo pannello (quello con il focus).
const NARROW: u16 = 80;

/// Dimensioni minime sotto le quali si mostra un messaggio invece della UI.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 6;

pub(crate) fn view(f: &mut Frame, model: &mut Model) {
    let area = f.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(f, area);
        return;
    }

    let [menu_area, body, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    let panel_layout = layout_panels(body, model.focus, area.width);

    for (p, r) in &panel_layout {
        draw_panel(f, model, *p, *r);
    }

    panels::menubar::render_menubar(f, menu_area, model.menu());
    panels::statusbar::render_statusbar(f, status_area, panels::statusbar::hints(model));
    if let Some(m) = model.menu() {
        panels::menubar::render_dropdown(f, menu_area, m);
    }
    if let Some(d) = model.dialog() {
        panels::dialog::render_dialog(f, d);
    }
}

fn draw_too_small(f: &mut Frame, area: Rect) {
    f.render_widget(Clear, area);
    let lines = [
        Line::raw("Terminal too small"),
        Line::raw(format!("Resize to at least {MIN_WIDTH}x{MIN_HEIGHT}")),
    ];
    f.render_widget(
        Paragraph::new(lines.to_vec())
            .style(theme::window())
            .alignment(Alignment::Center),
        area,
    );
}

fn layout_panels(body: Rect, focus: Panel, term_width: u16) -> Vec<(Panel, Rect)> {
    if term_width < NARROW {
        return vec![(focus, body)];
    }
    let [left, right] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(body);
    let [hist, bottom] =
        Layout::vertical([Constraint::Percentage(45), Constraint::Min(0)]).areas(right);
    let [chg, diff] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Min(0)]).areas(bottom);
    vec![
        (Panel::Branches, left),
        (Panel::History, hist),
        (Panel::Changes, chg),
        (Panel::Diff, diff),
    ]
}

fn draw_panel(f: &mut Frame, model: &mut Model, panel: Panel, area: Rect) {
    let focused = model.focus == panel;
    f.render_widget(Window::new(panel.title(), focused), area);
    let inner = Window::inner(area);
    let sel = theme::selection(focused);

    match panel {
        Panel::Branches => panels::branches::render(f, model, inner, sel),
        Panel::History => panels::history::render(f, model, inner, sel),
        Panel::Changes => panels::changes::render(f, model, inner, sel),
        Panel::Diff => panels::diff::render(f, model, inner),
    }
}

/// Renderizza `model` su un backend di `w` x `h` colonne e restituisce il
/// contenuto testuale del frame, riga per riga. Condiviso dai test di
/// rendering in `ui` e nei suoi sottomoduli.
#[cfg(test)]
pub(crate) fn render(w: u16, h: u16, model: &mut Model) -> String {
    use ratatui::{backend::TestBackend, Terminal};

    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view(f, model)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model_for_tests;

    #[test]
    fn renders_80x24() {
        let out = render(80, 24, &mut model_for_tests());
        println!("{out}");
        assert!(out.contains("History"));
    }

    #[test]
    fn renders_narrow_and_tiny_without_panic() {
        render(60, 20, &mut model_for_tests());
        render(20, 5, &mut model_for_tests());
        render(3, 2, &mut model_for_tests());
    }

    #[test]
    fn below_minimum_size_shows_message_instead_of_garbled_panels() {
        let out = render(30, 4, &mut model_for_tests());
        println!("{out}");
        assert!(out.contains("too small"));
    }

    #[test]
    fn narrow_terminal_shows_only_the_focused_panel() {
        for focus in [Panel::Branches, Panel::History, Panel::Changes, Panel::Diff] {
            let body = Rect::new(0, 0, NARROW - 1, 24);
            let panel_layout = layout_panels(body, focus, NARROW - 1);
            assert_eq!(panel_layout, vec![(focus, body)]);
        }
    }

    #[test]
    fn wide_terminal_shows_all_panels() {
        let body = Rect::new(0, 0, NARROW, 24);
        let panel_layout = layout_panels(body, Panel::History, NARROW);
        let shown: Vec<Panel> = panel_layout.iter().map(|(p, _)| *p).collect();
        assert_eq!(
            shown,
            vec![Panel::Branches, Panel::History, Panel::Changes, Panel::Diff]
        );
    }
}
