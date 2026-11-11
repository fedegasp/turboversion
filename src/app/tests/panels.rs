//
// src/app/tests/panels.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use super::*;

#[test]
fn space_on_an_empty_changes_list_does_not_panic() {
    let mut app = TestApp::new();
    update(
        &mut app.model,
        Msg::Loaded {
            branches: Vec::new(),
            commits: Vec::new(),
            files: Vec::new(),
        },
    );
    assert_eq!(
        app.working_copy_files.selected_index(),
        None,
        "lista vuota, nessuna riga"
    );

    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Char(' '));

    assert!(app.working_copy_files.is_empty());
}

#[test]
fn panel_next_and_prev_wrap_around() {
    assert_eq!(Panel::Diff.next(), Panel::Branches);
    assert_eq!(Panel::Branches.prev(), Panel::Diff);
    assert_eq!(Panel::Branches.next(), Panel::Remotes);
    assert_eq!(Panel::Remotes.next(), Panel::History);
    assert_eq!(Panel::History.prev(), Panel::Remotes);
    assert_eq!(Panel::Changes.prev(), Panel::History);
}

/// Il pannello a fuoco è l'unico a muoversi, anche quando il contenuto
/// non è una lista: `Model::content_mut` sceglie il cursore, `step` non
/// sa più quale sia.
#[test]
fn only_the_focused_panel_moves() {
    let mut app = TestApp::new();
    app.focus = Panel::Diff;
    press(&mut app, KeyCode::Down);
    assert_eq!(app.diff_scroll, 1);
    assert_eq!(app.history().selected_index(), Some(0));
    assert_eq!(app.working_copy_files.selected_index(), Some(0));

    app.focus = Panel::Changes;
    press(&mut app, KeyCode::Down);
    assert_eq!(app.working_copy_files.selected_index(), Some(1));
    assert_eq!(app.history().selected_index(), Some(0));
}
