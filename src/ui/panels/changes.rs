//
// src/ui/panels/changes.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Pannello "Changes": file nel working tree, con checkbox di staging.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{ListItem, ListState},
    Frame,
};

use crate::git::domain::{FileEntry, FileStatus};
use crate::ui::text::fit;
use crate::ui::theme;

const MARK_W: usize = 4;
const STATUS_W: usize = 2;

pub(crate) fn render(
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
            let mark = if e.staged { "[x]" } else { "[ ]" };
            let color = match e.status {
                FileStatus::Added => theme::ADDED,
                FileStatus::Deleted => theme::REMOVED,
                FileStatus::Modified | FileStatus::Untracked => theme::HASH,
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{mark} ")),
                Span::styled(format!("{} ", e.status.label()), color),
                Span::raw(fit(&e.path, path_w)),
            ]))
        })
        .collect();
    super::render_list(f, items, area, selection, state);
}
