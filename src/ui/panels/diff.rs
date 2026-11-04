//
// src/ui/panels/diff.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "Diff": contenuto del diff del commit/file selezionato.

use ratatui::{layout::Rect, text::Line, widgets::Paragraph, Frame};

use crate::ui::text::expand_tabs;
use crate::ui::theme;

pub(crate) fn render(f: &mut Frame, diff: &[String], area: Rect, scroll: u16) {
    let lines: Vec<Line> = diff
        .iter()
        .map(|l| {
            let color = match l.chars().next() {
                Some('+') => Some(theme::ADDED),
                Some('-') => Some(theme::REMOVED),
                Some('@') => Some(theme::HUNK),
                _ => None,
            };
            let text = expand_tabs(l).into_owned();
            match color {
                Some(c) => Line::styled(text, c),
                None => Line::raw(text),
            }
        })
        .collect();
    // Niente `Wrap`: le righe troppo lunghe vengono troncate a destra,
    // come in `less -S`. È anche ciò che tiene onesto lo scorrimento: il
    // `Paragraph` conta righe *visive*, chi limita lo scorrimento conta
    // righe del diff, e senza a capo automatico le due quantità coincidono.
    // Con il wrap una sola riga lunga ne occupava diverse e il fondo del
    // diff finiva fuori portata.
    f.render_widget(
        Paragraph::new(lines)
            .style(theme::window())
            .scroll((scroll, 0)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use crate::app::{model_for_tests, Model, Panel};
    use crate::ui::render;

    /// `diff` righe larghe il doppio del pannello, per vedere cosa succede
    /// a una riga che non ci sta in larghezza.
    fn model_with_wide_lines(lines: usize) -> Model {
        let mut model = model_for_tests();
        model.focus = Panel::Diff;
        model.diff = (0..lines)
            .map(|i| format!("line {i} {}", "x".repeat(200)))
            .collect();
        model
    }

    /// Scorrendo dall'inizio alla fine, ogni riga del diff compare prima o
    /// poi, anche se sono tutte più larghe del pannello: nessun contenuto
    /// resta irraggiungibile. È la proprietà che l'a capo automatico
    /// rompeva, visto che il `Paragraph` scorre per righe visive mentre il
    /// limite dello scorrimento conta righe del diff.
    #[test]
    fn scrolling_a_diff_of_wide_lines_reaches_every_line() {
        let mut model = model_with_wide_lines(100);
        let mut seen = [false; 100];
        for _ in 0..120 {
            let out = render(100, 30, &mut model);
            for (i, seen) in seen.iter_mut().enumerate() {
                *seen |= out.contains(&format!("line {i} "));
            }
            crate::app::update(&mut model, crate::msg::Msg::Down);
        }
        let missing: Vec<usize> = (0..100).filter(|&i| !seen[i]).collect();
        assert!(missing.is_empty(), "righe mai visibili: {missing:?}");

        // E in fondo si ferma con l'ultima riga a fondo pannello.
        let out = render(100, 30, &mut model);
        assert!(out.contains("line 99 "), "{out}");
        assert_eq!(
            model.diff_scroll,
            100 - model.diff_viewport,
            "nessuna riga vuota sotto la fine del diff"
        );
    }

    /// Una riga più larga del pannello viene troncata, non mandata a capo:
    /// il fondo del diff resta raggiungibile. Con l'a capo automatico dieci
    /// righe lunghe ne occupavano una quarantina, il limite dello
    /// scorrimento (che conta righe del diff) restava a zero e le ultime
    /// righe non si potevano più vedere.
    #[test]
    fn a_line_wider_than_the_panel_does_not_push_the_end_of_the_diff_away() {
        let mut model = model_with_wide_lines(10);
        let out = render(100, 30, &mut model);
        println!("{out}");
        assert!(out.contains("line 0 "), "{out}");
        assert!(
            out.contains("line 9 "),
            "le dieci righe ci stanno tutte nel pannello: {out}"
        );
    }
}
