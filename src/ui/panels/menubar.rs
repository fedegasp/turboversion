//
// src/ui/panels/menubar.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Barra dei menu e relativo dropdown.

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{MenuState, MENUS};
use crate::ui::theme;
use crate::ui::widgets::window::Window;

pub(crate) fn render_menubar(f: &mut Frame, area: Rect, open: Option<MenuState>) {
    let mut spans = Vec::new();
    for (i, (name, _)) in MENUS.iter().enumerate() {
        let active = open.is_some_and(|m| m.menu == i);
        let (base, hot) = if active {
            (theme::menubar_selected(), theme::menubar_hotkey_selected())
        } else {
            (theme::menubar(), theme::menubar_hotkey())
        };
        let (first, rest) = name.split_at(1);
        spans.push(Span::styled(" ", base));
        spans.push(Span::styled(first, hot));
        spans.push(Span::styled(format!("{rest} "), base));
    }
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::menubar()),
        area,
    );
}

pub(crate) fn render_dropdown(f: &mut Frame, menubar: Rect, m: MenuState) {
    let x = menubar.x
        + MENUS[..m.menu]
            .iter()
            .map(|(n, _)| n.width() as u16 + 2)
            .sum::<u16>();
    let items = MENUS[m.menu].1;
    let w = items
        .iter()
        .map(|(label, _)| label.width())
        .max()
        .unwrap_or(0) as u16
        + 4;
    let h = items.len() as u16 + 2;
    let r = Rect::new(x, menubar.y + 1, w, h).intersection(f.area());

    Window::shadow(r, f.buffer_mut());
    f.render_widget(Clear, r);
    let block = Block::bordered().style(theme::menubar());
    let inner = block.inner(r);
    f.render_widget(block, r);

    let list = List::new(
        items
            .iter()
            .map(|(label, _)| ListItem::new(format!(" {label}"))),
    )
    .style(theme::menubar())
    .highlight_style(theme::menubar_selected());
    let mut st = ListState::default().with_selected(Some(m.item));
    f.render_stateful_widget(list, inner, &mut st);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{model_for_tests, Overlay};

    #[test]
    fn renders_dropdown() {
        let mut model = model_for_tests();
        model
            .overlays
            .push(Overlay::Menu(MenuState { menu: 1, item: 1 }));
        let out = crate::ui::render(80, 24, &mut model);
        println!("{out}");
        assert!(out.contains("Checkout"));
    }
}
