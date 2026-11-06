//
// src/app/run.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Ciclo principale, l'unico file di `app/` che fa I/O: disegna il frame,
//! legge gli eventi del terminale e trasporta i `Cmd` verso il worker e i
//! suoi `Msg` di ritorno. Non decide nulla sul `Model`: ogni cambiamento
//! passa da `update`.

use std::io;
use std::time::Duration;

use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyEventKind},
    DefaultTerminal, Terminal,
};

use crate::input;
use crate::msg::Msg;
use crate::ui;
use crate::worker::{Delivery, Received, Worker};

use super::model::Model;
use super::update::update;

/// Attesa massima di `event::poll` mentre il worker ha dei `Cmd` da
/// eseguire: abbastanza breve da non introdurre un ritardo percepibile tra
/// la fine di un'operazione git e la comparsa del suo esito a schermo.
const BUSY_POLL: Duration = Duration::from_millis(50);

/// Attesa massima quando non c'è alcun `Cmd` in volo: non c'è alcun esito
/// da raccogliere, e la reattività ai tasti la dà `event::poll`, che
/// ritorna appena arriva un evento (tasto o ridimensionamento). Non è
/// un'attesa infinita per non legare del tutto il ciclo alla correttezza
/// del conteggio dei `Cmd`: nel caso peggiore un esito inatteso aspetta un
/// secondo invece che per sempre.
const IDLE_POLL: Duration = Duration::from_secs(1);

/// Ciclo principale: disegna quando c'è qualcosa di nuovo da mostrare,
/// aspetta un evento del terminale (non bloccandosi per sempre: un backend
/// git reale può impiegare più di un giro a rispondere, vedi `worker.rs`),
/// lo traduce in `Msg` (vedi `input::key_to_msg`) e lo passa a `update`;
/// poi, con o senza evento, drena i `Msg` già pronti dal worker.
pub(crate) fn run(
    model: &mut Model,
    worker: &Worker,
    terminal: &mut DefaultTerminal,
) -> io::Result<()> {
    let mut cycle = Cycle::new(model.diff_viewport);

    while model.running {
        cycle.draw(model, worker, terminal)?;
        if event::poll(cycle.timeout())? {
            cycle.handle_event(model, worker, event::read()?);
        }
        cycle.drain(model, worker);
    }
    Ok(())
}

/// Ciò che il ciclo ricorda da un giro all'altro. Non è stato
/// dell'applicazione — quello è il `Model`, e lo scrive solo `update` —
/// ma stato di trasporto: quando ridisegnare, quanto attendere, e se la
/// perdita del worker è già stata riportata a `update`.
struct Cycle {
    /// Il `Model` è cambiato dall'ultimo frame disegnato, oppure la
    /// superficie su cui disegnarlo è cambiata (ridimensionamento). Finché
    /// è falso ridisegnare produrrebbe lo stesso identico frame.
    dirty: bool,
    /// Quanti `Cmd` sono stati consegnati al worker senza che il loro
    /// esito sia ancora tornato. Un `Cmd` produce esattamente un `Msg`
    /// (vedi `effects::run`), quindi il conto non può scendere sotto il
    /// numero di esiti davvero in arrivo: nel dubbio si attende di meno,
    /// mai di più.
    in_flight: usize,
    /// La perdita del worker è già stata riportata a `update`. Un canale
    /// chiuso resta chiuso: senza questa memoria ogni giro del ciclo
    /// rimanderebbe lo stesso `Msg`, e il ciclo non tornerebbe mai a
    /// riposo.
    loss_reported: bool,
    /// Altezza del pannello Diff misurata nell'ultimo frame. L'altezza la
    /// conosce solo chi calcola il layout: vive qui, fuori dal `Model`, e
    /// ci rientra come `Msg` quando cambia, così disegnare non scrive nel
    /// `Model`.
    viewport: u16,
}

impl Cycle {
    fn new(viewport: u16) -> Self {
        Self {
            // Il primo frame non ha un precedente da confrontare.
            dirty: true,
            in_flight: 0,
            loss_reported: false,
            viewport,
        }
    }

    fn timeout(&self) -> Duration {
        if self.dirty {
            // C'è già un frame da rifare: aspettare un evento prima di
            // mostrarlo lo ritarderebbe senza motivo. Capita dopo il giro
            // di `Msg::DiffViewport`, l'unico cambiamento del `Model` che
            // nasce *dopo* che il frame è stato disegnato.
            Duration::ZERO
        } else if self.in_flight == 0 {
            IDLE_POLL
        } else {
            BUSY_POLL
        }
    }

    /// Disegna un frame, se c'è motivo. Se il pannello Diff è stato
    /// disegnato con un'altezza diversa da quella nota al `Model`, la
    /// riporta come `Msg`: il limite dello scorrimento ne dipende, quindi
    /// il `Model` cambia e serve un altro frame.
    ///
    /// Generica sul backend solo per poter essere esercitata su un
    /// `TestBackend`: in produzione il terminale è sempre un
    /// `DefaultTerminal`.
    fn draw<B: Backend>(
        &mut self,
        model: &mut Model,
        worker: &Worker,
        terminal: &mut Terminal<B>,
    ) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        terminal.draw(|f| ui::view(f, model, &mut self.viewport))?;
        self.dirty = false;

        let measured = self.viewport;
        if measured != model.diff_viewport {
            self.dispatch(model, worker, Msg::DiffViewport(measured));
        }
        Ok(())
    }

    fn handle_event(&mut self, model: &mut Model, worker: &Worker, event: Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if let Some(msg) = input::key_to_msg(key) {
                    // Un tasto che `update` ignora non cambia il `Model`,
                    // ma distinguere i due casi costerebbe più del frame
                    // in più che si risparmia.
                    self.dispatch(model, worker, msg);
                }
            }
            // Un ridimensionamento non cambia il `Model` e non produce
            // quindi alcun `Msg`: cambia l'area su cui disegnarlo, e per
            // questo è l'unico evento che segna il frame da rifare senza
            // passare da `update`.
            Event::Resize(_, _) => self.dirty = true,
            _ => {}
        }
    }

    /// Applica `msg` e inoltra al worker ogni `Cmd` richiesto da `update`.
    /// Il `Msg` di esito non torna qui: arriva più tardi, raccolto da
    /// `drain`.
    fn dispatch(&mut self, model: &mut Model, worker: &Worker, msg: Msg) {
        self.dirty = true;
        for cmd in update(model, msg) {
            match worker.send(cmd) {
                Delivery::Queued => self.in_flight += 1,
                Delivery::WorkerLost => self.report_loss(model, worker),
            }
        }
    }

    /// Raccoglie gli esiti già pronti. Li passa da `dispatch` e non da
    /// `update` diretto perché anche un esito può chiedere altri `Cmd`:
    /// scartarli lascerebbe l'operazione a metà senza che si veda.
    fn drain(&mut self, model: &mut Model, worker: &Worker) {
        loop {
            match worker.try_recv() {
                Received::Msg(msg) => {
                    self.in_flight = self.in_flight.saturating_sub(1);
                    self.dispatch(model, worker, msg);
                }
                Received::Pending => return,
                Received::WorkerLost => {
                    self.report_loss(model, worker);
                    return;
                }
            }
        }
    }

    /// Riporta la perdita del worker, una volta sola. Alzare
    /// `loss_reported` *prima* di passare da `dispatch` rende innocuo il
    /// rientro: un `Cmd` che fallisse l'invio tornerebbe qui e non
    /// troverebbe nulla da fare.
    fn report_loss(&mut self, model: &mut Model, worker: &Worker) {
        if self.loss_reported {
            return;
        }
        self.loss_reported = true;
        // Nessuno dei `Cmd` consegnati tornerà più indietro: tenerli nel
        // conto terrebbe il ciclo sveglio a vuoto per sempre.
        self.in_flight = 0;
        self.dispatch(model, worker, Msg::WorkerLost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::domain::Branch;
    use crate::msg::Cmd;
    use ratatui::backend::TestBackend;

    use crate::app::model_for_tests;
    use crate::app::Overlay;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(100, 30)).expect("il backend di test non fallisce")
    }

    /// Il contenuto testuale del frame disegnato, per distinguere "ha
    /// ridisegnato" da "ha lasciato lì quello di prima".
    fn screen(t: &Terminal<TestBackend>) -> String {
        let buf = t.backend().buffer();
        buf.content().iter().map(|c| c.symbol()).collect()
    }

    /// Un `Model` caricato con un dialog di conferma già aperto: premere
    /// `Enter` su "Yes" è il modo più breve per far chiedere un `Cmd` a
    /// `update`.
    fn model_asking_to_delete(name: &str) -> Model {
        let mut model = model_for_tests();
        let mut dialog = super::super::dialogs::delete_branch_confirm(name);
        dialog.button = 0;
        model.overlays.push(Overlay::Dialog(dialog));
        model
    }

    #[test]
    fn a_resize_asks_for_a_new_frame_without_touching_the_model() {
        let (worker, _results, _sent) = Worker::fake();
        let mut model = model_for_tests();
        let mut cycle = Cycle::new(0);
        cycle.dirty = false;
        let (focus, overlays) = (model.focus, model.overlays.len());

        // È l'unico evento che non produce alcun `Msg`: il `Model` non
        // cambia, cambia l'area su cui disegnarlo. Senza questo ramo
        // ridimensionare la finestra non ridisegnerebbe più nulla.
        cycle.handle_event(&mut model, &worker, Event::Resize(120, 40));

        assert!(cycle.dirty, "il frame va rifatto sulla nuova area");
        assert_eq!(model.focus, focus, "e il Model resta com'era");
        assert_eq!(model.overlays.len(), overlays);
        assert_eq!(cycle.in_flight, 0, "niente da chiedere a git");
    }

    #[test]
    fn only_a_key_press_becomes_a_msg() {
        let (worker, _results, sent) = Worker::fake();
        let mut model = model_asking_to_delete("develop");
        let mut cycle = Cycle::new(0);
        cycle.dirty = false;

        let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let release = KeyEvent {
            kind: KeyEventKind::Release,
            ..press
        };

        // Il rilascio dello stesso tasto non è un evento dell'app: i
        // terminali che lo riportano non devono far accadere due volte
        // quello che l'utente ha chiesto una volta.
        cycle.handle_event(&mut model, &worker, Event::Key(release));
        assert!(!cycle.dirty, "niente da ridisegnare");
        assert!(model.dialog().is_some(), "il dialog è ancora aperto");
        assert!(sent.try_recv().is_err(), "e nessun Cmd è partito");

        cycle.handle_event(&mut model, &worker, Event::Key(press));
        assert!(cycle.dirty);
        assert!(model.dialog().is_none(), "la pressione ha accettato");
        assert!(matches!(sent.try_recv(), Ok(Cmd::DeleteBranch(_))));
    }

    #[test]
    fn the_wait_is_long_only_when_there_is_nothing_to_wait_for() {
        let mut cycle = Cycle::new(0);
        assert_eq!(cycle.timeout(), Duration::ZERO, "c'è un frame da rifare");

        cycle.dirty = false;
        assert_eq!(cycle.timeout(), IDLE_POLL, "nessun Cmd in volo");

        cycle.in_flight = 1;
        assert_eq!(cycle.timeout(), BUSY_POLL, "un esito sta per arrivare");
    }

    #[test]
    fn a_delivered_cmd_is_counted_until_its_result_comes_back() {
        let (worker, results, sent) = Worker::fake();
        let mut model = model_asking_to_delete("develop");
        let mut cycle = Cycle::new(0);
        cycle.dirty = false;

        cycle.dispatch(&mut model, &worker, Msg::Enter);
        assert!(matches!(sent.try_recv(), Ok(Cmd::DeleteBranch(_))));
        assert_eq!(cycle.in_flight, 1, "l'esito non è ancora tornato");
        assert!(cycle.dirty, "il dialog si è chiuso: il frame cambia");

        results
            .send(Msg::BranchDeleted {
                name: "develop".to_string(),
                result: Ok(()),
            })
            .expect("il canale di prova è aperto");
        cycle.drain(&mut model, &worker);
        assert_eq!(cycle.in_flight, 0);
    }

    #[test]
    fn drain_takes_every_pending_result_before_reporting_the_loss() {
        let (worker, results, _sent) = Worker::fake();
        let mut model = Model::new();
        let mut cycle = Cycle::new(0);

        let branch = Branch {
            name: "main".to_string(),
            kind: crate::git::domain::BranchKind::Local,
            current: true,
        };
        results
            .send(Msg::Loaded {
                branches: vec![branch],
                commits: Vec::new(),
                files: Vec::new(),
                diff: Vec::new(),
            })
            .expect("il canale di prova è aperto");
        // Il worker muore con un esito già in coda: quell'esito è valido
        // e va applicato lo stesso, non buttato insieme al canale.
        drop(results);

        cycle.drain(&mut model, &worker);
        assert_eq!(
            model.local_branches.len(),
            1,
            "l'esito in coda è stato applicato"
        );
        assert!(cycle.loss_reported, "e la perdita riportata subito dopo");
        assert!(!model.git_available());
    }

    #[test]
    fn the_loss_reaches_update_once_and_puts_the_cycle_back_to_rest() {
        let worker = Worker::lost();
        let mut model = model_for_tests();
        let mut cycle = Cycle::new(0);
        cycle.in_flight = 3;

        cycle.report_loss(&mut model, &worker);
        assert_eq!(cycle.in_flight, 0, "quei Cmd non torneranno mai");
        assert!(cycle.dirty, "c'è un annuncio da mostrare");
        assert_eq!(model.overlays.len(), 1);

        // Ogni giro successivo ritrova il canale chiuso: la seconda
        // chiamata non deve più toccare niente, altrimenti il ciclo
        // ridisegnerebbe per sempre.
        cycle.dirty = false;
        cycle.report_loss(&mut model, &worker);
        assert!(!cycle.dirty, "niente di nuovo da disegnare");
        assert_eq!(cycle.timeout(), IDLE_POLL, "il ciclo torna a riposo");
    }

    #[test]
    fn a_frame_is_drawn_only_when_something_changed() {
        let (worker, _results, _sent) = Worker::fake();
        let mut model = model_for_tests();
        let mut cycle = Cycle::new(0);
        let mut t = terminal();

        // Due frame: il primo assesta l'altezza del pannello Diff (che
        // rientra come `Msg` e quindi chiede un altro frame), il secondo
        // chiude il giro e lascia il ciclo a riposo.
        cycle.draw(&mut model, &worker, &mut t).expect("disegna");
        cycle.draw(&mut model, &worker, &mut t).expect("disegna");
        let first = screen(&t);
        assert!(first.contains("History"), "il primo frame esce sempre");
        assert!(!cycle.dirty);

        // Il `Model` cambia senza passare da `update`: il ciclo non lo sa
        // e non deve ridisegnare.
        model
            .overlays
            .push(Overlay::Dialog(super::super::dialogs::about()));
        cycle
            .draw(&mut model, &worker, &mut t)
            .expect("non disegna");
        assert_eq!(screen(&t), first, "niente di segnalato, niente di nuovo");

        cycle.dirty = true;
        cycle.draw(&mut model, &worker, &mut t).expect("disegna");
        assert_ne!(screen(&t), first, "segnalato: il frame si rifà");
    }

    #[test]
    fn the_second_frame_in_a_row_does_not_ask_for_another() {
        let (worker, _results, _sent) = Worker::fake();
        let mut model = model_for_tests();
        let mut cycle = Cycle::new(0);
        let mut t = terminal();

        // Il primo frame misura il pannello Diff e rimanda l'altezza come
        // `Msg`, che cambia il `Model`: da lì il secondo frame.
        cycle.draw(&mut model, &worker, &mut t).expect("disegna");
        assert!(cycle.dirty, "l'altezza misurata è entrata nel Model");

        // Il secondo deve chiudere il giro. Se l'altezza misurata
        // dipendesse da ciò che quel `Msg` cambia, i due si
        // inseguirebbero e il ciclo girerebbe al 100% di CPU senza che
        // nulla fallisca.
        cycle.draw(&mut model, &worker, &mut t).expect("disegna");
        assert!(!cycle.dirty, "nessun altro Msg::DiffViewport");
    }
}
