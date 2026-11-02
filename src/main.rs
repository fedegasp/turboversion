//
// src/main.rs
// TurboVersion
//
// 2026-10-30 Federico Gasperini
//

mod app;
mod dialog;
mod mock;
mod theme;
mod ui;
mod widgets;

use std::io;

fn main() -> io::Result<()> {
    // `init` abilita raw mode + schermo alternativo e installa un panic hook
    // che ripristina il terminale prima di stampare il panic.
    let mut terminal = ratatui::init();
    let result = app::App::new().run(&mut terminal);
    ratatui::restore();
    result
}
