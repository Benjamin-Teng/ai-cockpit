<!-- markdownlint-disable-next-line MD041 -->
[English](README.md) | [繁體中文](README.zh-TW.md)

![AI Agent Cockpit: four agent workstreams shown as dot-matrix signals moving through Plan, Build and Review](docs/assets/readme-banner.svg)

# AI Agent Cockpit

A local, read-only dashboard for AI coding agents running in [HERDR](https://herdr.dev).
Cockpit turns HERDR panes into a live factory floor: which workstream each agent drives,
which stage it reached, and which one is waiting on you.

**Website:** <https://benjamin-teng.github.io/ai-cockpit/>

![Cockpit dashboard with sample data: project list, Factory Floor grid of workstreams and stages, and HERDR runtime cards](docs/research/2026-10-01/progress-1536.png)

## What it does

- **Factory Floor.** Each project is a pipeline of stages, and its workstreams run side by side.
  Task cards show whether work is running, blocked or failed. Advance a task, send it back or
  mark it with one click.
- **One picture across Windows and WSL.** Cockpit connects to several HERDR runtimes at once
  and merges them, with a connection light for each.
- **Live Output in color.** Read any pane's recent output with its colors intact. It is a
  view, not a terminal: nothing you do there reaches the agent.
- **Files, diffs and git history.** Browse the repo a pane is working in: Markdown, PDF, HTML,
  side-by-side diffs and a commit graph, all read-only.
- **Agents report their own progress.** An agent announces its task and moves it forward with
  one HTTP call from its own pane. Cockpit never guesses. For now this works from Windows-side
  panes only.
- **Desktop alerts.** Get a notification when an agent is blocked or a task fails, and switch
  on alerts for the rest. Alerts appear while the Cockpit window is open or minimized and not in
  front. Click one and Cockpit opens that pane.
- **Opens like a desktop app.** A Windows shortcut starts Cockpit in the background and opens
  it in its own window. Close the window and Cockpit shuts down.

The dashboard's labels are mostly in Traditional Chinese.

## Read-only by design

Cockpit is an observer. It cannot type into a pane, prompt an agent or stop HERDR.

- The only HERDR calls it can make are `session.snapshot`, `pane.read` and
  `events.subscribe`. A sealed trait in `herdr-client` enforces this list at compile time.
- The dashboard listens on loopback only (`127.0.0.1:7770` by default). Every endpoint that returns
  or changes your data, including the live state feed (`/api/state` and `/ws`), checks the `Host`
  and `Origin` headers, so other web pages open in your browser cannot read the dashboard.
- Progress changes only when you click or when an agent reports it. Cockpit writes nothing to
  your repos or to HERDR. Its own files are `cockpit.state.json` and, with the desktop launcher,
  `cockpit.log`.

## Get started

Cockpit is built from source with Rust (edition 2024). Windows is the tested platform, and
HERDR inside WSL is supported. The desktop launcher needs Chrome or Edge. The git views need
`git` on `PATH`. Cockpit has been tested with HERDR 0.9.0-preview on Windows and HERDR 0.8.2 in WSL.

```bash
git clone https://github.com/Benjamin-Teng/ai-cockpit.git
cd ai-cockpit
cargo run -p cockpit
```

Open <http://127.0.0.1:7770/>. Without a config file, Cockpit finds your local HERDR on its own.

To describe your projects, copy the example config. Before running, replace its `<user>`
placeholders, then list your HERDR runtimes, each project's stages and the pane every
workstream runs in. Cockpit reads `cockpit.toml` from the working directory, or the file you
pass with `--config`.

```powershell
Copy-Item cockpit.example.toml cockpit.toml
cargo run -p cockpit
```

### Desktop shortcut (Windows, optional)

```powershell
pwsh scripts/install-desktop.ps1
```

This builds a release, installs it under `%LOCALAPPDATA%\ai-cockpit\bin` and puts an
**AI Agent Cockpit** shortcut on your desktop. In this mode Cockpit shuts down about
10 seconds after you close its last window, so progress reports sent after that are lost.
Minimize the window instead of closing it while agents are working.

### No HERDR yet?

```bash
cargo run -p cockpit --example ui_preview
```

This opens the dashboard at <http://127.0.0.1:7770/> with sample data.

## Let agents report progress

From inside an agent's own HERDR pane (Windows-side runtimes):

```bash
# list the tasks bound to this pane
curl -s http://127.0.0.1:7770/api/agent/tasks -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
# declare the task you are working on
curl -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/start   -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
# finish this stage and move to the next
curl -i -X POST http://127.0.0.1:7770/api/agent/projects/cockpit/tasks/impl/advance -H "X-Herdr-Pane-Id: $HERDR_PANE_ID"
```

`cockpit` and `impl` are the project and task IDs from the example config; use your own.
In PowerShell, use `curl.exe` and `$env:HERDR_PANE_ID`. Marking a task completed or failed is
left to you. The full API, including a snippet you can paste into your project's `AGENTS.md`,
is in [`cockpit/README.md`](cockpit/README.md) (Traditional Chinese).

## How it is built

A Cargo workspace:

| Crate | Role |
|---|---|
| `herdr-client` | HERDR protocol client and transports (named pipe, Unix socket, child-process stdio for WSL); observer methods only |
| `cockpit-core` | Runtime and domain models, per-runtime driver, projection to the dashboard state; no HERDR types |
| `cockpit-herdr` | Translates HERDR into core types; the only crate that sees both sides |
| `cockpit-files` | File browsing with a root allowlist and safe paths |
| `cockpit-git` | A closed, read-only git query layer |
| `cockpit` | The program: axum HTTP and WebSocket, the embedded single-page UI, and the `cockpit-launch` desktop launcher |

Design decisions live in [`docs/adr/`](docs/adr/), the glossary in [`CONTEXT.md`](CONTEXT.md)
and the capability specs in [`openspec/specs/`](openspec/specs/).
