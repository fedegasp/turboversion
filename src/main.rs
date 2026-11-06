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

use std::collections::VecDeque;
use std::io;
use std::panic;
use std::process::ExitCode;

use git::git2::GitRepo;
use msg::Msg;
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
    load_initial_state(&mut model, &worker);

    match run_tui(&mut model, &worker) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("turboversion: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Esegue i `Cmd` di avvio prima del primo frame, aspettando ogni esito.
/// Bloccare va bene qui: non c'è ancora un terminale né un loop di eventi
/// da tenere reattivo (vedi `app::run`).
///
/// È una coda, non un semplice `for`: anche un esito può chiedere altri
/// `Cmd`, e buttarli lascerebbe l'avvio a metà senza che si veda.
fn load_initial_state(model: &mut app::Model, worker: &Worker) {
    let mut queue: VecDeque<msg::Cmd> = app::init_cmds().into();
    while let Some(cmd) = queue.pop_front() {
        worker.send(cmd);
        let Some(msg) = worker.recv() else {
            // Canale chiuso: il thread worker non c'è più e nessun esito
            // arriverà. Lo si riporta a `update` come ogni altro evento,
            // così la UI parte già dicendo che le operazioni git non sono
            // disponibili, invece di mostrare pannelli vuoti senza
            // spiegazione.
            app::update(model, Msg::WorkerLost);
            return;
        };
        queue.extend(app::update(model, msg));
    }
}

/// Isola init/restore del terminale in una funzione propria, così che
/// `ratatui::restore()` giri sempre, anche se `app::run` ritorna un errore.
fn run_tui(model: &mut app::Model, worker: &Worker) -> io::Result<()> {
    // `init` abilita raw mode + schermo alternativo e installa un panic hook
    // che ripristina il terminale prima di stampare il panic.
    let mut terminal = ratatui::init();
    spare_the_ui_from_worker_panics();
    let result = app::run(model, worker, &mut terminal);
    ratatui::restore();
    result
}

/// Incapsula l'hook di panic installato da `ratatui::init` con uno che lo
/// salta quando il panic arriva dal thread worker.
///
/// L'hook è globale e gira sul thread che va in panic, qualunque sia:
/// quello di ratatui ripristina il terminale (raw mode off, uscita dallo
/// schermo alternativo). Per un panic del thread UI è giusto — si sta
/// chiudendo tutto — ma per un panic del worker no: il thread UI è vivo e
/// continua a disegnare, e si ritroverebbe a farlo su un terminale
/// smontato, sopra la shell dell'utente.
///
/// Il worker cattura comunque i propri panic (vedi `Worker::spawn`) e
/// annuncia la perdita con un `Msg`; l'hook gira però *prima* della
/// cattura, quindi senza questo filtro la cattura non basterebbe. Il panic
/// del worker resta così senza traccia sul terminale: stamparla finirebbe
/// sullo schermo alternativo, in mezzo alla UI, ed è il dialog "Git
/// unavailable" a dire all'utente cos'è successo.
///
/// Un'eccezione: il worker parte e serve i `Cmd` d'avvio *prima* di qui,
/// quindi un suo panic in quella finestra passa dall'hook di default e
/// finisce stampato su stderr — innocuo, perché lo schermo alternativo
/// non esiste ancora, ed è l'unico caso in cui resta una traccia.
fn spare_the_ui_from_worker_panics() {
    let installed = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if worker::is_worker_thread() {
            return;
        }
        installed(info);
    }));
}
