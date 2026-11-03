//
// src/widgets/window.rs
// TurboVersion
//
// 2026-10-30 Federico Gasperini
//

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{Block, BorderType, Clear, Widget},
};

use crate::theme;

/// Finestra stile Turbo Vision: bordo spesso (`Thick`) se attiva, sottile
/// (`Plain`) se inattiva — il bordo doppio (`Double`) è riservato ai dialog,
/// vedi `ui::draw_dialog` — titolo centrato sul bordo superiore.
pub(crate) struct Window<'a> {
    title: &'a str,
    active: bool,
}

impl<'a> Window<'a> {
    pub(crate) fn new(title: &'a str, active: bool) -> Self {
        Self { title, active }
    }

    /// Area utile dentro il bordo.
    pub(crate) fn inner(area: Rect) -> Rect {
        Block::bordered().inner(area)
    }

    /// Ombra portata (2 colonne a destra, 1 riga sotto), clippata al buffer.
    /// Da usare solo per elementi flottanti (dropdown, dialog): va disegnata
    /// prima dell'elemento, che poi copre la propria area.
    pub(crate) fn shadow(area: Rect, buf: &mut Buffer) {
        let bounds = *buf.area();
        let (right, bottom) = (area.right(), area.bottom());
        let mut darken = |x: u16, y: u16| {
            if x < bounds.right() && y < bounds.bottom() {
                buf[(x, y)].set_style(theme::shadow());
            }
        };
        for y in (area.y + 1)..=bottom {
            for x in right..right + 2 {
                darken(x, y);
            }
        }
        for x in (area.x + 2)..right {
            darken(x, bottom);
        }
    }
}

impl Widget for Window<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let border_type = if self.active {
            BorderType::Thick
        } else {
            BorderType::Plain
        };
        Block::bordered()
            .border_type(border_type)
            .border_style(theme::border(self.active))
            .style(theme::window())
            .title(Line::styled(format!(" {} ", self.title), theme::title(self.active)).centered())
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_is_clipped_when_the_window_touches_the_buffer_edge() {
        let buf_area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(buf_area);
        // La finestra sborda sia a destra (right=9, ultima colonna valida)
        // sia in basso (bottom=4, ultima riga valida) rispetto al buffer:
        // le celle ancora dentro i limiti devono comunque essere scurite,
        // quelle fuori vanno silenziosamente ignorate invece di far
        // andare in panico `Buffer::index_of`.
        let window = Rect::new(5, 1, 4, 3);
        Window::shadow(window, &mut buf);
        assert_eq!(buf[(9, 2)].fg, theme::shadow().fg.unwrap());
        assert_eq!(buf[(9, 2)].bg, theme::shadow().bg.unwrap());
        assert_eq!(buf[(7, 4)].fg, theme::shadow().fg.unwrap());
        assert_eq!(buf[(7, 4)].bg, theme::shadow().bg.unwrap());

        // Una finestra completamente fuori dal bordo destro/inferiore non
        // deve produrre alcun indice fuori dai limiti del buffer.
        let fully_outside = Rect::new(20, 20, 5, 5);
        Window::shadow(fully_outside, &mut buf);
    }
}
