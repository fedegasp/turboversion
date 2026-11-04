//
// src/ui/panels/branches.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Elenco di riferimenti, con `*` sul ramo corrente. Serve sia il pannello
//! "Branches" (rami locali) sia "Remotes" (rami remote-tracking): la lista e
//! la selezione da disegnare arrivano come parametri, così i due pannelli
//! condividono una sola resa. Nessun remoto è il ramo corrente, quindi il
//! marcatore riguarda di fatto i soli locali.

use ratatui::{layout::Rect, style::Style, widgets::ListItem, widgets::ListState, Frame};

use crate::git::domain::Branch;
use crate::ui::text::fit;

pub(crate) fn render(
    f: &mut Frame,
    branches: &[Branch],
    area: Rect,
    selection: Style,
    state: &mut ListState,
) {
    let width = area.width as usize;
    let items: Vec<ListItem> = branches
        .iter()
        .map(|b| {
            let mark = if b.current { '*' } else { ' ' };
            ListItem::new(fit(&format!("{mark} {}", b.name), width))
        })
        .collect();
    super::render_list(f, items, area, selection, state);
}
