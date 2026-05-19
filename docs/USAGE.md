# ccdash usage

A walkthrough of every feature ccdash ships in v1.0.

## CLI

After `brew install cjtaylor10/ccdash-tap/ccdash`, three binaries are on PATH:
`ccdash`, `ccdash-daemon`, `ccdash-ui`.

### `ccdash status`

Reports the daemon's health and a count of projects + sessions.

```
$ ccdash status
daemon: ok
projects: 4
sessions: 0
```

### `ccdash project add <path>`

Registers a git repository as a ccdash project. Worktrees are auto-discovered
via `git worktree list --porcelain`.

```
$ ccdash project add ~/code/loanplatform
ok: registered project "loanplatform" (5 worktrees discovered)
```

### `ccdash project list`

Pretty-prints the registered projects.

### `ccdash project rm <id>`

Unregisters a project. Sessions and worktrees are not deleted — they're just
removed from ccdash's registry.

### `ccdash list [--project <p>]`

Lists running tmux sessions (filtered to project if `--project` is given).

### `ccdash launch <project> [--worktree <name>] [--command <override>]`

Launches a new tmux session running `claude` (or your override) in the named
worktree of the named project. If port conflicts are detected, the daemon
returns a `PortConflict` with a `force_token` — re-invoke with
`--force <token>` to bypass.

### `ccdash kill <session-id>`

Terminates a tmux session by its stable tmux session id (`$N`).

### `ccdash ports [--project <p>]`

Lists running TCP listeners + declared ports per project.

### `ccdash plan <project>`

Pretty-prints the parsed phase/task progress from each plan markdown file
under that project's `docs/superpowers/{specs,plans}/`.

## UI

`ccdash-ui` opens the desktop app. On first launch it shows a welcome modal:

1. **Pick a directory to scan.** Most users pick `~/Documents` or wherever
   their code lives. The scanner descends up to 4 levels deep and ignores
   `node_modules`, `target`, `.git`, `dist`, `build`, `.next`, etc.
2. **Approve discovered repos.** All checkboxes are pre-selected; uncheck
   anything you don't want. Click "Add N projects".
3. **Skip for now** is also available — you can add projects manually via the
   `+ Add` button in the sidebar header.

### Sidebar

- One row per project. Click to select; the session/port/plan tabs filter to
  the selected project's data.
- Worktrees appear as sub-rows. Branch names longer than 24 chars get
  middle-ellipsis truncation; hover for the full name.
- **Drag-and-drop** project rows to reorder. Order is persisted to
  `~/.ccdash/projects.toml`.
- **Right-click** a project row → context menu → "Remove project" (with
  confirm).
- **+ Add** button in the header opens a native folder picker to add a
  project.

### Tabs

#### Sessions

Table of running tmux sessions. Columns: tmux id, name, pid, cwd, state,
actions. Actions:

- **Attach** — mounts an `xterm.js` terminal in the bottom pane wrapping
  `tmux attach-session -t <id>`. Resize the panel and the pty resizes too.
- **Kill** — terminates the tmux session (with confirm).

When >10 sessions exist, a filter input appears above the table that
live-filters on name + cwd + tmux session id.

#### Ports

Two tables: running TCP listeners (from `lsof`) and declared ports (parsed
from `package.json`, `.env`, `docker-compose.yml`, `Procfile`).

#### Plans

Parsed plan markdown files. Each plan shows:
- Title + path
- "Open in VS Code" button (deep link via `vscode://file/{path}`)
- Each phase with its task list, done/not-done state, and progress count.

Task titles render inline markdown (backticks, emphasis, links).

#### Browser

Embedded loopback URL preview.

- **Left rail** lists detected URLs from two sources combined and deduped:
  - Running TCP listeners (synthesized as `http://localhost:PORT`)
  - Live terminal output regex-matched for `localhost`/`127.0.0.1`/`0.0.0.0`/`[::1]` URLs
- **Address bar** + Go button for manual navigation.
- **Back / Forward / Reload** navigation chrome.
- **Open in external browser** (↗) launches the default system browser via
  the Tauri shell plugin.

### Top bar

- **Launch session** opens a modal: project picker + worktree picker +
  command override. On port conflict, the modal lists the colliding holders
  and offers "Launch anyway" using the daemon-supplied force token.
- **Mirror dropdown** lets this window follow another window's selection.
- **+ New window** opens a fresh ccdash window.
- **Theme select** — Auto / Dark / Light. Persisted to localStorage.
- **Health dot** — green (connected), yellow (reconnecting), red
  (disconnected). Tooltip names the state.

### Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Cmd+N` | New window |
| `Cmd+W` | Close window |
| `Cmd+K` | Command palette |
| `Cmd+L` | Launch session dialog |

The command palette has type-ahead filter over every project switch +
common actions (Launch, Add, New/Close window, Tab switches).

### Multi-window

- `+ New window` opens an additional ccdash window. Each window has its own
  selection state.
- The mirror dropdown sets the current window to follow another window's
  selected project + tab. Terminal panes auto-sync because both windows
  attach to the same underlying tmux session id.
- Off-screen restored window positions snap back to the primary monitor.

## Daemon

The daemon runs as a launchd agent (macOS) or systemd user unit (Linux).
Both register on `brew install` and auto-start on next login.

### Manual start/stop (macOS)

```bash
brew services start cjtaylor10/ccdash-tap/ccdash
brew services restart cjtaylor10/ccdash-tap/ccdash
brew services stop cjtaylor10/ccdash-tap/ccdash
```

### Manual start/stop (Linux)

```bash
systemctl --user enable --now ccdash-daemon.service
systemctl --user restart ccdash-daemon.service
systemctl --user stop ccdash-daemon.service
```

### Configuration

| Path | Contents |
|---|---|
| `~/.ccdash/auth` | shared-secret token; mode `0600` |
| `~/.ccdash/projects.toml` | registered projects + order |
| `~/.ccdash/sessions.toml` | session metadata (tmux session_id ↔ project) |
| `~/.ccdash/ui.log` | UI process Rust + frontend log |
| macOS log: `~/Library/Logs/ccdash/` | daemon stdout/stderr |
| Linux log: `journalctl --user -u ccdash-daemon` | daemon journal |

### Reconnect behavior

If the daemon crashes or is restarted, every open UI window shows a
"Disconnected from daemon — retrying in Ns" banner with a "Retry now"
button. Auto-retry happens with exponential backoff (5s → 30s cap).
