//
// src/app/tests/overlays.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use super::*;
use crate::dialog::{Action, Dialog};

#[test]
fn about_opens_a_dialog_and_esc_closes_it() {
    let mut app = TestApp::new();
    run_menu_action(&mut app, MenuAction::About);
    assert!(app.dialog().is_some());
    assert!(app.menu().is_none());
    press(&mut app, KeyCode::Esc);
    assert!(app.dialog().is_none());
}

#[test]
fn dialog_is_modal() {
    let mut app = TestApp::new();
    app.overlays
        .push(Overlay::Dialog(Dialog::input("t", "x", Action::NewBranch)));
    let focus = app.focus;

    type_text(&mut app, "q"); // finisce nel campo, non esce
    press(&mut app, KeyCode::F(10)); // non apre il menu
    press_mods(&mut app, KeyCode::Char('f'), KeyModifiers::ALT);

    assert!(app.running);
    assert!(app.menu().is_none());
    assert_eq!(app.focus, focus);
    assert_eq!(app.dialog().unwrap().input, "q");
}

#[test]
fn q_in_main_panel_no_longer_quits() {
    let mut app = TestApp::new();
    press(&mut app, KeyCode::Char('q'));
    assert!(app.running);
}

#[test]
fn alt_x_quits_even_with_dialog_open() {
    let mut app = TestApp::new();
    run_menu_action(&mut app, MenuAction::About);
    press_mods(&mut app, KeyCode::Char('x'), KeyModifiers::ALT);
    assert!(!app.running);
}

#[test]
fn ctrl_c_quits_even_with_dialog_or_menu_open() {
    let mut app = TestApp::new();
    press_mods(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!app.running);

    let mut app = TestApp::new();
    run_menu_action(&mut app, MenuAction::About);
    press_mods(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!app.running);
}

#[test]
fn altgr_like_combo_is_not_swallowed_as_a_menu_shortcut() {
    // Alt+Ctrl è come molte piattaforme riportano AltGr: non deve aprire
    // un menu, e il carattere deve comunque raggiungere il campo di testo.
    let mut app = TestApp::new();
    app.overlays
        .push(Overlay::Dialog(Dialog::input("t", "x", Action::NewBranch)));
    press_mods(
        &mut app,
        KeyCode::Char('e'),
        KeyModifiers::ALT | KeyModifiers::CONTROL,
    );
    assert!(app.menu().is_none());
    assert_eq!(app.dialog().unwrap().input, "e");
}

#[test]
fn unimplemented_items_report_it() {
    let mut app = TestApp::new();
    run_menu_action(&mut app, MenuAction::OpenRepository);
    assert!(app.dialog().unwrap().text.contains("not available"));
}
