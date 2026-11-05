//
// src/app/panel.rs
// TurboVersion
//
// 2026-11-05 Federico Gasperini
//

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Panel {
    Branches,
    Remotes,
    History,
    Changes,
    Diff,
}

impl Panel {
    const ALL: [Panel; 5] = [
        Panel::Branches,
        Panel::Remotes,
        Panel::History,
        Panel::Changes,
        Panel::Diff,
    ];

    fn index(self) -> usize {
        self as usize
    }
    pub(crate) fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }
    pub(crate) fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
    pub(crate) fn title(self) -> &'static str {
        match self {
            Panel::Branches => "Branches",
            Panel::Remotes => "Remotes",
            Panel::History => "History",
            Panel::Changes => "Changes",
            Panel::Diff => "Diff",
        }
    }
}
