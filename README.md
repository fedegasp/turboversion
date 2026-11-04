# TurboVersion

*Turbo Vision meets version control.*

A terminal-based Git client written in Rust, styled after the blue/yellow
**Turbo Vision / Turbo C** look of early-90s Borland IDEs, with the panel
layout and workflow of **SourceTree**: branches, commit history, working-tree
changes and diff, all visible at once, navigable entirely from the keyboard.

> **Status: early prototype.** The UI, layout, menu system and keybindings
> are implemented and working. The app reads branches, commit history,
> working-tree status and diffs from a real Git repository (via
> [`git2`](https://docs.rs/git2), see [`src/git/git2.rs`](src/git/git2.rs)):
> it picks up the repository containing the current directory and exits
> with an error if there isn't one. Creating and deleting local branches
> writes to the repository for real; staging and commits don't yet.
> `src/git/mock.rs` is now test-only fixture data.

## Preview

```
┌File─Branch─Commit─View─Help──────────────────────────────────────────────┐
╔ Branches ════╗╔ History ═════════════════════════════════════════════════╗
║ * main       ║║ ●─╮ a1b2c3d Merge feature/menu               Mario      ║
║   develop    ║║ │ ● d4e5f6a Add dropdown menu                Luca       ║
║   feature/.. ║║ │ ● 9f8e7d6 Add menubar widget               Luca       ║
║   origin/... ║║ ●─╯ c0ffee1 Fix status bar                   Mario      ║
║   v0.1.0     ║╚══════════════════════════════════════════════════════════╝
║              ║┌ Changes ═══════════┐┌ Diff ══════════════════════════════┐
║              ║│ [x] M src/main.rs  ││ diff --git a/src/theme.rs ...      │
║              ║│ [ ] M src/theme.rs ││ @@ -1,6 +1,9 @@                    │
╚══════════════╝│ [ ] A src/ui.rs    ││-pub fn window() -> Style {         │
                │ [ ] D README.md    ││+pub fn window() -> Style {         │
                └────────────────────┘└────────────────────────────────────┘
 Tab Panel  ↑↓ Move  Space Stage  F10 Menu  Alt-X Quit
```

A thick border marks the focused panel (a double border is reserved for modal
dialogs); the active window's title is highlighted; a drop shadow is cast
behind floating elements such as the dropdown menus — all classic Turbo
Vision touches.

## Features

- **Turbo Vision aesthetic**: 16-color ANSI palette, yellow-on-blue windows,
  thick border when focused / plain border otherwise, drop shadows on
  floating panels.
- **SourceTree-style layout**: Branches, History, Changes and Diff panels
  on screen together, automatically collapsing to a single focused panel on
  narrow terminals (< 80 columns).
- **Keyboard-driven UX**: panel switching, list navigation, file staging
  toggle, diff scrolling.
- **Turbo Vision-style menu bar**: `F10` or `Alt+<letter>` opens a dropdown
  menu, navigable with arrow keys.
- **Modal dialogs**: Turbo Vision-style message boxes, Yes/No confirmations and
  text-input prompts, centered with a drop shadow. They capture all input until
  dismissed and the status bar switches to the dialog's own key hints.
- Built on [`ratatui`](https://ratatui.rs) with a custom `Window` widget
  implementing the bordered/shadowed Turbo Vision chrome.

## Roadmap

- [x] Replace mock data with real repository data (via `git2`): branches,
      commit log, working-tree status, diffs.
- [ ] Wire up the remaining menu actions. Working so far: `File > Quit`,
      `Help > About`, `Branch > New branch...` and `Branch > Delete` (both
      now change the repository for real); the rest show a "not available
      yet" dialog.
- [ ] Stage/unstage and commit from the UI.
- [x] Branch creation/deletion (local branches). Checkout still missing.
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
| `Alt+X` / `Ctrl+C` | Quit (`Alt+X` and `Ctrl+C` also work with a dialog or menu open) |

> **macOS note:** Terminal.app does not send `Alt`/`Option` as a Meta key by
> default, so `Alt+X` and `Alt+<letter>` menu shortcuts won't register unless
> you enable *Use Option as Meta Key* in the profile's Keyboard settings (or
> use an emulator that does this by default, e.g. iTerm2). `F10` and
> `Ctrl+C` always work regardless of this setting.

### Dialogs

| Key | Action |
|---|---|
| `Enter` | Activate the focused button (`OK` / `Yes`) |
| `Esc` | Cancel / close |
| `Tab` / `←` / `→` | Move between buttons |
| `Y` / `N` | Answer a Yes/No dialog directly |
| any text, `Backspace` | Edit the field of an input dialog |

## Project layout

The app follows an Elm-style Model / Update / View split, with a single
boundary towards git (the `Repo` trait). See
[`docs/architettura.md`](docs/architettura.md) for the full picture.

```
src/
├── main.rs          Terminal setup/teardown, entry point
├── app.rs            Model and update(): state plus the main loop
├── msg.rs            Msg (incoming events) and Cmd (requested effects)
├── effects.rs        Runs a Cmd against Repo, produces the resulting Msg
├── worker.rs         Worker thread owning the Repo, so git never blocks the UI
├── dialog.rs         Modal dialog state and key handling (no rendering)
├── input/
│   └── mod.rs        Key events -> Msg
├── git/
│   ├── repo.rs       The Repo trait: the only boundary towards git
│   ├── domain.rs     Branch, Commit, FileEntry: git as the app sees it
│   ├── git2.rs       Real repository backend, via `git2`
│   ├── validate.rs   Branch-name rules and error messages, shared by backends
│   └── mock.rs       Test-only fixture data
└── ui/
    ├── mod.rs        view(): layout of all panels
    ├── theme.rs      Turbo Vision color palette and styles
    ├── text.rs       Text-fitting helpers
    ├── panels/       Branches, History, Changes, Diff, menubar, status bar
    └── widgets/
        └── window.rs Bordered + shadowed Turbo Vision-style window widget
```

## License

MIT — see [LICENSE](LICENSE).
