use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use unicode_truncate::{Alignment as UAlign, UnicodeTruncateStr};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, MenuState, Panel, MENUS},
    dialog::{Dialog, Kind},
    mock, theme,
    widgets::window::Window,
};

/// Sotto questa larghezza si mostra un solo pannello (quello con il focus).
const NARROW: u16 = 80;

/// Dimensioni minime sotto le quali si mostra un messaggio invece della UI.
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 6;

type Hints = &'static [(&'static str, &'static str)];

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
    draw_statusbar(f, status_area, hints(app));
    if let Some(m) = app.menu {
        draw_dropdown(f, menu_area, m);
    }
    if let Some(d) = &app.dialog {
        draw_dialog(f, d);
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
            let items: Vec<ListItem> = app
                .branch_list
                .iter()
                .map(|b| {
                    let mark = if b.current { '*' } else { ' ' };
                    ListItem::new(fit(&format!("{mark} {}", b.name), w))
                })
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

const MAIN_KEYS: Hints = &[
    ("Tab", "Panel"),
    ("↑↓", "Move"),
    ("Space", "Stage"),
    ("F10", "Menu"),
    ("Alt-X", "Quit"),
];
const MENU_KEYS: Hints = &[
    ("←→", "Switch"),
    ("↑↓", "Select"),
    ("Enter", "Activate"),
    ("Esc", "Close"),
];

fn hints(app: &App) -> Hints {
    match (&app.dialog, app.menu) {
        (Some(d), _) => d.hints(),
        (None, Some(_)) => MENU_KEYS,
        (None, None) => MAIN_KEYS,
    }
}

fn draw_statusbar(f: &mut Frame, area: Rect, keys: Hints) {
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

fn draw_dialog(f: &mut Frame, d: &Dialog) {
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
        .map(|l| l.width().max(1).div_ceil(inner_w))
        .sum();
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

    let [text_area, _, input_area, _, btn_area] = Layout::vertical([
        Constraint::Length(text_rows as u16),
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
    use crate::dialog::Action;
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

    #[test]
    fn confirm_dialog_renders_centered_with_buttons_and_hints() {
        let mut app = App::new();
        app.dialog = Some(Dialog::confirm(
            "Delete branch",
            "Delete 'x'?",
            Action::Info,
        ));
        let out = render(80, 24, &mut app);
        println!("{out}");
        assert!(out.contains("Delete branch"));
        assert!(out.contains("Delete 'x'?"));
        assert!(out.contains("[ Yes ]"));
        assert!(out.contains("  No  "));
        assert!(out.contains("Y/N"));
    }

    #[test]
    fn input_dialog_shows_typed_text_and_keeps_tail_when_long() {
        let mut app = App::new();
        let mut d = Dialog::input("New branch", "Branch name:", Action::NewBranch);
        d.input = "feature/login".into();
        app.dialog = Some(d);
        assert!(render(80, 24, &mut app).contains("feature/login_"));

        app.dialog.as_mut().unwrap().input = format!("{}END", "a".repeat(60));
        let out = render(80, 24, &mut app);
        println!("{out}");
        assert!(out.contains("END_"));
    }

    #[test]
    fn dialog_survives_small_terminals() {
        for (w, h) in [(80, 24), (40, 10), (30, 8), (20, 6), (20, 5), (3, 2)] {
            let mut app = App::new();
            let mut d = Dialog::input("New branch", "Branch name:", Action::NewBranch);
            d.input = "x".repeat(60);
            app.dialog = Some(d);
            render(w, h, &mut app);

            app.dialog = Some(Dialog::info(
                "About",
                "line one\nline two that is rather long",
            ));
            render(w, h, &mut app);
        }
    }

    #[test]
    fn long_message_wraps_inside_the_dialog() {
        let mut app = App::new();
        let text = "word ".repeat(40);
        app.dialog = Some(Dialog::info("About", text.trim()));
        let out = render(60, 24, &mut app);
        println!("{out}");
        assert!(out.contains("[ OK ]"));
    }
}
