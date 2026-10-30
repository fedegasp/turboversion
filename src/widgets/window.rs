use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{Block, BorderType, Clear, Widget},
};

use crate::theme;

/// Finestra stile Turbo Vision: bordo doppio se attiva, singolo se inattiva,
/// titolo centrato sul bordo superiore.
pub struct Window<'a> {
    title: &'a str,
    active: bool,
}

impl<'a> Window<'a> {
    pub fn new(title: &'a str, active: bool) -> Self {
        Self { title, active }
    }

    /// Area utile dentro il bordo.
    pub fn inner(area: Rect) -> Rect {
        Block::bordered().inner(area)
    }

    /// Ombra portata (2 colonne a destra, 1 riga sotto), clippata al buffer.
    /// Da usare solo per elementi flottanti (dropdown, dialog): va disegnata
    /// prima dell'elemento, che poi copre la propria area.
    pub fn shadow(area: Rect, buf: &mut Buffer) {
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
            BorderType::Double
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
