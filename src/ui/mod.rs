//
// src/ui/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Orchestrazione generale del rendering: layout dei pannelli e dispatch
//! verso `ui::panels`. Legge il `Model` e disegna soltanto — non chiama mai
//! nulla in `git/`.

pub(crate) mod hints;
pub(crate) mod panels;
pub(crate) mod text;
pub(crate) mod theme;
pub(crate) mod widgets;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::Line,
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::{Model, Panel, PanelContent};
use widgets::window::Window;

/// Sotto questa larghezza si mostra un solo pannello (quello con il focus).
const NARROW: u16 = 80;

/// Dimensioni minime sotto le quali si mostra un messaggio invece della UI.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 6;

/// Disegna il frame. `diff_viewport` è un valore in uscita: ci finiscono
/// le righe utili del pannello Diff, che solo il layout conosce e che serve
/// a `update` per limitare lo scorrimento. Non è un campo del `Model`
/// perché disegnare non deve scriverlo: a portarcelo è `run`, con un
/// `Msg::DiffViewport`, quando il valore cambia. Se il pannello non viene
/// disegnato (terminale minuscolo, oppure troppo stretto e il focus
/// altrove) il valore resta quello che era: non c'è un'altezza nuova da
/// riportare, e senza il pannello non c'è nemmeno modo di scorrerlo.
pub(crate) fn view(f: &mut Frame, model: &mut Model, diff_viewport: &mut u16) {
    let area = f.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(f, area);
        return;
    }

    let [menu_area, body, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    let panel_layout = layout_panels(body, model.focus, area.width);

    for (p, r) in &panel_layout {
        if *p == Panel::Diff {
            *diff_viewport = Window::inner(*r).height;
        }
        draw_panel(f, model, *p, *r);
    }

    panels::menubar::render_menubar(f, menu_area, model.menu());
    panels::statusbar::render_statusbar(f, status_area, hints::active(model));
    if let Some(m) = model.menu() {
        panels::menubar::render_dropdown(f, menu_area, m);
    }
    if let Some(d) = model.dialog() {
        panels::dialog::render_dialog(f, d);
    }
}

fn draw_too_small(f: &mut Frame, area: Rect) {
    f.render_widget(Clear, area);
    let lines = [
        Line::raw("Terminal too small"),
        Line::raw(format!("Resize to at least {MIN_WIDTH}x{MIN_HEIGHT}")),
    ];
    f.render_widget(
        Paragraph::new(lines.to_vec())
            .style(theme::window())
            .alignment(Alignment::Center),
        area,
    );
}

/// Le aree dei pannelli. Dipendono dall'area totale e dal pannello a
/// fuoco, e non devono dipendere da altro: l'altezza del pannello Diff che
/// ne esce rientra nel `Model` come `Msg::DiffViewport`, e quel `Msg`
/// cambia `diff_scroll`. Se la misura dipendesse a sua volta da
/// `diff_scroll` (o da qualunque altra cosa che quel `Msg` tocca), ogni
/// frame ne chiederebbe un altro e il ciclo principale non tornerebbe mai
/// a riposo.
fn layout_panels(body: Rect, focus: Panel, term_width: u16) -> Vec<(Panel, Rect)> {
    if term_width < NARROW {
        return vec![(focus, body)];
    }
    let [left, right] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(body);
    // I rami locali sono quelli su cui si lavora: prendono la parte alta,
    // più grande, della colonna.
    let [branches, remotes] =
        Layout::vertical([Constraint::Percentage(60), Constraint::Min(0)]).areas(left);
    let [hist, bottom] =
        Layout::vertical([Constraint::Percentage(45), Constraint::Min(0)]).areas(right);
    let [chg, diff] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Min(0)]).areas(bottom);
    vec![
        (Panel::Branches, branches),
        (Panel::Remotes, remotes),
        (Panel::History, hist),
        (Panel::Changes, chg),
        (Panel::Diff, diff),
    ]
}

fn draw_panel(f: &mut Frame, model: &mut Model, panel: Panel, area: Rect) {
    let focused = model.focus == panel;
    f.render_widget(Window::new(panel.title(), focused), area);
    let inner = Window::inner(area);
    let sel = theme::selection(focused);

    // Quale lista finisca in quale pannello lo dice `Model::content_mut`,
    // che è l'unica tabella pannello -> dati: qui si sceglie solo con
    // quale resa disegnarla. I `ListPanel` arrivano interi, ma da `ui` di
    // essi si può solo leggere e prestare il `ListState` (`parts_mut`):
    // i metodi che cambiano lista e selezione non sono raggiungibili da
    // qui.
    match model.content_mut(panel) {
        PanelContent::Branches(list) => {
            let (items, state) = list.parts_mut();
            panels::branches::render(f, items, inner, sel, state);
        }
        PanelContent::History(list) => {
            let (items, state) = list.parts_mut();
            panels::history::render(f, items, inner, sel, state);
        }
        PanelContent::Files(list) => {
            let (items, state) = list.parts_mut();
            panels::changes::render(f, items, inner, sel, state);
        }
        // Il limite dello scorrimento (`viewport`) serve solo a `update`:
        // al renderer basta sapere da quale riga partire.
        PanelContent::Diff { lines, scroll, .. } => panels::diff::render(f, lines, inner, *scroll),
    }
}

/// Disegna un frame replicando il giro di `app::run`: l'altezza misurata
/// del pannello Diff rientra nel `Model` come `Msg`, invece di essere
/// scritta mentre si disegna. I test di rendering che scorrono il diff
/// vedono così gli stessi limiti dell'app vera.
#[cfg(test)]
fn draw_once<B: ratatui::backend::Backend>(t: &mut ratatui::Terminal<B>, model: &mut Model) {
    let mut diff_viewport = model.diff_viewport;
    t.draw(|f| view(f, model, &mut diff_viewport)).unwrap();
    if diff_viewport != model.diff_viewport {
        crate::app::update(model, crate::msg::Msg::DiffViewport(diff_viewport));
    }
}

/// Renderizza `model` su un backend di `w` x `h` colonne e restituisce il
/// buffer disegnato. Serve ai test che guardano gli attributi delle celle
/// (colori, grassetto) e non solo i simboli.
#[cfg(test)]
pub(crate) fn render_buffer(w: u16, h: u16, model: &mut Model) -> ratatui::buffer::Buffer {
    use ratatui::{backend::TestBackend, Terminal};

    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    draw_once(&mut t, model);
    t.backend().buffer().clone()
}

/// Il contenuto testuale del frame, riga per riga. Condiviso dai test di
/// rendering in `ui` e nei suoi sottomoduli.
#[cfg(test)]
pub(crate) fn render(w: u16, h: u16, model: &mut Model) -> String {
    let buf = render_buffer(w, h, model);
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model_for_tests;

    #[test]
    fn renders_80x24() {
        let out = render(80, 24, &mut model_for_tests());
        println!("{out}");
        assert!(out.contains("History"));
    }

    /// Lo stato che il `Model` tiene, nella forma che un frame potrebbe
    /// alterare: righe delle liste, cursori, scorrimento del diff, focus e
    /// overlay aperti.
    fn model_snapshot(model: &Model) -> Vec<usize> {
        let lists = [
            (
                model.local_branches.len(),
                model.local_branches.selected_index(),
            ),
            (
                model.remote_branches.len(),
                model.remote_branches.selected_index(),
            ),
            (model.history().len(), model.history().selected_index()),
            (model.files.len(), model.files.selected_index()),
        ];
        let mut out: Vec<usize> = lists
            .iter()
            .flat_map(|(len, sel)| [*len, sel.unwrap_or(usize::MAX)])
            .collect();
        out.push(usize::from(model.diff_scroll));
        out.push(model.diff.len());
        out.push(model.focus as usize);
        out.push(model.overlays.len());
        out
    }

    /// Disegnare non cambia il `Model`. Il compilatore già lo impone per le
    /// liste — da `ui` un `ListPanel` si legge, e del suo `ListState` si
    /// presta solo quanto serve a ratatui per ricordare l'offset — e questo
    /// test guarda lo stesso dalla parte del comportamento: due frame di
    /// fila lasciano righe, cursori e scorrimento dove stavano.
    #[test]
    fn drawing_a_frame_leaves_the_model_as_it_was() {
        let mut model = model_for_tests();
        model.focus = Panel::Changes;
        model.local_branches.select(1);
        model.files.select(1);
        model.diff_scroll = 1;

        // Il primo frame assesta l'altezza del pannello Diff (che rientra
        // come `Msg`, non scritta da chi disegna) e con essa il limite
        // dello scorrimento: da qui in poi non c'è più nulla da assestare.
        render(100, 30, &mut model);
        let before = model_snapshot(&model);

        render(100, 30, &mut model);
        assert_eq!(model_snapshot(&model), before);
    }

    #[test]
    fn renders_narrow_and_tiny_without_panic() {
        render(60, 20, &mut model_for_tests());
        render(20, 5, &mut model_for_tests());
        render(3, 2, &mut model_for_tests());
        // Largo ma basso: la colonna di sinistra si divide comunque in due.
        render(100, 6, &mut model_for_tests());

        // A terminale stretto si disegna il solo pannello a fuoco: anche
        // quando è il nuovo Remotes, che a quella larghezza prende tutto il
        // corpo invece della mezza colonna di sinistra.
        let mut model = model_for_tests();
        model.focus = Panel::Remotes;
        assert!(render(60, 20, &mut model).contains("origin/main"));
    }

    #[test]
    fn below_minimum_size_shows_message_instead_of_garbled_panels() {
        let out = render(30, 4, &mut model_for_tests());
        println!("{out}");
        assert!(out.contains("too small"));
    }

    #[test]
    fn narrow_terminal_shows_only_the_focused_panel() {
        for focus in [
            Panel::Branches,
            Panel::Remotes,
            Panel::History,
            Panel::Changes,
            Panel::Diff,
        ] {
            let body = Rect::new(0, 0, NARROW - 1, 24);
            let panel_layout = layout_panels(body, focus, NARROW - 1);
            assert_eq!(panel_layout, vec![(focus, body)]);
        }
    }

    /// L'anello fra layout e `update`: l'altezza con cui il pannello Diff
    /// è stato disegnato arriva nel `Model`, dove il limite dello
    /// scorrimento la va a leggere.
    #[test]
    fn drawing_reports_the_height_of_the_diff_panel() {
        let mut model = model_for_tests();
        assert_eq!(model.diff_viewport, 0, "nessun frame disegnato");

        // A terminale stretto si disegna il solo pannello a fuoco: con il
        // focus sul Diff prende tutto il corpo, un'altezza che nessun altro
        // pannello ha mai.
        model.focus = Panel::Diff;
        render(60, 20, &mut model);
        let whole_body = Window::inner(Rect::new(0, 1, 60, 18)).height;
        assert_eq!(model.diff_viewport, whole_body);

        // A terminale largo il Diff è uno dei cinque e si rimisura.
        render(100, 30, &mut model);
        let diff = layout_panels(Rect::new(0, 1, 100, 28), model.focus, 100)
            .into_iter()
            .find_map(|(p, r)| (p == Panel::Diff).then_some(r))
            .expect("il pannello Diff è visibile a 100 colonne");
        assert_eq!(model.diff_viewport, Window::inner(diff).height);
        assert_ne!(model.diff_viewport, whole_body, "altezza rimisurata");
    }

    /// Il contrario del test sopra: l'altezza che si registra è quella del
    /// pannello Diff, non quella del pannello che capita di disegnare. A 60
    /// colonne con il focus altrove il Diff non compare affatto.
    #[test]
    fn a_panel_that_is_not_the_diff_does_not_report_its_height() {
        let mut model = model_for_tests();
        model.focus = Panel::Changes;
        render(60, 20, &mut model);
        assert_eq!(model.diff_viewport, 0, "il Diff non è stato disegnato");
    }

    #[test]
    fn wide_terminal_shows_all_panels() {
        let body = Rect::new(0, 0, NARROW, 24);
        let panel_layout = layout_panels(body, Panel::History, NARROW);
        let shown: Vec<Panel> = panel_layout.iter().map(|(p, _)| *p).collect();
        assert_eq!(
            shown,
            vec![
                Panel::Branches,
                Panel::Remotes,
                Panel::History,
                Panel::Changes,
                Panel::Diff
            ]
        );
    }

    /// Testo delle righe su cui cade l'evidenziazione del pannello a fuoco
    /// (l'unico sfondo di quel colore nel frame), per verificare non solo
    /// *cosa* si disegna ma *dove* finisce la selezione.
    fn focused_selection_rows(w: u16, h: u16, model: &mut Model) -> Vec<String> {
        let want = theme::selection(true).bg;
        let buf = render_buffer(w, h, model);
        (0..h)
            .filter_map(|y| {
                let row: String = (0..w)
                    .filter(|&x| want == Some(buf[(x, y)].bg))
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect();
                (!row.trim().is_empty()).then_some(row)
            })
            .collect()
    }

    #[test]
    fn the_remotes_panel_highlights_its_own_selection() {
        let mut model = model_for_tests();
        model.focus = Panel::Remotes;
        model.remote_branches.select(1);
        // La selezione dei locali resta sulla prima riga: se i due pannelli
        // leggessero lo stesso `ListState`, l'evidenziazione cadrebbe su
        // "main" invece che sul secondo remoto.
        model.local_branches.select(0);

        let rows = focused_selection_rows(100, 30, &mut model);
        assert_eq!(rows.len(), 1, "una sola riga evidenziata: {rows:?}");
        assert!(rows[0].contains("origin/develop"), "{rows:?}");
    }

    #[test]
    fn remote_branches_have_their_own_panel_and_tags_are_shown_nowhere() {
        let out = render(100, 30, &mut model_for_tests());
        println!("{out}");
        let lines: Vec<&str> = out.lines().collect();
        let split = lines
            .iter()
            .position(|l| l.contains("Remotes"))
            .expect("il pannello Remotes deve comparire");
        let above = lines[..split].join("\n");
        let below = lines[split..].join("\n");

        assert!(above.contains("Branches"));
        assert!(above.contains("* main"));
        assert!(
            !above.contains("origin/"),
            "i remoti non vanno nel pannello Branches"
        );
        assert!(below.contains("origin/main"));
        assert!(!out.contains("v0.1.0"), "i tag non si mostrano più");
    }
}
