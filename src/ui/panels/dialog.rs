//
// src/ui/panels/dialog.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Rendering dei dialog modali (vedi `dialog::Dialog` per lo stato).

use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
    Frame,
};
use unicode_truncate::UnicodeTruncateStr;
use unicode_width::UnicodeWidthStr;

use crate::dialog::{Dialog, Kind};
use crate::ui::text::fit;
use crate::ui::theme;
use crate::ui::widgets::window::Window;

/// Numero di righe che `Paragraph::wrap` userebbe per `line` su `width`
/// colonne, spezzando sugli spazi (come il wrap a parole di ratatui) invece
/// di dividere `width` per la larghezza totale della riga: una stima basata
/// solo sulla divisione sottostima lo spazio quando il wrap va a capo prima
/// della fine esatta di una colonna.
fn wrapped_row_count(line: &str, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let mut rows = 1usize;
    let mut col = 0usize;
    for word in line.split_whitespace() {
        let word_w = word.width();
        if word_w > width {
            // parola più lunga della riga: va comunque spezzata su più righe.
            if col > 0 {
                rows += 1;
            }
            rows += word_w / width;
            col = word_w % width;
            continue;
        }
        let needed = if col == 0 { word_w } else { col + 1 + word_w };
        if needed > width {
            rows += 1;
            col = word_w;
        } else {
            col = needed;
        }
    }
    rows
}

pub(crate) fn render_dialog(f: &mut Frame, d: &Dialog) {
    const PAD_X: usize = 2;
    const MIN_W: usize = 30;
    const INPUT_W: usize = 32;
    let area = f.area();
    let is_input = d.kind == Kind::Input;
    let buttons = d.buttons();

    let buttons_w = buttons.iter().map(|b| b.width() + 4).sum::<usize>() + 2 * (buttons.len() - 1);
    let text_w = d.text.lines().map(|l| l.width()).max().unwrap_or(0);
    let content_w = text_w
        .max(buttons_w)
        .max(d.title.width() + 2)
        .max(if is_input { INPUT_W } else { 0 });
    let w = (content_w + 2 + 2 * PAD_X)
        .max(MIN_W)
        .min(area.width.saturating_sub(2) as usize);
    let inner_w = w.saturating_sub(2 + 2 * PAD_X).max(1);
    let text_rows: usize = d
        .text
        .lines()
        .map(|l| wrapped_row_count(l, inner_w))
        .sum::<usize>()
        .max(1);
    let input_rows = if is_input { 2 } else { 0 };
    // bordi + margine verticale + testo + (input) + riga vuota + pulsanti
    let h = (text_rows + input_rows + 6).min(area.height.saturating_sub(2) as usize);

    let (w, h) = (w as u16, h as u16);
    let r = Rect::new(
        area.x + (area.width - w.min(area.width)) / 2,
        area.y + (area.height - h.min(area.height)) / 2,
        w.min(area.width),
        h.min(area.height),
    );

    Window::shadow(r, f.buffer_mut());
    f.render_widget(Clear, r);
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(theme::dialog_frame())
        .style(theme::dialog())
        .title(Line::styled(format!(" {} ", d.title), theme::dialog_frame()).centered());
    let inner = block.inner(r).inner(Margin::new(PAD_X as u16, 1));
    f.render_widget(block, r);

    // Il testo (Min) è il primo a restringersi se lo spazio manca; pulsanti
    // e separatori (Length) restano quindi visibili il più a lungo possibile.
    let [text_area, _, input_area, _, btn_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(u16::from(is_input)),
        Constraint::Length(u16::from(is_input)),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    f.render_widget(
        Paragraph::new(d.text.as_str())
            .style(theme::dialog())
            .wrap(Wrap { trim: false })
            .alignment(if is_input {
                Alignment::Left
            } else {
                Alignment::Center
            }),
        text_area,
    );

    if is_input {
        let width = input_area.width as usize;
        let (tail, _) = d.input.unicode_truncate_start(width.saturating_sub(1));
        f.render_widget(
            Paragraph::new(fit(&format!("{tail}_"), width)).style(theme::window()),
            input_area,
        );
    }

    let mut spans = Vec::new();
    for (i, b) in buttons.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme::dialog()));
        }
        let focused = i == d.button;
        let label = if focused {
            format!("[ {b} ]")
        } else {
            format!("  {b}  ")
        };
        spans.push(Span::styled(label, theme::button(focused)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans))
            .style(theme::dialog())
            .alignment(Alignment::Center),
        btn_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{model_for_tests, Overlay};
    use crate::dialog::Action;

    #[test]
    fn confirm_dialog_renders_centered_with_buttons_and_hints() {
        let mut model = model_for_tests();
        model.overlays.push(Overlay::Dialog(Dialog::confirm(
            "Delete branch",
            "Delete 'x'?",
            Action::Info,
        )));
        let out = crate::ui::render(80, 24, &mut model);
        println!("{out}");
        assert!(out.contains("Delete branch"));
        assert!(out.contains("Delete 'x'?"));
        assert!(out.contains("[ Yes ]"));
        assert!(out.contains("  No  "));
        assert!(out.contains("Y/N"));
    }

    #[test]
    fn input_dialog_shows_typed_text_and_keeps_tail_when_long() {
        let mut model = model_for_tests();
        let mut d = Dialog::input("New branch", "Branch name:", Action::NewBranch);
        d.input = "feature/login".into();
        model.overlays.push(Overlay::Dialog(d));
        assert!(crate::ui::render(80, 24, &mut model).contains("feature/login_"));

        model.dialog_mut().unwrap().input = format!("{}END", "a".repeat(60));
        let out = crate::ui::render(80, 24, &mut model);
        println!("{out}");
        assert!(out.contains("END_"));
    }

    #[test]
    fn dialog_survives_small_terminals() {
        for (w, h) in [(80, 24), (40, 10), (30, 8), (20, 6), (20, 5), (3, 2)] {
            let mut model = model_for_tests();
            let mut d = Dialog::input("New branch", "Branch name:", Action::NewBranch);
            d.input = "x".repeat(60);
            model.overlays.push(Overlay::Dialog(d));
            crate::ui::render(w, h, &mut model);

            model.overlays.pop();
            model.overlays.push(Overlay::Dialog(Dialog::info(
                "About",
                "line one\nline two that is rather long",
            )));
            crate::ui::render(w, h, &mut model);
        }
    }

    #[test]
    fn long_message_wraps_inside_the_dialog() {
        let mut model = model_for_tests();
        let text = "word ".repeat(40);
        model
            .overlays
            .push(Overlay::Dialog(Dialog::info("About", text.trim())));
        let out = crate::ui::render(60, 24, &mut model);
        println!("{out}");
        assert!(out.contains("[ OK ]"));
    }
}
