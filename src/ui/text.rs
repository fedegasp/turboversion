//
// src/ui/text.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Utility di formattazione testo condivise da più pannelli.

use std::borrow::Cow;

use unicode_truncate::{Alignment as UAlign, UnicodeTruncateStr};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Sostituisce i tab con spazi (allineati a colonne di 4) prima che il testo
/// arrivi a schermo: dati non fidati (messaggi di commit, percorsi, diff) non
/// devono rompere l'allineamento dei pannelli, e i tab letterali non vengono
/// comunque resi in modo leggibile dal terminale.
pub(crate) fn expand_tabs(s: &str) -> Cow<'_, str> {
    if !s.contains('\t') {
        return Cow::Borrowed(s);
    }
    const TAB_STOP: usize = 4;
    let mut out = String::with_capacity(s.len());
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == '\t' {
            let spaces = TAB_STOP - (col % TAB_STOP);
            out.extend(std::iter::repeat(' ').take(spaces));
            col += spaces;
        } else {
            out.push(ch);
            col += ch.width().unwrap_or(0);
        }
    }
    Cow::Owned(out)
}

/// Tronca (con ellissi) o riempie `s` alla larghezza visuale `width`,
/// tenendo conto di caratteri larghi/combinanti (non del solo conteggio byte/char).
pub(crate) fn fit(s: &str, width: usize) -> String {
    let s = expand_tabs(s);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_pads_and_truncates_by_display_width() {
        assert_eq!(fit("ab", 5), "ab   ");
        assert_eq!(fit("abcdef", 5), "abcd…");
        // wide (double-width) characters must be measured by display width, not char count
        assert_eq!(fit("你好", 5), "你好 ");
        assert_eq!(fit("你好吗", 5), "你好…");
    }

    #[test]
    fn tabs_in_diff_lines_are_expanded_instead_of_breaking_alignment() {
        assert_eq!(expand_tabs("a\tb"), "a   b");
        assert_eq!(expand_tabs("ab\tc"), "ab  c");
        assert_eq!(expand_tabs("no tabs here"), "no tabs here");
    }
}
