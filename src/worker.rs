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
//!
//! Il thread può morire anche senza che il `Worker` venga droppato: basta
//! un panic dentro `effects::run`. Da quel momento ogni `Cmd` resterebbe
//! senza esito, quindi i tre metodi riportano se il thread c'è ancora
//! (`Delivery`, `Received`, `None` da `recv`) invece di far sparire
//! l'informazione: chi chiama decide cosa farne.

use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;

use crate::effects;
use crate::git::repo::Repo;
use crate::msg::{Cmd, Msg};

/// Nome del thread worker. Serve a distinguerlo dal thread UI nell'hook di
/// panic (vedi `is_worker_thread`): è l'unico modo che l'hook ha di sapere
/// su quale thread sta girando.
const THREAD_NAME: &str = "git-worker";

/// Se il thread corrente è quello che esegue i `Cmd`. Lo chiede l'hook di
/// panic installato in `main.rs`: un panic qui non deve smontare il
/// terminale, perché il thread UI sta ancora disegnando.
pub(crate) fn is_worker_thread() -> bool {
    thread::current().name() == Some(THREAD_NAME)
}

/// Esito dell'invio di un `Cmd` al worker. Non è un `Result`: inviare e
/// non guardare l'esito è legittimo (la perdita del worker si vede
/// comunque sul canale di ritorno), quindi ignorarlo non deve diventare un
/// errore di compilazione.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Delivery {
    /// Il `Cmd` è in coda per il thread worker.
    Queued,
    /// Il thread worker non c'è più: il `Cmd` non verrà eseguito e nessun
    /// `Msg` di esito arriverà.
    WorkerLost,
}

/// Cosa trova il thread UI sul canale di ritorno del worker.
#[derive(Debug)]
pub(crate) enum Received {
    /// Un `Msg` di esito, pronto da applicare.
    Msg(Msg),
    /// Niente di pronto adesso: il worker è vivo e o sta ancora lavorando,
    /// o non ha nulla da fare.
    Pending,
    /// Il canale è chiuso: il thread worker non c'è più. Distinto da
    /// `Pending` perché è definitivo, mentre `Pending` passa.
    WorkerLost,
}

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
    ///
    /// Un panic dentro `effects::run` (un difetto del backend, o di
    /// `git2`) non srotola il thread: viene catturato, e il worker chiude
    /// annunciandolo con `Msg::WorkerLost`. Lasciarlo srotolare farebbe
    /// sparire il thread in silenzio, e soprattutto farebbe girare l'hook
    /// di panic — che in questa applicazione ripristina il terminale —
    /// mentre il thread UI continua a disegnare.
    pub(crate) fn spawn(mut repo: Box<dyn Repo + Send>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();

        let handle = thread::Builder::new()
            .name(THREAD_NAME.to_string())
            .spawn(move || {
                for cmd in cmd_rx {
                    // `AssertUnwindSafe`: dopo un panic il `Repo` potrebbe
                    // essere in uno stato incoerente, ed è esattamente per
                    // questo che non lo si usa più — il loop termina
                    // invece di servire il `Cmd` successivo.
                    let Ok(result) =
                        panic::catch_unwind(AssertUnwindSafe(|| effects::run(cmd, repo.as_mut())))
                    else {
                        let _ = msg_tx.send(Msg::WorkerLost);
                        break;
                    };
                    if msg_tx.send(result).is_err() {
                        // Il thread UI ha già droppato il `Receiver` dei
                        // `Msg` (es. uscita in corso): niente da fare, si
                        // esce dal loop e il thread termina.
                        break;
                    }
                }
            });

        match handle {
            Ok(handle) => Self {
                cmd_tx: Some(cmd_tx),
                msg_rx,
                handle: Some(handle),
            },
            // Il sistema non ha concesso un thread. Non è un caso da
            // gestire a parte: è la stessa condizione di un worker morto,
            // e l'applicazione la racconta già.
            Err(_) => Self {
                cmd_tx: None,
                msg_rx,
                handle: None,
            },
        }
    }

    /// Invia un `Cmd` al worker. Non bloccante: il canale è illimitato e il
    /// worker lo consuma sul proprio thread.
    pub(crate) fn send(&self, cmd: Cmd) -> Delivery {
        match self.cmd_tx.as_ref().map(|tx| tx.send(cmd)) {
            Some(Ok(())) => Delivery::Queued,
            // `None` solo durante `Drop`, `Some(Err(_))` se il thread non
            // riceve più: in entrambi i casi il `Cmd` è perso.
            _ => Delivery::WorkerLost,
        }
    }

    /// Preleva un `Msg` di esito già pronto, senza bloccare.
    pub(crate) fn try_recv(&self) -> Received {
        match self.msg_rx.try_recv() {
            Ok(msg) => Received::Msg(msg),
            Err(mpsc::TryRecvError::Empty) => Received::Pending,
            Err(mpsc::TryRecvError::Disconnected) => Received::WorkerLost,
        }
    }

    /// Attende bloccando il prossimo `Msg` di esito. Usato solo da
    /// `main.rs` per i `Cmd` di avvio, prima di disegnare il primo frame:
    /// non ci sono ancora un terminale né un loop di eventi da bloccare.
    ///
    /// `None` vuol dire una cosa sola: il canale si è chiuso senza altri
    /// `Msg`, cioè il thread worker non c'è più. Non serve un `Received`
    /// qui, perché un'attesa bloccante non può restituire "non ancora".
    pub(crate) fn recv(&self) -> Option<Msg> {
        self.msg_rx.recv().ok()
    }

    /// Un `Worker` senza thread dietro, con in mano i due capi opposti dei
    /// suoi canali: il `Sender` con cui fingere gli esiti e il `Receiver`
    /// su cui leggere i `Cmd` inviati. Droppare uno dei due capi riproduce
    /// la morte del worker, in modo deterministico e senza thread di
    /// mezzo.
    #[cfg(test)]
    pub(crate) fn fake() -> (Self, mpsc::Sender<Msg>, mpsc::Receiver<Cmd>) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
        let worker = Self {
            cmd_tx: Some(cmd_tx),
            msg_rx,
            handle: None,
        };
        (worker, msg_tx, cmd_rx)
    }

    /// Un `Worker` i cui canali sono già chiusi: lo stato "thread perso".
    /// I due capi restituiti da `fake` vengono droppati sul posto.
    #[cfg(test)]
    pub(crate) fn lost() -> Self {
        Self::fake().0
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
    use crate::git::domain::{Branch, Commit, CommitId, DiffTarget, FileDiff, FileEntry};
    use crate::git::mock::MockGitRepo;
    use crate::git::repo::GitError;
    use std::time::{Duration, Instant};

    /// Un `Repo` che va in panic alla prima lettura: serve a esercitare
    /// un backend difettoso davvero, invece di simulare solo i canali
    /// chiusi.
    struct PanickingRepo;

    impl Repo for PanickingRepo {
        fn branches(&self) -> Vec<Branch> {
            panic!("repo rotto");
        }
        fn commits(&self) -> Vec<Commit> {
            panic!("repo rotto");
        }
        fn files(&self) -> Vec<FileEntry> {
            panic!("repo rotto");
        }
        fn files_of_commit(&self, _id: &CommitId) -> Result<Vec<FileEntry>, GitError> {
            panic!("repo rotto");
        }
        fn file_diff(&self, _target: &DiffTarget) -> Result<FileDiff, GitError> {
            panic!("repo rotto");
        }
        fn create_branch(&mut self, _name: &str) -> Result<Branch, GitError> {
            panic!("repo rotto");
        }
        fn delete_branch(&mut self, _name: &str) -> Result<(), GitError> {
            panic!("repo rotto");
        }
    }

    #[test]
    fn runs_a_cmd_on_the_worker_thread_and_delivers_its_msg() {
        let worker = Worker::spawn(Box::new(MockGitRepo::new()));
        assert_eq!(worker.send(Cmd::LoadAll), Delivery::Queued);

        let msg = worker.recv().expect("il worker deve rispondere");
        assert!(matches!(msg, Msg::Loaded { .. }));
    }

    #[test]
    fn try_recv_does_not_block_when_nothing_is_ready() {
        let worker = Worker::spawn(Box::new(MockGitRepo::new()));
        assert!(matches!(worker.try_recv(), Received::Pending));
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

    #[test]
    fn a_closed_channel_is_reported_instead_of_being_swallowed() {
        let worker = Worker::lost();
        assert_eq!(worker.send(Cmd::LoadAll), Delivery::WorkerLost);
        assert!(matches!(worker.try_recv(), Received::WorkerLost));
        assert!(worker.recv().is_none());
    }

    #[test]
    fn a_panicking_repo_is_announced_instead_of_unwinding_the_thread() {
        let worker = Worker::spawn(Box::new(PanickingRepo));
        assert_eq!(worker.send(Cmd::LoadAll), Delivery::Queued);

        // Il panic viene catturato: il thread non srotola (quindi l'hook
        // di panic non smonta il terminale sotto la UI) e la perdita
        // arriva per il canale normale, come un esito qualunque.
        assert!(matches!(worker.recv(), Some(Msg::WorkerLost)));
    }

    /// Un `Repo` che riferisce su quale thread è stato chiamato, usando il
    /// nome del ramo restituito come unico canale a disposizione.
    struct ThreadNameRepo;

    impl Repo for ThreadNameRepo {
        fn branches(&self) -> Vec<Branch> {
            vec![Branch {
                name: if is_worker_thread() {
                    "worker"
                } else {
                    "altro"
                }
                .to_string(),
                kind: crate::git::domain::BranchKind::Local,
                current: true,
            }]
        }
        fn commits(&self) -> Vec<Commit> {
            Vec::new()
        }
        fn files(&self) -> Vec<FileEntry> {
            Vec::new()
        }
        fn files_of_commit(&self, _id: &CommitId) -> Result<Vec<FileEntry>, GitError> {
            unreachable!("non usato da questo test")
        }
        fn file_diff(&self, _target: &DiffTarget) -> Result<FileDiff, GitError> {
            unreachable!("non usato da questo test")
        }
        fn create_branch(&mut self, _name: &str) -> Result<Branch, GitError> {
            unreachable!("non usato da questo test")
        }
        fn delete_branch(&mut self, _name: &str) -> Result<(), GitError> {
            unreachable!("non usato da questo test")
        }
    }

    #[test]
    fn the_thread_carries_the_name_the_panic_hook_looks_for() {
        // Senza il nome, l'hook di panic non saprebbe distinguere questo
        // thread da quello della UI e ripristinerebbe il terminale mentre
        // la UI sta ancora disegnando.
        assert!(!is_worker_thread(), "il thread del test non è il worker");

        let worker = Worker::spawn(Box::new(ThreadNameRepo));
        worker.send(Cmd::LoadAll);
        let Some(Msg::Loaded { branches, .. }) = worker.recv() else {
            panic!("atteso Msg::Loaded");
        };
        assert_eq!(branches[0].name, "worker");
    }
}
