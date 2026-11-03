//
// src/main.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

mod app;
mod dialog;
mod effects;
mod git;
mod input;
mod msg;
mod ui;

use std::io;

fn main() -> io::Result<()> {
    // `init` abilita raw mode + schermo alternativo e installa un panic hook
    // che ripristina il terminale prima di stampare il panic.
    let mut terminal = ratatui::init();

    let mut repo = git::mock::MockGitRepo::new();
    let mut model = app::Model::new();
    for cmd in app::init_cmds() {
        let msg = effects::run(cmd, &mut repo);
        app::update(&mut model, msg);
    }

    let result = app::run(&mut model, &mut repo, &mut terminal);
    ratatui::restore();
    result
}
