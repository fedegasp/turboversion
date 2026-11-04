//
// src/ui/panels/history.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "History": log dei commit con grafo, hash, messaggio e autore,
//! preceduto — quando c'è — dalla riga dello stato non committato.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{ListItem, ListState},
    Frame,
};

use crate::app::HistoryEntry;
use crate::git::domain::Commit;
use crate::ui::text::fit;
use crate::ui::theme;

const GRAPH_W: usize = 4;
const HASH_W: usize = 8;
const AUTHOR_W: usize = 12;

const WORKING_COPY_TEXT: &str = "Uncommitted changes";

pub(crate) fn render(
    f: &mut Frame,
    entries: &[HistoryEntry],
    area: Rect,
    selection: Style,
    state: &mut ListState,
) {
    let avail = (area.width as usize).saturating_sub(GRAPH_W + HASH_W);
    let author_w = AUTHOR_W.min(avail);
    let msg_w = avail.saturating_sub(author_w);
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| match entry {
            HistoryEntry::WorkingCopy => working_copy_row(msg_w, author_w),
            HistoryEntry::Commit(c) => commit_row(c, msg_w, author_w),
        })
        .collect();
    super::render_list(f, items, area, selection, state);
}

/// La riga della working copy. Occupa le stesse colonne di un commit, ma
/// grafo, hash e autore restano vuoti: quelle modifiche non sono un nodo
/// del grafo, non hanno un hash che le identifichi e non hanno ancora un
/// autore. È quell'assenza — non il solo colore — a dire che non sono
/// state committate.
///
/// Nessun glifo nella colonna del grafo, nemmeno un segnaposto: i
/// candidati (`○`, `·`, i box-drawing) sono "East Asian ambiguous" e certi
/// terminali li rendono a doppia larghezza, sfalsando di una cella le
/// colonne della sola riga non committata. Un carattere ASCII non avrebbe
/// quel problema ma si confonderebbe con il grafo del backend reale, che
/// disegna i commit con `*`.
fn working_copy_row(msg_w: usize, author_w: usize) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::raw(fit("", GRAPH_W + HASH_W)),
        Span::styled(fit(WORKING_COPY_TEXT, msg_w), theme::uncommitted()),
        Span::raw(fit("", author_w)),
    ]))
}

fn commit_row(c: &Commit, msg_w: usize, author_w: usize) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(fit(&c.graph, GRAPH_W), theme::GRAPH),
        Span::styled(fit(&format!("{} ", c.hash), HASH_W), theme::HASH),
        Span::raw(fit(&c.msg, msg_w)),
        Span::styled(fit(&c.author, author_w), theme::DIM),
    ]))
}

#[cfg(test)]
mod tests {
    use ratatui::{
        buffer::Buffer,
        style::{Color, Modifier},
    };

    use super::{GRAPH_W, HASH_W, WORKING_COPY_TEXT};
    use crate::app::{model_for_tests, model_without_changes_for_tests, update, Panel};
    use crate::msg::Msg;
    use crate::ui::{render, render_buffer, theme};

    /// Coordinate della prima cella di `needle` nel frame.
    fn find(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        let area = *buf.area();
        let text: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
        (0..area.height).find_map(|y| {
            let row: Vec<String> = (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            row.windows(text.len())
                .position(|w| w == text.as_slice())
                .map(|x| (x as u16, y))
        })
    }

    /// La riga dello stato non committato sta in cima e, a differenza di
    /// un commit, lascia vuote le colonne del grafo e dell'hash: non è una
    /// riga del log, e si vede anche senza colori.
    #[test]
    fn the_uncommitted_row_is_drawn_on_top_without_a_hash() {
        let out = render(100, 30, &mut model_for_tests());
        println!("{out}");

        let rows: Vec<Vec<char>> = out.lines().map(|l| l.chars().collect()).collect();
        let top = rows
            .iter()
            .position(|r| r.iter().collect::<String>().contains(WORKING_COPY_TEXT))
            .expect("riga non committata assente");
        let first_commit = rows
            .iter()
            .position(|r| r.iter().collect::<String>().contains("a1b2c3d"))
            .expect("primo commit assente");
        assert!(top < first_commit, "deve stare sopra ai commit: {out}");

        let text: Vec<char> = WORKING_COPY_TEXT.chars().collect();
        let start = rows[top]
            .windows(text.len())
            .position(|w| w == text)
            .expect("appena trovata nella riga");
        assert!(
            start >= GRAPH_W + HASH_W,
            "il testo deve venire dopo le colonne di grafo e hash"
        );
        let prefix: String = rows[top][start - (GRAPH_W + HASH_W)..start]
            .iter()
            .collect();
        assert!(
            prefix.trim().is_empty(),
            "grafo e hash devono restare vuoti: {prefix:?}"
        );
    }

    #[test]
    fn a_clean_working_copy_shows_the_log_alone() {
        let out = render(100, 30, &mut model_without_changes_for_tests());
        println!("{out}");
        assert!(!out.contains(WORKING_COPY_TEXT), "{out}");
        assert!(out.contains("a1b2c3d"), "i commit restano: {out}");
    }

    /// Lo stile della riga non committata, letto dalle celle disegnate:
    /// senza questo controllo, togliere lo stile dal testo non romperebbe
    /// alcun test.
    #[test]
    fn the_uncommitted_row_is_styled_apart_from_the_log() {
        let mut model = model_for_tests();
        model.focus = Panel::History;
        // Sposta la selezione sul primo commit: l'evidenziazione della
        // riga selezionata coprirebbe lo stile che si vuole osservare.
        update(&mut model, Msg::Down);

        let buf = render_buffer(100, 30, &mut model);
        let (x, y) = find(&buf, WORKING_COPY_TEXT).expect("riga non committata assente");
        let row = &buf[(x, y)];
        assert_eq!(row.fg, Color::White);
        assert!(row.modifier.contains(Modifier::BOLD));

        let (cx, cy) = find(&buf, "Merge feature/menu").expect("primo commit assente");
        let commit = &buf[(cx, cy)];
        assert_ne!(
            (row.fg, row.modifier),
            (commit.fg, commit.modifier),
            "la riga non committata non si deve confondere con un commit"
        );
    }

    /// Da selezionata la riga prende i colori della selezione, ma resta
    /// distinguibile: ratatui applica `highlight_style` come patch, quindi
    /// il grassetto sopravvive (e le colonne vuote restano vuote).
    #[test]
    fn the_uncommitted_row_stays_distinguishable_when_selected() {
        let mut model = model_for_tests();
        model.focus = Panel::History;

        let buf = render_buffer(100, 30, &mut model);
        let (x, y) = find(&buf, WORKING_COPY_TEXT).expect("riga non committata assente");
        let cell = &buf[(x, y)];
        assert_eq!(
            Some(cell.bg),
            theme::selection(true).bg,
            "è la riga selezionata"
        );
        assert!(cell.modifier.contains(Modifier::BOLD));
    }
}
