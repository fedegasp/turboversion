//
// src/ui/panels/changes.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "Changes": i file della working copy, con la checkbox dello
//! stage (`[x]` in stage, `[~]` in stage solo in parte, `[ ]` no), oppure
//! i file toccati dal commit selezionato in History, senza checkbox perché
//! un commit non ha un indice. Quale delle due liste
//! arrivi lo decide il `Model`; qui si sceglie solo come appaiono. Quando
//! un commit non ha una lista da elencare (file in arrivo, lettura
//! fallita, git non più disponibile, commit vuoto) il pannello lo dice con
//! una riga di testo al posto della lista: un pannello vuoto non
//! distinguerebbe questi casi.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{ListItem, ListState},
    Frame,
};

use crate::app::ChangesNotice;
use crate::git::domain::{FileEntry, FileStatus, Staging};
use crate::ui::text::fit;
use crate::ui::theme;

const MARK_W: usize = 4;
const STATUS_W: usize = 2;

/// I file della working copy: ognuno con la sua checkbox di stage.
pub(crate) fn render_working_copy(
    f: &mut Frame,
    files: &[FileEntry],
    area: Rect,
    selection: Style,
    state: &mut ListState,
) {
    let path_w = (area.width as usize).saturating_sub(MARK_W + STATUS_W);
    let items: Vec<ListItem> = files
        .iter()
        .map(|e| {
            let mark = match e.staging {
                Staging::Staged => "[x]",
                // `~` e non un glifo come `▪` o `◐`: ASCII, quindi largo
                // una colonna su ogni terminale.
                Staging::Partly => "[~]",
                Staging::Unstaged => "[ ]",
            };
            let mut spans = vec![Span::raw(format!("{mark} "))];
            spans.extend(status_and_path(e, path_w));
            ListItem::new(Line::from(spans))
        })
        .collect();
    super::render_list(f, items, area, selection, state);
}

/// I file di un commit: senza checkbox, che lì direbbe "non in stage" di
/// un file per cui lo stage non esiste. Il percorso prende la larghezza
/// liberata.
pub(crate) fn render_commit(
    f: &mut Frame,
    files: &[FileEntry],
    area: Rect,
    selection: Style,
    state: &mut ListState,
) {
    if files.is_empty() {
        // Un commit può non toccare alcun file (`--allow-empty`).
        super::render_notice_text(f, NO_FILES, area);
        return;
    }
    let path_w = (area.width as usize).saturating_sub(STATUS_W);
    let items: Vec<ListItem> = files
        .iter()
        .map(|e| ListItem::new(Line::from(status_and_path(e, path_w).to_vec())))
        .collect();
    super::render_list(f, items, area, selection, state);
}

const NO_FILES: &str = "No files changed";

/// Perché, su un commit, non c'è una lista da mostrare.
pub(crate) fn render_notice(f: &mut Frame, notice: ChangesNotice<'_>, area: Rect) {
    let text = match notice {
        ChangesNotice::Loading => "Loading...",
        ChangesNotice::Failed(message) => message,
        ChangesNotice::GitUnavailable => "Git unavailable: files not loaded",
    };
    super::render_notice_text(f, text, area);
}

fn status_and_path(e: &FileEntry, path_w: usize) -> [Span<'static>; 2] {
    let color = match e.status {
        FileStatus::Added => theme::ADDED,
        FileStatus::Deleted => theme::REMOVED,
        FileStatus::Modified | FileStatus::Untracked => theme::HASH,
    };
    [
        Span::styled(format!("{} ", e.status.label()), color),
        Span::raw(fit(&e.path, path_w)),
    ]
}
