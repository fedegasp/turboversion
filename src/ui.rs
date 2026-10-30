use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::{
    app::{App, MenuState, Panel, MENUS},
    mock, theme,
    widgets::window::Window,
};

/// Sotto questa larghezza si mostra un solo pannello (quello con il focus).
const NARROW: u16 = 80;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let [menu_area, body, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    let panels = layout_panels(body, app.focus, area.width);

    for (p, r) in &panels {
        draw_panel(f, app, *p, *r);
    }

    draw_menubar(f, menu_area, app.menu);
    draw_statusbar(f, status_area);
    if let Some(m) = app.menu {
        draw_dropdown(f, menu_area, m);
    }
}

fn layout_panels(body: Rect, focus: Panel, term_width: u16) -> Vec<(Panel, Rect)> {
    if term_width < NARROW {
        return vec![(focus, body)];
    }
    let [left, right] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(body);
    let [hist, bottom] =
        Layout::vertical([Constraint::Percentage(45), Constraint::Min(0)]).areas(right);
    let [chg, diff] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Min(0)]).areas(bottom);
    vec![
        (Panel::Branches, left),
        (Panel::History, hist),
        (Panel::Changes, chg),
        (Panel::Diff, diff),
    ]
}

fn draw_panel(f: &mut Frame, app: &mut App, panel: Panel, area: Rect) {
    let focused = app.focus == panel;
    f.render_widget(Window::new(panel.title(), focused), area);
    let inner = Window::inner(area);
    let sel = theme::selection(focused);

    match panel {
        Panel::Branches => {
            let items: Vec<ListItem> = mock::BRANCHES.iter().map(|b| ListItem::new(*b)).collect();
            render_list(f, items, inner, sel, &mut app.branches);
        }
        Panel::History => {
            let items: Vec<ListItem> = mock::COMMITS
                .iter()
                .map(|c| {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{:<4}", c.graph), theme::GRAPH),
                        Span::styled(format!("{} ", c.hash), theme::HASH),
                        Span::raw(format!("{:<24}", c.msg)),
                        Span::styled(c.author, theme::DIM),
                    ]))
                })
                .collect();
            render_list(f, items, inner, sel, &mut app.history);
        }
        Panel::Changes => {
            let items: Vec<ListItem> = app
                .files
                .iter()
                .map(|e| {
                    let mark = if e.staged { "[x]" } else { "[ ]" };
                    let color = match e.status {
                        mock::FileStatus::Added => theme::ADDED,
                        mock::FileStatus::Deleted => theme::REMOVED,
                        mock::FileStatus::Modified | mock::FileStatus::Untracked => theme::HASH,
                    };
                    ListItem::new(Line::from(vec![
                        Span::raw(format!("{mark} ")),
                        Span::styled(format!("{} ", e.status.label()), color),
                        Span::raw(e.path),
                    ]))
                })
                .collect();
            render_list(f, items, inner, sel, &mut app.changes);
        }
        Panel::Diff => {
            let lines: Vec<Line> = mock::DIFF
                .iter()
                .map(|l| {
                    let color = match l.chars().next() {
                        Some('+') => Some(theme::ADDED),
                        Some('-') => Some(theme::REMOVED),
                        Some('@') => Some(theme::HUNK),
                        _ => None,
                    };
                    match color {
                        Some(c) => Line::styled(*l, c),
                        None => Line::raw(*l),
                    }
                })
                .collect();
            f.render_widget(
                Paragraph::new(lines)
                    .style(theme::window())
                    .scroll((app.diff_scroll, 0)),
                inner,
            );
        }
    }
}

fn render_list(
    f: &mut Frame,
    items: Vec<ListItem>,
    area: Rect,
    selection: ratatui::style::Style,
    state: &mut ListState,
) {
    f.render_stateful_widget(
        List::new(items).style(theme::window()).highlight_style(selection),
        area,
        state,
    );
}

fn draw_menubar(f: &mut Frame, area: Rect, open: Option<MenuState>) {
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
    f.render_widget(Paragraph::new(Line::from(spans)).style(theme::menubar()), area);
}

fn draw_dropdown(f: &mut Frame, menubar: Rect, m: MenuState) {
    let x = menubar.x
        + MENUS[..m.menu]
            .iter()
            .map(|(n, _)| n.chars().count() as u16 + 2)
            .sum::<u16>();
    let items = MENUS[m.menu].1;
    let w = items.iter().map(|s| s.chars().count()).max().unwrap_or(0) as u16 + 4;
    let h = items.len() as u16 + 2;
    let r = Rect::new(x, menubar.y + 1, w, h).intersection(f.area());

    Window::shadow(r, f.buffer_mut());
    f.render_widget(Clear, r);
    let block = Block::bordered().style(theme::menubar());
    let inner = block.inner(r);
    f.render_widget(block, r);

    let list = List::new(items.iter().map(|s| ListItem::new(format!(" {s}"))))
        .style(theme::menubar())
        .highlight_style(theme::menubar_selected());
    let mut st = ListState::default().with_selected(Some(m.item));
    f.render_stateful_widget(list, inner, &mut st);
}

fn draw_statusbar(f: &mut Frame, area: Rect) {
    let keys = [
        ("Tab", "Panel"),
        ("↑↓", "Move"),
        ("Space", "Stage"),
        ("F10", "Menu"),
        ("Alt-X", "Quit"),
    ];
    let mut spans = Vec::new();
    for (k, label) in keys {
        spans.push(Span::styled(format!(" {k}"), theme::menubar_hotkey()));
        spans.push(Span::styled(format!(" {label} "), theme::menubar()));
    }
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(Line::from(spans)).style(theme::menubar()), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render(w: u16, h: u16, app: &mut App) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_80x24() {
        let out = render(80, 24, &mut App::new());
        println!("{out}");
        assert!(out.contains("History"));
    }

    #[test]
    fn renders_narrow_and_tiny_without_panic() {
        render(60, 20, &mut App::new());
        render(20, 5, &mut App::new());
        render(3, 2, &mut App::new());
    }

    #[test]
    fn renders_dropdown() {
        let mut app = App::new();
        app.menu = Some(MenuState { menu: 1, item: 1 });
        let out = render(80, 24, &mut app);
        println!("{out}");
        assert!(out.contains("Checkout"));
    }
}
