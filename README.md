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
> with an error if there isn't one. Local and remote-tracking branches live
> in two separate panels; tags are still read from the repository (the
> branch-name validation needs them) but aren't shown in any panel. When the
> working copy has changes, History shows an "Uncommitted changes" row above
> the log. The Changes panel follows the History selection: the working copy
> on that row, the files touched by the selected commit on any other. The
> Diff panel follows the Changes selection: it shows the diff of the selected
> file only, in the same context (working copy or commit).
> Creating and deleting local branches, staging and unstaging files and
> committing write to the repository for real; checkout, amend and stash
> don't exist yet. `src/git/mock.rs` is now test-only fixture data.

## Preview

```
┌File─Branch─Commit─View─Help──────────────────────────────────────────────┐
╔ Branches ════╗╔ History ═════════════════════════════════════════════════╗
║ * main       ║║             Uncommitted changes                         ║
║   develop    ║║ ●─╮ a1b2c3d Merge feature/menu               Mario      ║
║   feature/.. ║║ │ ● d4e5f6a Add dropdown menu                Luca       ║
║              ║║ │ ● 9f8e7d6 Add menubar widget               Luca       ║
║              ║║ ●─╯ c0ffee1 Fix status bar                   Mario      ║
╚══════════════╝╚══════════════════════════════════════════════════════════╝
┌ Remotes ─────┐┌ Changes ═══════════┐┌ Diff ══════════════════════════════┐
│ origin/main  ││ [x] M src/main.rs  ││ diff --git a/src/theme.rs ...      │
│ origin/dev.. ││ [ ] M src/theme.rs ││ @@ -1,6 +1,9 @@                    │
│              ││ [ ] A src/ui.rs    ││-pub fn window() -> Style {         │
│              ││ [ ] D README.md    ││+pub fn window() -> Style {         │
└──────────────┘└────────────────────┘└────────────────────────────────────┘
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
- **SourceTree-style layout**: Branches, Remotes, History, Changes and Diff
  panels on screen together — local branches and remote-tracking branches
  each get their own panel, sharing the left column — automatically
  collapsing to a single focused panel on narrow terminals (< 80 columns).
- **Uncommitted changes in the log**: when the working copy is dirty, the
  History panel lists it as its first row, with the graph, hash and author
  columns left empty — so it reads as "not committed yet" even without
  colors. With a clean working copy the row isn't there at all.
- **Changes follows History**: select a commit and Changes lists the files
  it touched, compared with its first parent (so a merge shows what it
  brought into its branch); select the uncommitted row and you're back on
  the working copy, with the cursor where you left it. While a commit's
  files are loading, when they can't be read, or once git is no longer
  available, the panel says so instead of going blank. Renames show up as a
  delete plus an add.
- **Diff follows Changes**: the Diff panel shows the diff of the file
  selected in Changes, and nothing else. On the working copy it is
  `git diff HEAD` for that file — staged and unstaged changes together,
  since Changes has a single row per file — and untracked files show their
  content; on a commit it is what that commit did to the file, compared
  with its first parent (or with nothing, for the root commit). Binary
  files, files with no line changes (an empty new file, a permission
  change) and read errors each get their own message in the panel. Moving
  quickly through History or Changes doesn't flood git with reads: at most
  one read per panel is in flight at a time, so the rows you only pass over
  are mostly never read. Going back to one of the last commits you looked
  at shows its files and diff again without reading them.
- **Stage, unstage and commit**: `Space` on a working-copy file in Changes
  stages it, or unstages it if it is fully staged; the checkbox shows `[x]`
  for staged, `[ ]` for unstaged and `[~]` for a file staged only in part
  (staged, then changed again), and `Space` on it stages the rest. Staging
  a deleted file stages its deletion, an untracked directory is staged with
  all the files in it, and unstaging a new file makes it untracked again
  (also before the first commit). The checkbox changes when the repository,
  read again after the write, says so — never ahead of it. `Commit >
  Commit...` asks for a message and commits what is staged; an empty
  message, or nothing staged, is refused with a message instead of making
  an empty commit. The new commit shows up on top of History, and the
  "Uncommitted changes" row goes away once nothing is left to commit.
- **Keyboard-driven UX**: panel switching, list navigation, stage toggle,
  diff scrolling — scrolling
  stops with the last line of the diff at the bottom of the panel, instead
  of pushing it to the top of an otherwise empty panel, and goes back to
  the top only when a different diff is shown.
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
- [x] Show the uncommitted working copy on top of the history.
- [x] Branch creation/deletion (local branches). Checkout still missing.
- [x] Make Changes follow the History selection: the working copy for the
      uncommitted row, the files of the commit otherwise.
- [x] Make Diff follow it too: the diff of the file selected in Changes.
- [x] Stage/unstage and commit from the UI.
- [ ] Wire up the remaining menu actions. Working so far: `File > Quit`,
      `Help > About`, `Branch > New branch...`, `Branch > Delete` and
      `Commit > Commit...` (all three change the repository for real); the
      rest show a "not available yet" dialog.
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
| `Tab` / `Shift+Tab` | Move focus to the next / previous panel (Branches, Remotes, History, Changes, Diff) |
| `↑` / `k`, `↓` / `j` | Move selection (or scroll the diff, when focused) |
| `Space` | Stage the selected file, or unstage it if it is fully staged (Changes panel, working copy only — ignored on a commit's files). The status bar says which one it will do |
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

> **Note:** `Branch > Delete` acts on the branch selected in the focused
> panel when that panel lists branches — *Remotes* when it has the focus,
> *Branches* otherwise, including when the focus is on History, Changes or
> Diff. What happens next depends on that branch and not on the focus: a
> remote-tracking branch is refused with a message, the current branch is
> refused too, any other local branch is deleted after a confirmation. With
> the Remotes panel focused but empty there is no branch to act on, so the
> message is "No branch selected." instead.

> **Note:** the Diff panel does not wrap long lines: it cuts them at the
> right edge, like `less -S`. There is no horizontal scrolling, so the tail
> of a line wider than the panel can't be read.

> **Note:** the diff of the working copy is read again every time the
> repository is reloaded, even if nothing changed — and the repository is
> reloaded after every stage, unstage and commit: for a moment the panel
> says "Loading..." and the scroll position goes back to the top. For the
> same reason, pressing `Space` twice in quick succession can stage a file
> twice instead of staging and then unstaging it (the repository is not
> harmed). An untracked directory that is itself a Git repository shows
> "Nested repository" in the Diff panel, since its files belong to another
> repository, and can't be staged from here: staging a directory that
> contains one is refused with a message, without staging anything. Changes
> lists such a directory as a single row, so its other files have to be
> staged with `git`. A tracked submodule is different: when it points to a
> new commit it is staged like any other change, and the Diff panel shows
> the `-Subproject commit` / `+Subproject commit` lines, as `git diff` does.

> **Note:** commits are created through `libgit2`, not by running
> `git commit`. Git hooks (`pre-commit`, `commit-msg`, ...) don't run,
> `commit.gpgsign` is ignored, so with signing configured the commit is
> created **unsigned, without any warning**, and `commit.template` and
> `commit.cleanup` are ignored too. The message is a single line of up to
> 200 characters, so a body can't be written from here. A commit is
> refused while a merge, rebase or similar operation is in progress: finish
> it with `git` first.

> **Note:** git work happens on a worker thread. If that thread dies — a
> defect in the backend, or in `git2` itself — the app does not quit: it says
> so once with a "Git unavailable" dialog, then keeps a `!` marker at the
> head of the status bar. Branches, log and the working copy's file list
> already read stay on screen and can still be browsed; they just don't
> refresh any more, and any action that would need git is refused with that
> same message instead of failing silently. Restart to use git again. The
> refusal names the condition but not the action refused, so a confirmed
> "Delete branch" turns into a bare "Git unavailable", and text typed into
> "New branch" or "Commit" is lost. `Space` on a file gets the same refusal.
> Moving through History never brings up that dialog: the files of the last 8 commits you looked at, and the last 8 diffs you
> opened in those commits, stay readable; any other commit shows "Git
> unavailable: files not loaded". The working copy's diff is not kept: back
> on the "Uncommitted changes" row the file list is still there, but the Diff
> panel shows "Git unavailable: diff not loaded", because files on disk may
> have changed since it was read.

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
├── app/              Model, update() and the main loop, one concern per file
│   ├── mod.rs          Module wiring and re-exports; init_cmds()
│   ├── panel.rs        Panel: the five panels and their Tab order
│   ├── list_panel.rs   ListPanel<T>: a list's items and its cursor as one
│   │                   value, keeping the selection valid (or None on an
│   │                   empty list); whatever changes them is visible only
│   │                   inside app/, so drawing can't
│   ├── model/          Model and its overlays; Model::content_mut(), the
│   │                   single "panel -> data" table; HistoryEntry, the
│   │                   History rows (the commits, plus the uncommitted one)
│   │   ├── changes.rs    What the Changes panel shows: the working copy or
│   │   │                 a commit's files
│   │   ├── reading.rs    Reading<K, T>: data read while navigating (a
│   │   │                 commit's files, a file's diff), tied to what it
│   │   │                 was read for, and whether it has arrived; Kept,
│   │   │                 the last views already read, so going back
│   │   │                 doesn't read them again
│   │   └── navigation.rs Changes follows History, Diff follows Changes;
│   │                     which reads to ask for, at most one in flight per
│   │                     panel
│   ├── dialogs.rs      Every dialog the app can show, built in one place:
│   │                   one function per dialog, private to app/
│   ├── menu.rs         The menu bar's items (MENUS) and its key handling
│   ├── dialog_flow.rs  Dialog key handling and the Cmd an accepted dialog asks for
│   ├── results.rs      Applies the outcome of a Cmd (a Msg from the worker)
│   ├── update.rs       update(): Msg -> Model changes and Cmds; main-panel keys
│   ├── run.rs          The main loop: the only file in app/ that does I/O
│   ├── test_support.rs Test-only helpers: a Model filled with the mock data
│   └── tests/          Tests of update(); diff_follow.rs: the Diff panel
│                       following Changes; staging.rs: stage, unstage and
│                       commit
├── msg.rs            Msg (incoming events) and Cmd (requested effects)
├── effects.rs        Runs a Cmd against Repo, produces the resulting Msg
├── worker.rs         Worker thread owning the Repo, so git never blocks the UI
├── dialog.rs         Modal dialog state and key handling (no rendering)
├── input/
│   └── mod.rs        Key events -> Msg
├── git/
│   ├── repo.rs       The Repo trait: the only boundary towards git
│   ├── domain.rs     Branch, CommitId, Commit, FileEntry, DiffTarget,
│   │                 FileDiff: git as the app sees it
│   ├── git2.rs       Real repository backend, via `git2`
│   ├── validate.rs   Branch-name rules and error messages, shared by backends
│   └── mock.rs       Test-only fixture data
└── ui/
    ├── mod.rs        view(): panel layout and render dispatch; reports the
    │                 height of the Diff panel back to update() as a Msg
    ├── hints.rs      The key hints for the status bar, as a function of the
    │                 Model's state (rows come straight from
    │                 Model::content_mut, matched in mod.rs)
    ├── theme.rs      Turbo Vision color palette and styles
    ├── text.rs       Text-fitting helpers
    ├── panels/       Branches, Remotes, History, Changes, Diff, menubar,
    │                 status bar, dialogs (branches.rs renders both branch
    │                 panels)
    └── widgets/
        └── window.rs Bordered + shadowed Turbo Vision-style window widget
```

## License

MIT — see [LICENSE](LICENSE).
