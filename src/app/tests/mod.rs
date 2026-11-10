//
// src/app/tests/mod.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

//! Test di `update` e dei sottomoduli di `app`, raggruppati per
//! tema; qui vive solo l'infrastruttura comune (`TestApp` e i suoi
//! helper).

use super::test_support::run_to_completion;
use super::*;
use crate::git::mock::MockGitRepo;
use crate::input;
use crate::msg::{Cmd, Msg};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::ops::{Deref, DerefMut};

mod branches;
mod changes;
mod diff;
mod overlays;
mod panels;
mod worker_loss;

/// Replica il ciclo di `run` (traduzione del tasto, `update`, eventuali
/// effetti contro un `MockGitRepo`, e gli effetti che i loro esiti chiedono
/// a loro volta) per i test, esponendo il `Model`
/// tramite `Deref`/`DerefMut`: un wrapper di comodo solo per i test, non
/// un tipo pubblico dell'app.
///
/// A differenza di `run`/`dispatch` di produzione, qui non passa da un
/// `Worker`: comporrebbe `update()` + `effects::run()` contro un vero
/// thread in background, rendendo i test dipendenti dal timing di quel
/// thread. Compone invece `update()` + `effects::run()` direttamente
/// contro `MockGitRepo`, esattamente come `model_for_tests`
/// (`test_support.rs`): resta deterministico e veloce, e non duplica la
/// logica di smistamento (che vive comunque solo in `update`).
pub(super) struct TestApp {
    pub(super) model: Model,
    pub(super) repo: MockGitRepo,
}

impl TestApp {
    pub(super) fn new() -> Self {
        let mut model = Model::new();
        let mut repo = MockGitRepo::new();
        run_to_completion(&mut model, &mut repo, init_cmds());
        Self { model, repo }
    }

    pub(super) fn send(&mut self, msg: Msg) {
        let cmds = update(&mut self.model, msg);
        self.run(cmds);
    }

    /// Esegue `cmds` e tutto ciò che i loro esiti chiedono. Serve ai test
    /// che trattengono i `Cmd` di `update` per consegnarli più tardi, o in
    /// un altro ordine.
    pub(super) fn run(&mut self, cmds: Vec<Cmd>) {
        run_to_completion(&mut self.model, &mut self.repo, cmds);
    }
}

impl Deref for TestApp {
    type Target = Model;
    fn deref(&self) -> &Model {
        &self.model
    }
}

impl DerefMut for TestApp {
    fn deref_mut(&mut self) -> &mut Model {
        &mut self.model
    }
}

fn press_mods(app: &mut TestApp, code: KeyCode, mods: KeyModifiers) {
    if let Some(msg) = input::key_to_msg(KeyEvent::new(code, mods)) {
        app.send(msg);
    }
}

pub(super) fn press(app: &mut TestApp, code: KeyCode) {
    press_mods(app, code, KeyModifiers::NONE);
}

fn type_text(app: &mut TestApp, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// Trova (menu, item) per un'azione, cercando in `MENUS` invece di usare
/// indici magici nei test: una voce che cambia posizione non rompe i test.
fn menu_position(action: MenuAction) -> (usize, usize) {
    for (mi, (_, items)) in MENUS.iter().enumerate() {
        if let Some(ii) = items.iter().position(|(_, a)| *a == action) {
            return (mi, ii);
        }
    }
    panic!("azione {action:?} non trovata in MENUS");
}

/// Indice di un ramo locale cercato per nome: i test non dipendono
/// così dall'ordine dei dati di partenza.
fn local_position(app: &TestApp, name: &str) -> usize {
    app.local_branches
        .items()
        .iter()
        .position(|b| b.name == name)
        .unwrap_or_else(|| panic!("ramo locale {name:?} non trovato"))
}

fn run_menu_action(app: &mut TestApp, action: MenuAction) {
    let (menu, item) = menu_position(action);
    app.overlays.push(Overlay::Menu(MenuState { menu, item }));
    press(app, KeyCode::Enter);
}
