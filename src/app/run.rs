//
// src/app/run.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

use std::io;
use std::time::Duration;

use ratatui::{
    crossterm::event::{self, Event, KeyEventKind},
    DefaultTerminal,
};

use crate::input;
use crate::msg::Msg;
use crate::ui;
use crate::worker::Worker;

use super::model::Model;
use super::update::update;

/// Intervallo di `event::poll`: abbastanza breve da non introdurre un
/// ritardo percepibile nel drenare i `Msg` del worker tra un tasto e
/// l'altro, abbastanza lungo da non consumare CPU in un loop stretto da
/// ferma.
const POLL_TIMEOUT: Duration = Duration::from_millis(50);

/// Ciclo principale: disegna, aspetta fino a `POLL_TIMEOUT` un tasto (non
/// bloccante: un backend git reale può impiegare più di un giro a
/// rispondere, vedi `worker.rs`), lo traduce in `Msg` (vedi
/// `input::key_to_msg`) e lo passa a `dispatch`; poi, con o senza tasto,
/// drena i `Msg` già pronti dal worker e li applica con `update`.
pub(crate) fn run(
    model: &mut Model,
    worker: &Worker,
    terminal: &mut DefaultTerminal,
) -> io::Result<()> {
    // L'altezza del pannello Diff la conosce solo chi calcola il layout.
    // Vive qui, fuori dal `Model`, e rientra come `Msg` quando cambia:
    // disegnare non scrive nel `Model`.
    let mut diff_viewport = model.diff_viewport;

    while model.running {
        terminal.draw(|f| ui::view(f, model, &mut diff_viewport))?;
        if diff_viewport != model.diff_viewport {
            dispatch(model, worker, Msg::DiffViewport(diff_viewport));
        }

        if event::poll(POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(msg) = input::key_to_msg(key) {
                        dispatch(model, worker, msg);
                    }
                }
            }
            // Gli eventi Resize fanno semplicemente ridisegnare al giro dopo.
        }

        while let Some(msg) = worker.try_recv() {
            update(model, msg);
        }
    }
    Ok(())
}

/// Applica `msg` e inoltra al worker ogni `Cmd` richiesto da `update`. A
/// differenza del vecchio `dispatch` sincrono (sostituito da questo), il
/// `Msg` di esito non torna qui: arriva più tardi, drenato da `run` tramite
/// `Worker::try_recv`.
fn dispatch(model: &mut Model, worker: &Worker, msg: Msg) {
    for cmd in update(model, msg) {
        worker.send(cmd);
    }
}
