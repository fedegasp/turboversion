use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use unicode_truncate::{Alignment as UAlign, UnicodeTruncateStr};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, MenuState, Panel, MENUS},
    mock, theme,
    widgets::window::Window,
};

/// Sotto questa larghezza si mostra un solo pannello (quello con il focus).
const NARROW: u16 = 80;

/// Dimensioni minime sotto le quali si mostra un messaggio invece della UI.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 6;

/// Tronca (con ellissi) o riempie `s` alla larghezza visuale `width`,
/// tenendo conto di caratteri larghi/combinanti (non del solo conteggio byte/char).
fn fit(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if s.width() <= width {
        return s.unicode_pad(width, UAlign::Left, true).into_owned();
    }
    let (head, _) = s.unicode_truncate(width.saturating_sub(1));
    format!("{head}…")
        .unicode_pad(width, UAlign::Left, true)
        .into_owned()
}

pub fn draw(f: &mut Frame, app: &mut App) {
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

    let panels = layout_panels(body, app.focus, area.width);

    for (p, r) in &panels {
        draw_panel(f, app, *p, *r);
    }

    draw_menubar(f, menu_area, app.menu);
    draw_statusbar(f, status_area, app.menu.is_some());
    if let Some(m) = app.menu {
        draw_dropdown(f, menu_area, m);
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
            let w = inner.width as usize;
            let items: Vec<ListItem> = mock::BRANCHES
                .iter()
                .map(|b| ListItem::new(fit(b, w)))
                .collect();
            render_list(f, items, inner, sel, &mut app.branches);
        }
        Panel::History => {
            const GRAPH_W: usize = 4;
            const HASH_W: usize = 8;
            const AUTHOR_W: usize = 12;
            let avail = (inner.width as usize).saturating_sub(GRAPH_W + HASH_W);
            let author_w = AUTHOR_W.min(avail);
            let msg_w = avail.saturating_sub(author_w);
            let items: Vec<ListItem> = mock::COMMITS
                .iter()
                .map(|c| {
                    ListItem::new(Line::from(vec![
                        Span::styled(fit(c.graph, GRAPH_W), theme::GRAPH),
                        Span::styled(fit(&format!("{} ", c.hash), HASH_W), theme::HASH),
                        Span::raw(fit(c.msg, msg_w)),
                        Span::styled(fit(c.author, author_w), theme::DIM),
                    ]))
                })
                .collect();
            render_list(f, items, inner, sel, &mut app.history);
        }
        Panel::Changes => {
            const MARK_W: usize = 4;
            const STATUS_W: usize = 2;
            let path_w = (inner.width as usize).saturating_sub(MARK_W + STATUS_W);
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
                        Span::raw(fit(e.path, path_w)),
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
                    .wrap(Wrap { trim: false })
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
        List::new(items)
            .style(theme::window())
            .highlight_style(selection),
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
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::menubar()),
        area,
    );
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

fn draw_statusbar(f: &mut Frame, area: Rect, menu_open: bool) {
    const MAIN_KEYS: &[(&str, &str)] = &[
        ("Tab", "Panel"),
        ("↑↓", "Move"),
        ("Space", "Stage"),
        ("F10", "Menu"),
        ("Alt-X", "Quit"),
    ];
    const MENU_KEYS: &[(&str, &str)] = &[
        ("←→", "Switch"),
        ("↑↓", "Select"),
        ("Enter", "Activate"),
        ("Esc", "Close"),
    ];
    let keys = if menu_open { MENU_KEYS } else { MAIN_KEYS };
    let mut spans = Vec::new();
    for (k, label) in keys {
        spans.push(Span::styled(format!(" {k}"), theme::menubar_hotkey()));
        spans.push(Span::styled(format!(" {label} "), theme::menubar()));
    }
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::menubar()),
        area,
    );
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
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
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
    fn below_minimum_size_shows_message_instead_of_garbled_panels() {
        let out = render(30, 4, &mut App::new());
        println!("{out}");
        assert!(out.contains("too small"));
    }

    #[test]
    fn statusbar_hints_change_when_menu_opens() {
        let mut app = App::new();
        let closed = render(80, 24, &mut app);
        assert!(closed.contains("Menu"));
        assert!(!closed.contains("Close"));

        app.menu = Some(MenuState { menu: 0, item: 0 });
        let open = render(80, 24, &mut app);
        assert!(open.contains("Close"));
    }

    #[test]
    fn fit_pads_and_truncates_by_display_width() {
        assert_eq!(fit("ab", 5), "ab   ");
        assert_eq!(fit("abcdef", 5), "abcd…");
        // wide (double-width) characters must be measured by display width, not char count
        assert_eq!(fit("你好", 5), "你好 ");
        assert_eq!(fit("你好吗", 5), "你好…");
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
