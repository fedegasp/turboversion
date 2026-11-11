//
// src/app/tests/diff.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use super::super::model::max_diff_scroll;
use super::*;
use crate::ui;

#[test]
fn diff_scroll_resets_when_history_selection_changes() {
    let mut app = TestApp::new();
    app.focus = Panel::Diff;
    app.diff_scroll = 3;
    app.focus = Panel::History;
    press(&mut app, KeyCode::Down);
    assert_eq!(app.diff_scroll, 0);
}

/// Un `TestApp` con il focus sul Diff, `lines` righe di diff e un
/// pannello alto `viewport` righe, annunciato come lo annuncerebbe un
/// frame appena disegnato (`viewport` a 0 vuol dire "mai disegnato").
fn app_showing_diff(lines: usize, viewport: u16) -> TestApp {
    let mut app = TestApp::new();
    app.focus = Panel::Diff;
    app.show_diff_for_tests((0..lines).map(|i| format!("line {i}")).collect());
    app.send(Msg::DiffViewport(viewport));
    app
}

#[test]
fn diff_scroll_stops_with_the_last_line_at_the_bottom_of_the_panel() {
    let mut app = app_showing_diff(10, 4);
    for _ in 0..20 {
        press(&mut app, KeyCode::Down);
    }
    // Righe 6..9 visibili: l'ultima è in fondo al pannello, non in cima.
    assert_eq!(app.diff_scroll, 6);
}

#[test]
fn a_diff_shorter_than_the_panel_does_not_scroll() {
    let mut app = app_showing_diff(3, 10);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.diff_scroll, 0);
}

/// Ingrandire il terminale non deve lasciare righe vuote sotto
/// l'ultima riga del diff: il limite si riapplica quando arriva la
/// nuova altezza, non al tasto successivo.
#[test]
fn a_taller_panel_pulls_the_scroll_back_to_the_bottom() {
    let mut app = app_showing_diff(10, 4);
    for _ in 0..20 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.diff_scroll, 6);

    app.send(Msg::DiffViewport(19));
    assert_eq!(app.diff_viewport, 19);
    assert_eq!(app.diff_scroll, 0, "il diff ci sta tutto");

    app.send(Msg::DiffViewport(7));
    assert_eq!(app.diff_scroll, 0, "rimpicciolire non muove lo scroll");
}

/// L'anello completo: il pannello viene disegnato, la sua altezza
/// rientra nel `Model` e lo scorrimento si ferma dove il pannello
/// finisce. Gli altri test di scorrimento fissano l'altezza a mano e
/// non passano da `ui`.
#[test]
fn scrolling_a_drawn_panel_stops_at_its_real_bottom() {
    let mut app = app_showing_diff(100, 0);
    ui::render(100, 30, &mut app.model);
    let visible = app.diff_viewport;
    assert!(visible > 0, "il pannello è stato disegnato");

    for _ in 0..200 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.diff_scroll, 100 - visible);

    // L'ultima riga del diff è davvero l'ultima riga del pannello.
    let out = ui::render(100, 30, &mut app.model);
    assert!(out.contains("line 99"), "{out}");
    assert!(!out.contains("line 85"), "{out}");
}

#[test]
fn an_empty_diff_or_an_unknown_height_scrolls_without_panicking() {
    let mut app = app_showing_diff(0, 0);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.diff_scroll, 0);

    app.send(Msg::DiffViewport(10));
    press(&mut app, KeyCode::Down);
    assert_eq!(app.diff_scroll, 0);

    // Altezza ancora sconosciuta su un diff non vuoto: lo scorrimento
    // non si blocca, e resta comunque dentro il diff.
    let mut app = app_showing_diff(3, 0);
    for _ in 0..10 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.diff_scroll, 2);
}

/// Un diff più lungo di quanto `u16` possa contare non fa saltare lo
/// scorrimento in cima né panicare: il limite si ferma a `u16::MAX`.
#[test]
fn a_diff_longer_than_u16_clamps_instead_of_wrapping_around() {
    assert_eq!(max_diff_scroll(usize::MAX, 10), u16::MAX);
    assert_eq!(max_diff_scroll(70_000, 20), u16::MAX);
    assert_eq!(max_diff_scroll(0, 0), 0);
    assert_eq!(max_diff_scroll(0, 40), 0);
}
