//! Palette e stili in stile Turbo C / Turbo Vision (16 colori ANSI).

use ratatui::style::{Color, Modifier, Style};

/// Contenuto delle finestre: testo giallo su blu.
pub fn window() -> Style {
    Style::new().fg(Color::LightYellow).bg(Color::Blue)
}

pub fn border(active: bool) -> Style {
    let s = Style::new().fg(Color::White).bg(Color::Blue);
    if active {
        s.add_modifier(Modifier::BOLD)
    } else {
        s
    }
}

pub fn title(active: bool) -> Style {
    if active {
        Style::new()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Gray).bg(Color::Blue)
    }
}

/// Riga selezionata in una lista.
pub fn selection(focused: bool) -> Style {
    if focused {
        Style::new().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::new().fg(Color::White).bg(Color::DarkGray)
    }
}

/// Barra dei menu, dropdown e status bar: nero su grigio chiaro.
pub fn menubar() -> Style {
    Style::new().fg(Color::Black).bg(Color::Gray)
}

pub fn menubar_hotkey() -> Style {
    Style::new().fg(Color::Red).bg(Color::Gray)
}

pub fn menubar_selected() -> Style {
    Style::new().fg(Color::Black).bg(Color::Green)
}

pub fn menubar_hotkey_selected() -> Style {
    Style::new().fg(Color::Red).bg(Color::Green)
}

/// Dialog modali: nero su grigio chiaro, come i box di Turbo Vision.
pub fn dialog() -> Style {
    Style::new().fg(Color::Black).bg(Color::Gray)
}

pub fn dialog_frame() -> Style {
    dialog().add_modifier(Modifier::BOLD)
}

/// Pulsanti dei dialog; il focus è dato anche dalle parentesi `[ ]`, non solo dal colore.
pub fn button(focused: bool) -> Style {
    if focused {
        Style::new()
            .fg(Color::White)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Black).bg(Color::Green)
    }
}

/// Ombra portata: le celle sotto restano, ma scurite.
pub fn shadow() -> Style {
    Style::new().fg(Color::DarkGray).bg(Color::Black)
}

// Colori semantici usati nei contenuti.
pub const GRAPH: Color = Color::LightGreen;
pub const HASH: Color = Color::LightCyan;
pub const DIM: Color = Color::Gray;
pub const ADDED: Color = Color::LightGreen;
pub const REMOVED: Color = Color::LightRed;
pub const HUNK: Color = Color::LightMagenta;
