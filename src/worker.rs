//
// src/worker.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Thread worker che esegue i `Cmd` contro un `Repo` reale, fuori dal
//! thread UI. Un backend reale (`git::git2::GitRepo`) può bloccare su I/O,
//! quindi `effects::run` (che resta sincrona, vedi il commento in
//! `effects.rs`) non può più girare nello stesso ciclo di `app::run` come
//! succedeva con `MockGitRepo`.
//!
//! `Worker` possiede il `Repo` su un thread dedicato: riceve `Cmd` da un
//! canale, chiama `effects::run` e rimanda il `Msg` di esito su un secondo
//! canale. Il thread UI invia `Cmd` senza bloccarsi (`send`) e drena i `Msg`
//! pronti senza bloccarsi (`try_recv`), tranne all'avvio (vedi `main.rs`),
//! dove un'attesa bloccante prima del primo frame è accettabile (`recv`).

use std::sync::mpsc;
use std::thread;

use crate::effects;
use crate::git::repo::Repo;
use crate::msg::{Cmd, Msg};

pub(crate) struct Worker {
    // `Option` per poter droppare il sender esplicitamente in `Drop`,
    // *prima* di fare `join()`: è il drop del sender a far terminare il
    // `for cmd in cmd_rx` nel thread, altrimenti `join()` si bloccherebbe
    // per sempre (il campo verrebbe droppato solo *dopo* `Drop::drop`).
    cmd_tx: Option<mpsc::Sender<Cmd>>,
    msg_rx: mpsc::Receiver<Msg>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Avvia il thread worker, che resta in ascolto di `Cmd` finché il
    /// `Worker` non viene droppato.
    pub(crate) fn spawn(mut repo: Box<dyn Repo + Send>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();

        let handle = thread::spawn(move || {
            for cmd in cmd_rx {
                let result = effects::run(cmd, repo.as_mut());
                if msg_tx.send(result).is_err() {
                    // Il thread UI ha già droppato il `Receiver` dei `Msg`
                    // (es. uscita in corso): niente da fare, si esce dal
                    // loop e il thread termina.
                    break;
                }
            }
        });

        Self {
            cmd_tx: Some(cmd_tx),
            msg_rx,
            handle: Some(handle),
        }
    }

    /// Invia un `Cmd` al worker. Non bloccante: il canale è illimitato e il
    /// worker lo consuma sul proprio thread.
    pub(crate) fn send(&self, cmd: Cmd) {
        // Se il thread worker è morto (non dovrebbe succedere finché
        // `Worker` è vivo) l'invio fallisce silenziosamente: non c'è un
        // `Msg` di esito da generare comunque, e `update` non si aspetta
        // una risposta per ogni `Cmd` inviato.
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    /// Preleva un `Msg` di esito già pronto, senza bloccare. `None` se il
    /// worker non ha ancora finito di eseguire il/i `Cmd` in corso.
    pub(crate) fn try_recv(&self) -> Option<Msg> {
        self.msg_rx.try_recv().ok()
    }

    /// Attende bloccando il prossimo `Msg` di esito. Usato solo da
    /// `main.rs` per i `Cmd` di avvio, prima di disegnare il primo frame:
    /// non ci sono ancora un terminale né un loop di eventi da bloccare.
    pub(crate) fn recv(&self) -> Option<Msg> {
        self.msg_rx.recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Droppare il sender chiude il canale: il `for cmd in cmd_rx` nel
        // thread worker termina, e il thread può uscire da solo.
        self.cmd_tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::mock::MockGitRepo;
    use std::time::{Duration, Instant};

    #[test]
    fn runs_a_cmd_on_the_worker_thread_and_delivers_its_msg() {
        let worker = Worker::spawn(Box::new(MockGitRepo::new()));
        worker.send(Cmd::LoadAll);

        let msg = worker.recv().expect("il worker deve rispondere");
        assert!(matches!(msg, Msg::Loaded { .. }));
    }

    #[test]
    fn try_recv_does_not_block_when_nothing_is_ready() {
        let worker = Worker::spawn(Box::new(MockGitRepo::new()));
        assert!(worker.try_recv().is_none());
    }

    #[test]
    fn drop_joins_the_thread_instead_of_leaking_it() {
        let worker = Worker::spawn(Box::new(MockGitRepo::new()));
        let started = Instant::now();
        drop(worker);
        // Un join pulito deve essere pressoché immediato: se il thread
        // fosse rimasto bloccato in attesa di altri `Cmd`, `drop` non
        // tornerebbe mai (il test andrebbe in timeout, non solo lento).
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
