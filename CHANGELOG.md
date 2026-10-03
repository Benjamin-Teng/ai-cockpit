# Changelog

All notable changes to AI Agent Cockpit are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

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
