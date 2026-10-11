# Changelog

All notable changes to AI Agent Cockpit are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Cards follow your OpenSpec progress.** For a repo you added as a project, Cockpit now reads
  each pane's current git branch and the `openspec/changes/` folder of its worktree about every
  10 seconds, and moves the card to the stage that matches the change's phase: planning (no task
  ticked yet), implementing (some ticked), review (all ticked) or complete (archived). The change
  is matched by the last part of the branch name (`feat/foo` matches `openspec/changes/foo`); if
  that fails, an archived change with that name, or the only change in progress, is used. A card
  that matches a change shows the change name, the ticked count (for example `3/8`) and whether it
  is **Auto** or **Manual**.
- **Pick the OpenSpec phase of each stage.** The stage editor (the **⋯** menu on a project) has a
  dropdown on every row: Plan, Implement, Review, Complete or none. A phase can belong to one stage
  only. New projects start with the four default stages already mapped, and in existing projects,
  each stage named exactly like a default (English or Traditional Chinese) is mapped to its phase
  the first time they are loaded (if a phase would repeat, only the first stage keeps it).
- `POST /api/repo-projects` accepts an optional `phases` list and each stage in
  `PATCH /api/repo-projects/<pid>` an optional `phase` (leaving it out means no phase).

### Changed

- **Your own moves win until the progress changes.** After you advance or step back a card (or an
  agent advances it), the card stays where you put it and is marked **Manual**. The next time the
  OpenSpec progress changes (for example another task is ticked), it moves back to the matching
  stage and returns to **Auto**. Cards marked Completed or Failed never move by themselves, and
  Cockpit never sets those marks for you, not even when a change is archived.
- **State file version 4.** `cockpit.state.json` now records the phase of each stage and, for each
  card, whether it is Auto or Manual. Older state files are read as before and upgraded the next
  time something changes. **Going back is not possible without a reset:** Cockpit 0.1.5 and earlier
  refuse to start when they find a version 4 state file. Before installing an older version, delete
  or rename `cockpit.state.json`; you lose your progress and the repos you added.
- The detection is read-only: it only reads git and files inside the repo, never calls the
  `openspec` command and never sends anything to HERDR. It does not start a stopped WSL
  distribution, and a worktree whose runtime is disconnected keeps its last known state.

## [0.1.5] - 2026-10-08

### Changed

- **Selecting a project now selects one of its panes.** Clicking a project in the left column
  (or choosing it with the keyboard, or adding a repo) now selects one of that project's panes,
  just like pressing **View output** on its row: the Files and Changes tabs and Live Output switch to
  that pane, and the runtime list on the right scrolls to it. A pane whose agent is working is
  preferred, otherwise the first bound workstream is used; if no workstream is bound, the selected
  pane stays as it was. Your open file tabs and side-by-side columns are kept. The project shown
  when the page first opens, and choosing a project while rebinding a workstream, do not select a
  pane.

## [0.1.4] - 2026-10-08

### Added

- **Add a git repo as a project from the dashboard.** The Project tab in the left column now
  lists the git repos your panes are working in under **Detected repos**, each with an **Add**
  button. After you add one, every pane inside that repo (any worktree, any HERDR workspace)
  becomes a workstream with its own task card, and the card goes away when the pane closes. A new
  project starts with four stages; the **⋯** menu on a project lets you rename it, edit its stages
  or remove it, all without restarting. Panes in a linked git worktree are labelled with the
  worktree's folder name. You no longer have to write `[[project]]` sections in `cockpit.toml`;
  the ones you already have keep working. A pane that moves to another folder can take up to
  about 30 seconds to move to its new repo, and a folder opened from both Windows and WSL is
  listed as two repos.
- **Agents can advance their task without any IDs.** `POST /api/agent/advance` finds the task
  that belongs to the calling pane from the `X-Herdr-Pane-Id` header, so one line in `AGENTS.md`
  is enough. It answers 404 `no_task_for_pane` when the pane has no task and 409
  `ambiguous_task` when it has more than one. As before, panes of a WSL runtime are not supported.
- **Progress is saved without a config file too.** Without a `cockpit.toml`, Cockpit now keeps
  its state in `%LOCALAPPDATA%\ai-cockpit\cockpit.state.json`, so the repos you added
  survive a restart. The file is only created once there is something to save.

### Changed

- **State file version 3.** `cockpit.state.json` now records the repos you added. Older state
  files are read as before and upgraded the next time something changes. **Going back is not
  possible without a reset:** Cockpit 0.1.3 and earlier refuse to start when they find a version 3
  state file. Before installing an older version, delete or rename `cockpit.state.json`; you lose
  your progress and the repos you added.
- **The empty dashboard no longer asks you to restart.** It now points to **Detected repos**.

## [0.1.3] - 2026-10-07

### Added

- **Read files side by side.** Up to three file tabs can be shown next to each other in equal
  columns. Each file tab has a split button (shown next to its close button when you hover over
  the tab); Ctrl+click or Ctrl+Enter on a tab does the same. One column is the focus column, with
  an accent outline: click anywhere in a column to make it the focus column, and opening another
  file (from a tab, the file tree or a Markdown link) replaces it while the other columns stay as
  they are. Every visible column keeps updating when its file changes, and the arrangement is
  restored when you reload the page. Live Output, diff and Git Graph tabs cannot be split. In a
  window narrower than 760 pixels only the focus column is shown.

## [0.1.2] - 2026-10-05

### Changed

- **New app icon.** Cockpit now has its own icon, an attitude indicator with a prompt sign. It
  appears on the desktop and Start menu shortcuts, the installer, the installed apps list, the
  page icon of the Cockpit window and desktop notifications. Before, the shortcuts showed the
  blank Windows program icon.
- **Program details.** `cockpit.exe` and `cockpit-launch.exe` now carry the product name
  "AI Agent Cockpit" and their version number, so Task Manager and the file properties show them.
- **Version in the status bar.** The bottom-right corner of the dashboard now shows the Cockpit
  version, for example `v0.1.2`. It used to show a counter of screen updates, which looked like a
  version number.

## [0.1.1] - 2026-10-04

### Added

- **Automatic updates for the installer version.** When you open Cockpit from the shortcut and it
  is not already running, it checks GitHub for a newer stable release, at most once every 24
  hours, and asks before doing anything. If you agree, it downloads the installer, verifies its
  SHA-256 against the release's `SHA256SUMS.txt`, closes, updates and reopens itself. The
  download does not go through a browser, so SmartScreen's prompt usually appears only at the
  first install. If the download or the verification fails, Cockpit shows an error and opens the
  version you already have. The check sends one request to `api.github.com`; set `COCKPIT_NO_UPDATE_CHECK=1` to
  turn it off. The zip, `install-desktop.ps1` installs and source builds are not updated
  automatically. Anyone on v0.1.0 has to install the new version by hand once, because v0.1.0 has
  no updater.

### Changed

- **v0.1.0 is now marked as a pre-release** on GitHub, since it cannot update itself. It stays
  available for download; v0.1.1 is the first stable release.

## [0.1.0] - 2026-10-03

First public release. Windows x64 only; HERDR must already be installed.

### Added

- **Installer and zip.** `ai-cockpit-0.1.0-x64-setup.exe` installs for the current user without
  admin rights and adds Start menu and desktop shortcuts. `ai-cockpit-0.1.0-x64.zip` holds the same
  two programs for running without installing. Neither is code-signed yet, so Windows SmartScreen
  asks before the first run.
- **Factory Floor.** Each project is a pipeline of stages with workstreams side by side. Task
  cards show running, blocked or failed work, and one click advances a task, sends it back or
  marks it.
- **Several HERDR runtimes at once.** Windows and WSL runtimes merge into one picture, each with
  its own connection light.
- **Live Output in color.** Read any pane's recent output with its colors. It is a view, not a
  terminal.
- **Files, diffs and git history.** Read-only Markdown, PDF and HTML viewers, side-by-side diffs
  and a commit graph for the repo a pane works in.
- **Agent progress reports.** An agent announces its task and moves it forward with one HTTP call
  from a Windows-side pane.
- **Desktop alerts** when an agent is blocked or a task fails, with optional alerts for the rest.
- **English or Traditional Chinese.** The dashboard picks one from your browser language and time
  zone, with a switch in the top bar. The launcher's message boxes follow the Windows display
  language.
- **Desktop launcher.** One shortcut starts Cockpit in the background and opens it in its own
  window; closing the window shuts Cockpit down.
- **Read-only by design.** Cockpit can only send `session.snapshot`, `pane.read` and
  `events.subscribe` to HERDR, listens on loopback only, and checks `Host` and `Origin` on every
  endpoint that returns or changes your data.
