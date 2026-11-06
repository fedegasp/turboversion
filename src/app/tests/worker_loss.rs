//
// src/app/tests/worker_loss.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Cosa succede al `Model` quando il thread che esegue le operazioni git
//! non c'è più. Qui si entra da `Msg::WorkerLost`, come fa il ciclo
//! principale quando trova il canale del worker chiuso; che un canale
//! chiuso (o un `Repo` che va in panic) produca davvero quella condizione
//! lo verificano i test di `worker.rs`.

use super::*;
use crate::dialog::Kind;

/// Il testo del dialog che annuncia la perdita, per non ripeterlo nei
/// test: basta un pezzo riconoscibile.
const ANNOUNCEMENT: &str = "no longer available";

#[test]
fn losing_the_worker_announces_it_once() {
    let mut app = TestApp::new();

    update(&mut app.model, Msg::WorkerLost);
    assert!(app.dialog().expect("un dialog").text.contains(ANNOUNCEMENT));
    assert_eq!(app.overlays.len(), 1);

    // Ogni `Cmd` successivo fallisce allo stesso modo: il secondo (e il
    // centesimo) annuncio non deve impilare un altro dialog identico.
    update(&mut app.model, Msg::WorkerLost);
    update(&mut app.model, Msg::WorkerLost);
    assert_eq!(app.overlays.len(), 1);
}

#[test]
fn the_announcement_does_not_come_back_after_being_dismissed() {
    let mut app = TestApp::new();
    update(&mut app.model, Msg::WorkerLost);
    press(&mut app, KeyCode::Enter);
    assert!(app.dialog().is_none());

    update(&mut app.model, Msg::WorkerLost);
    assert!(
        app.dialog().is_none(),
        "la perdita è già stata annunciata una volta"
    );
    assert!(!app.git_available(), "ma resta vera");
}

#[test]
fn the_app_stays_alive_and_what_is_loaded_stays_navigable() {
    let mut app = TestApp::new();
    update(&mut app.model, Msg::WorkerLost);
    press(&mut app, KeyCode::Enter);

    assert!(app.running, "non si chiude di colpo");

    // I dati già letti sono ancora validi: il cursore si muove come prima.
    app.focus = Panel::History;
    let rows = app.history().items().len();
    assert!(rows > 1, "il fixture deve avere più di una riga");
    press(&mut app, KeyCode::Down);
    assert_eq!(app.history().selected_index(), Some(1));
}

#[test]
fn the_announcement_stacks_over_an_open_overlay_without_losing_it() {
    let mut app = TestApp::new();
    run_menu_action(&mut app, MenuAction::NewBranch);
    assert_eq!(
        app.dialog().expect("il dialog del nuovo ramo").kind,
        Kind::Input
    );

    // La perdita può capitare mentre un overlay è aperto: l'annuncio si
    // impila sopra e riceve l'input, invece di sostituire ciò che
    // l'utente stava facendo.
    update(&mut app.model, Msg::WorkerLost);
    assert_eq!(app.overlays.len(), 2);
    assert!(app.dialog().expect("un dialog").text.contains(ANNOUNCEMENT));

    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.dialog().expect("il dialog di prima").kind,
        Kind::Input,
        "chiudendo l'annuncio si torna all'overlay sotto"
    );
}

#[test]
fn an_operation_asked_after_the_loss_is_refused_out_loud() {
    let mut app = TestApp::new();
    // Un ramo cancellabile davvero: quello corrente verrebbe rifiutato
    // per un'altra ragione, e il test non direbbe nulla.
    let i = local_position(&app, "develop");
    app.local_branches.select(i);
    let before = app.local_branches.len();

    update(&mut app.model, Msg::WorkerLost);
    press(&mut app, KeyCode::Enter); // chiude l'annuncio

    // Il menu continua a offrire le voci: chi le aziona deve ricevere una
    // risposta, non il silenzio di un `Cmd` che nessuno eseguirà.
    run_menu_action(&mut app, MenuAction::DeleteBranch);
    press(&mut app, KeyCode::Char('y'));

    assert!(
        app.dialog().expect("un dialog").text.contains(ANNOUNCEMENT),
        "il rifiuto dice perché, e lo dice con lo stesso testo dell'annuncio"
    );
    assert_eq!(
        app.local_branches.len(),
        before,
        "niente è stato cancellato"
    );
}

#[test]
fn an_action_that_asks_nothing_to_git_is_not_refused() {
    let mut app = TestApp::new();
    update(&mut app.model, Msg::WorkerLost);
    press(&mut app, KeyCode::Enter);

    // La guardia riguarda i `Cmd`, non i tasti: navigare, aprire il menu
    // e chiudere un dialog devono continuare a funzionare in silenzio.
    run_menu_action(&mut app, MenuAction::About);
    assert!(app.dialog().expect("About").title.contains("About"));
    press(&mut app, KeyCode::Esc);
    assert!(app.dialog().is_none());
}
