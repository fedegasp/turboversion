# TurboVersion

*Turbo Vision meets version control.*

A terminal-based Git client written in Rust, styled after the blue/yellow
**Turbo Vision / Turbo C** look of early-90s Borland IDEs, with the panel
layout and workflow of **SourceTree**: branches, commit history, working-tree
changes and diff, all visible at once, navigable entirely from the keyboard.

> **Status: early prototype.** The UI, layout, menu system and keybindings
> are implemented and working, but the app does not talk to a real Git
> repository yet — all panels are populated with mock data
> (see [`src/mock.rs`](src/mock.rs)) so the interface can be built and tuned
> independently of the Git backend.

## Preview

```
┌File─Branch─Commit─View─Help──────────────────────────────────────────────┐
╔ Branches ════╗╔ History ════════════════════════════════════════════════╗
║ * main       ║║ ●─╮ a1b2c3d Merge feature/menu              Mario       ║
║   develop    ║║ │ ● d4e5f6a Add dropdown menu                Luca       ║
║   feature/.. ║║ │ ● 9f8e7d6 Add menubar widget                Luca       ║
║   origin/... ║║ ●─╯ c0ffee1 Fix status bar                   Mario       ║
║   v0.1.0     ║╚══════════════════════════════════════════════════════════╝
║              ║┌ Changes ══════════┐┌ Diff ═══════════════════════════════┐
║              ║│ [x] M src/main.rs ││ diff --git a/src/theme.rs ...       │
║              ║│ [ ] M src/theme.rs││ @@ -1,6 +1,9 @@                     │
╚══════════════╝│ [ ] A src/ui.rs   ││-pub fn window() -> Style {          │
                 │ [ ] D README.md  ││+pub fn window() -> Style {          │
                 └────────────────────┘└─────────────────────────────────────┘
 Tab Panel  ↑↓ Move  Space Stage  F10 Menu  Alt-X Quit
```

Double borders mark the focused panel; the active window's title is
highlighted; a drop shadow is cast behind floating elements such as the
dropdown menus — all classic Turbo Vision touches.

## Features

- **Turbo Vision aesthetic**: 16-color ANSI palette, yellow-on-blue windows,
  double border when focused / single border otherwise, drop shadows on
  floating panels.
- **SourceTree-style layout**: Branches, History, Changes and Diff panels
  on screen together, automatically collapsing to a single focused panel on
  narrow terminals (< 80 columns).
- **Keyboard-driven UX**: panel switching, list navigation, file staging
  toggle, diff scrolling.
- **Turbo Vision-style menu bar**: `F10` or `Alt+<letter>` opens a dropdown
  menu, navigable with arrow keys.
- Built on [`ratatui`](https://ratatui.rs) with a custom `Window` widget
  implementing the bordered/shadowed Turbo Vision chrome.

## Roadmap

- [ ] Replace `src/mock.rs` with real repository data (via `git2` or shelling
      out to `git`): branches, commit log, working-tree status, diffs.
- [ ] Wire up the menu actions (currently only `File > Quit` works).
- [ ] Stage/unstage and commit from the UI.
- [ ] Branch checkout/creation/deletion.
- [ ] Repository picker (`File > Open repository...`, `Clone...`).

## Getting started

### Requirements

- Rust (2021 edition toolchain), installable via [rustup](https://rustup.rs).
- A terminal emulator with reasonable ANSI color support.

### Build & run

```sh
cargo run
```

### Test

```sh
cargo test
```

## Keybindings

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Move focus to the next / previous panel |
| `↑` / `k`, `↓` / `j` | Move selection (or scroll the diff, when focused) |
| `Space` | Toggle stage/unstage for the selected file (Changes panel) |
| `F10` | Open the menu bar |
| `Alt` + letter | Open the corresponding menu directly |
| `←` / `→` (menu open) | Switch menu |
| `↑` / `↓` (menu open) | Move selection within the dropdown |
| `Enter` (menu open) | Activate the selected menu item |
| `Esc` / `F10` (menu open) | Close the menu |
| `q` / `Alt+X` | Quit |

## Project layout

```
src/
├── main.rs          Terminal setup/teardown, entry point
├── app.rs            Application state and input handling
├── ui.rs              Layout and rendering of all panels
├── theme.rs           Turbo Vision color palette and styles
├── mock.rs            Placeholder data (branches, commits, files, diff)
└── widgets/
    └── window.rs      Bordered + shadowed Turbo Vision-style window widget
```

## License

MIT — see [LICENSE](LICENSE).
