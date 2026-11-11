//
// src/ui/panels/diff.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "Diff": il diff del file selezionato in Changes, nel contesto
//! di Changes (working copy o commit). Quando non ci sono righe da
//! mostrare il pannello dice perché, con una riga di testo: un pannello
//! vuoto non distinguerebbe un diff in arrivo da un binario, da un errore o
//! dall'assenza di un file selezionato.

use ratatui::{layout::Rect, text::Line, widgets::Paragraph, Frame};

use crate::app::DiffNotice;
use crate::ui::text::expand_tabs;
use crate::ui::theme;

/// Perché il pannello Diff non ha righe da scorrere.
pub(crate) fn render_notice(f: &mut Frame, notice: DiffNotice<'_>, area: Rect) {
    let text = match notice {
        DiffNotice::NoFile => "No file selected",
        DiffNotice::Loading => "Loading...",
        DiffNotice::FilesNotRead => "No diff: the files of this commit could not be read",
        DiffNotice::Binary => "Binary file: no text to show",
        DiffNotice::NoTextChanges => "No text changes to show",
        DiffNotice::Failed(message) => message,
        DiffNotice::GitUnavailable => "Git unavailable: diff not loaded",
    };
    super::render_notice_text(f, text, area);
}

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
        model.show_diff_for_tests(
            (0..lines)
                .map(|i| format!("line {i} {}", "x".repeat(200)))
                .collect(),
        );
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

    /// Il testo del pannello Diff, a terminale stretto con il focus lì: il
    /// pannello prende tutto il corpo e nient'altro finisce nel testo.
    fn diff_panel_text(model: &mut Model) -> String {
        model.focus = Panel::Diff;
        render(60, 12, model)
    }

    /// Ogni caso senza righe da scorrere ha il suo testo, e tutti sono fatti
    /// di caratteri a larghezza certa.
    #[test]
    fn the_diff_panel_says_why_it_has_no_lines() {
        use crate::git::domain::{DiffSource, DiffTarget, FileDiff};
        use crate::git::repo::GitError;
        use crate::msg::Msg;

        // Il secondo file della working copy, chiesto e non ancora arrivato.
        let pending = || {
            let mut model = model_for_tests();
            model.focus = Panel::Changes;
            let cmds = crate::app::update(&mut model, Msg::Down);
            assert_eq!(cmds.len(), 1, "il diff è stato chiesto");
            let target = DiffTarget {
                source: DiffSource::WorkingCopy,
                path: model.working_copy_files.items()[1].path.clone(),
            };
            (model, target)
        };
        let arrive = |model: &mut Model, target, result| {
            crate::app::update(model, Msg::FileDiffLoaded { target, result });
        };

        let (mut model, _) = pending();
        assert!(diff_panel_text(&mut model).contains("Loading..."));

        let cases = [
            (Ok(FileDiff::Binary), "Binary file: no text to show"),
            (Ok(FileDiff::NoTextChanges), "No text changes to show"),
            (
                Err(GitError("Cannot read: broken".into())),
                "Cannot read: broken",
            ),
        ];
        for (result, text) in cases {
            let (mut model, target) = pending();
            arrive(&mut model, target, result);
            let out = diff_panel_text(&mut model);
            assert!(out.contains(text), "{out}");
            assert!(text.is_ascii());
        }

        let (mut model, _) = pending();
        crate::app::update(&mut model, Msg::WorkerLost);
        crate::app::update(&mut model, Msg::Enter);
        // Il marcatore della barra di stato dice "Git unavailable" anche lui:
        // si cerca il testo che solo il pannello scrive.
        assert!(diff_panel_text(&mut model).contains("diff not loaded"));

        let mut model = model_for_tests();
        crate::app::update(
            &mut model,
            Msg::Loaded {
                branches: Vec::new(),
                commits: Vec::new(),
                files: Vec::new(),
            },
        );
        assert!(diff_panel_text(&mut model).contains("No file selected"));

        // I file di un commit illeggibili: il Diff non dice "nessun file
        // selezionato", né ripete l'errore che Changes già mostra.
        let mut model = crate::app::model_without_changes_for_tests();
        crate::app::update(&mut model, Msg::Down);
        let id = match model.history().selected() {
            Some(crate::app::HistoryEntry::Commit(c)) => c.id.clone(),
            _ => panic!("History è su un commit"),
        };
        crate::app::update(
            &mut model,
            Msg::CommitFilesLoaded {
                id,
                result: Err(GitError("Cannot read: broken".into())),
            },
        );
        let out = diff_panel_text(&mut model);
        assert!(out.contains("could not be read"), "{out}");
        assert!(!out.contains("broken") && !out.contains("No file selected"));
    }

    /// Il diff a schermo è quello del file selezionato, non dell'intero
    /// repository: gli altri file della working copy non compaiono.
    #[test]
    fn the_diff_panel_shows_only_the_selected_file() {
        let mut model = model_for_tests();
        let files: Vec<String> = model
            .working_copy_files
            .items()
            .iter()
            .map(|f| f.path.clone())
            .collect();
        let out = diff_panel_text(&mut model);
        assert!(out.contains(&format!("a/{}", files[0])), "{out}");
        for other in &files[1..] {
            assert!(!out.contains(other.as_str()), "{other} in {out}");
        }
    }
}
