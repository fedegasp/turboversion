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
mod worker;

use std::io;
use std::process::ExitCode;

use git::git2::GitRepo;
use worker::Worker;

fn main() -> ExitCode {
    // Sceglie il backend usando il repository nella cwd *prima* di toccare
    // il terminale: se la cwd non è dentro un repository git, l'app deve
    // uscire con un messaggio chiaro, non con una TUI vuota o un panic.
    // `MockGitRepo` resta solo per i test.
    let repo = match GitRepo::discover(".") {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("turboversion: {err}");
            return ExitCode::FAILURE;
        }
    };

    let worker = Worker::spawn(Box::new(repo));
    let mut model = app::Model::new();
    // Bloccante va bene qui: è prima del primo frame, non c'è ancora un
    // terminale né un loop di eventi da bloccare (vedi `app::run`).
    for cmd in app::init_cmds() {
        worker.send(cmd);
        if let Some(msg) = worker.recv() {
            app::update(&mut model, msg);
        }
    }

    match run_tui(&mut model, &worker) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("turboversion: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Isola init/restore del terminale in una funzione propria, così che
/// `ratatui::restore()` giri sempre, anche se `app::run` ritorna un errore.
fn run_tui(model: &mut app::Model, worker: &Worker) -> io::Result<()> {
    // `init` abilita raw mode + schermo alternativo e installa un panic hook
    // che ripristina il terminale prima di stampare il panic.
    let mut terminal = ratatui::init();
    let result = app::run(model, worker, &mut terminal);
    ratatui::restore();
    result
}
