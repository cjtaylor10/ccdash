# ccdash — Claude Code Dashboard

**Status:** Design approved, pending implementation plan
**Date:** 2026-05-17
**Audience:** Personal-first, OSS-eventual

---

## 1. Problem

Working with Claude Code at scale produces a sprawl that's hard to keep visible:

- Many projects, each with multiple git worktrees (5–8 per active project is typical).
- Multiple `claude` sessions running in parallel across worktrees and terminals.
- Port conflicts between projects (Loanplatform on 3000 vs another project on 3000).
- Plan/spec files (`docs/superpowers/specs/`, `docs/superpowers/plans/`) that track multi-phase work but live as scattered markdown.
- No single view of "what's running where, what's planned, what's stuck."

The Terminal + IDE + scattered notes workflow scales linearly with project count, and parallel sessions amplify the chaos. There's no observable, integrated workspace for this style of work.

## 2. Goal

Build a local desktop dashboard (`ccdash`) that gives one cohesive view of:

1. **Projects** — registered repos and their worktrees.
2. **Sessions** — running `claude` instances (tmux-backed; survive crashes; attach from anywhere).
3. **Ports** — what's listening, owned by which project, conflict prevention before launch.
4. **Plans** — phase/task progress parsed from superpowers markdown plan files.

Plus first-class support for multi-window workflows: open the dashboard in several windows, each focused on a different project, with optional "follow this window" mirror mode for second-monitor monitoring.

## 3. Non-Goals (v1)

- File browser / file editor inside the dashboard — use VS Code / Finder.
- Web/cloud deployment — local desktop only.
- Multi-user / team sharing — single-user, single-machine.
- Custom plan format — read what `writing-plans` already produces.
- Windows support — Mac + Linux v1.
- Authentication beyond local socket — single-user assumption.

## 4. Architecture

### 4.1 High-level

```
┌─────────────────────────────────────────────────────────┐
│  ccdash-daemon (long-lived Rust process)                │
│  • tmux session manager (control-mode + polling)        │
│  • project + worktree registry                          │
│  • port registry (lsof scan + declared-port parsers)    │
│  • plan watcher (notify on docs/superpowers/{specs,plans}/) │
│  • event bus (broadcast over Unix socket; JSON-RPC 2.0) │
└─────────────────────────────────────────────────────────┘
                ▲                          ▲
                │  JSON-RPC                │  JSON-RPC
                │  $SOCK                   │
       ┌────────┴───────────┐    ┌────────┴────────────────┐
       │  ccdash CLI        │    │  ccdash-ui (Tauri 2.x)  │
       │  (Rust binary)     │    │  Rust backend = proxy   │
       │                    │    │  SvelteKit frontend     │
       │  launch, list,     │    │  xterm.js terminals     │
       │  kill, status,     │    │  multi-window, mirror   │
       │  ports, plan, …    │    │                         │
       └────────────────────┘    └─────────────────────────┘
```

### 4.2 Process model

- **Daemon** is the **single source of truth** for all state. No state in the UI.
- **Daemon installed as a service**: launchd agent (`~/Library/LaunchAgents/com.ccdash.daemon.plist`) on macOS, systemd user unit on Linux. Auto-starts on login.
- **Daemon survives all windows closed.** State persisted to `~/.ccdash/{projects,sessions}.toml`; runtime state recoverable from tmux + lsof on restart.
- **One Unix socket** per host: `/tmp/ccdash.sock` (macOS), `$XDG_RUNTIME_DIR/ccdash.sock` (Linux).
- **Socket permissions** `0600` + shared-secret token at `~/.ccdash/auth` (`0600`). Clients present token in JSON-RPC handshake.

### 4.3 Source layout

Cargo workspace + Tauri app:

```
ccdash/
├── crates/
│   ├── ccdash-core/      # shared protocol + domain types
│   ├── ccdash-daemon/    # the long-lived service
│   └── ccdash-cli/       # `ccdash` command-line binary
├── apps/
│   └── ccdash-ui/        # Tauri app (Rust backend + SvelteKit frontend)
└── packaging/
    ├── homebrew/         # brew formula
    ├── launchd/          # macOS service plist
    └── systemd/          # Linux user unit
```

## 5. Components

### 5.1 `ccdash-core` (shared library)

- **Protocol types** — JSON-RPC method/param/result enums; serde-derived; single contract for daemon ↔ client.
- **Domain types** — `Project`, `Worktree`, `Session`, `PortBinding`, `Plan`, `PlanPhase`, `PlanTask`.
- **Auth loader** — reads `~/.ccdash/auth`, validates handshake token.

### 5.2 `ccdash-daemon`

| Module       | Responsibility                                                                                                                           |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `tmux`       | Long-lived `tmux -CC` control-mode subscription for instant events + 2s polling fallback via `tmux list-panes -a`.                       |
| `projects`   | `~/.ccdash/projects.toml` persistence; root-dir scanner producing a "detected" tray for user confirmation.                               |
| `worktrees`  | `git worktree list --porcelain` per project on directory-mtime change.                                                                   |
| `ports`      | 5s `lsof -nP -iTCP -sTCP:LISTEN` scan; per-project declared-port parsers for `package.json`, `.env`, `docker-compose.yml`, `Procfile`. |
| `plans`      | `notify` crate watches each project's `docs/superpowers/{specs,plans}/**/*.md`; parsed with `pulldown-cmark`; cached in-memory.          |
| `sessions`   | Joins live tmux state with `~/.ccdash/sessions.toml` metadata, keyed on tmux's stable `session_id` (not name).                          |
| `rpc`        | Unix-socket listener; JSON-RPC 2.0; method routing.                                                                                       |
| `broadcast`  | `tokio::sync::broadcast` channel; every state change publishes; subscribers receive pushed events.                                       |

### 5.3 `ccdash-cli`

```
ccdash launch <project> [--worktree <name>] [--command <override>]
ccdash list [--project <p>]
ccdash kill <session-id>
ccdash status                                # daemon health + counts
ccdash project add <path>
ccdash project rm <id>
ccdash project scan [<dir>]                  # trigger root-dir scan
ccdash ports [--project <p>]
ccdash plan <project>                        # pretty-print plan progress
```

### 5.4 `ccdash-ui` (Tauri 2.x)

- **Rust backend** — thin proxy. One JSON-RPC connection per app instance; fans events out to all windows via Tauri's event bus.
- **Frontend** — SvelteKit (lighter than React for dense state UI). Svelte stores per-window hydrated from snapshots + live events.
- **Terminal panes** — `xterm.js` paired with a Rust pty bridge:
  - **Live mode** (default) — pty bridge to `tmux attach-session -t <id>`.
  - **Monitor mode** — `tmux pipe-pane` stream only, no input.
  - Per-pane toggle.
- **Window relationships** — local Svelte store `{windowId, mirrorTarget?}`; when `mirrorTarget` set, subscribe to that window's published UI-state via Tauri's `emit_to`. Tmux handles terminal-level sync automatically because both windows attach to the same session-id.

## 6. Data Flow

### 6.1 Startup

1. User launches Tauri app or runs `ccdash <cmd>`.
2. Client tries to connect to the socket path.
3. On failure → invokes `launchctl load` / `systemctl --user start ccdash` → retries with exponential backoff (3 attempts, 0.5s/1s/2s).
4. On connect, sends `handshake { token }` from `~/.ccdash/auth`.
5. Sends `subscribe { topics: ["sessions", "ports", "plans", "projects"] }`.
6. Daemon returns current snapshot per topic, then streams deltas.

### 6.2 Launching a session

1. UI: user picks project + worktree, clicks "Launch."
2. UI → daemon: `session.launch { project_id, worktree, command? }`.
3. Daemon parses declared ports for that project; queries port registry.
4. **If conflict** → returns `Err(PortConflict { conflicts: [...], force_token })`.
5. UI shows remediation dialog with three actions:
   - **Kill conflicting session** — sends `session.kill { id }` for the blocking session, then retries launch.
   - **Change port** — opens an env-override input; launches with `PORT=<new>`.
   - **Launch anyway** — re-sends with `force_token`.
6. Daemon runs `tmux new-session -d -s ccdash:<project>:<worktree> -c <path> 'claude'`. Writes metadata to `sessions.toml` (keyed on tmux `session_id`). Broadcasts `session.launched`.
7. All windows update their session lists from the broadcast.

### 6.3 Plan progress update

1. User edits `docs/superpowers/plans/foo-plan.md` in their editor (VS Code, etc.), ticks a `- [ ]` to `- [x]`.
2. `notify` fires event → daemon re-parses file → updates in-memory plan state.
3. Daemon broadcasts `plan.updated { project_id, plan_id, phases }`.
4. All windows showing that project re-render the plan panel.

### 6.4 Multi-window mirror

1. Window B (the source) publishes UI-state changes to a local Tauri channel: `ui-state-broadcast::<window-b-id>`.
2. Window A (mirror) subscribes when user toggles "follow window B" from the window menu.
3. Window A receives `{project, worktree, view, selectedSession, openPanes}` deltas; applies them to its own Svelte store.
4. Terminal panes auto-sync because both windows attach to the same tmux `session_id` — tmux handles input/scroll sharing natively.

## 7. Key Design Decisions

### 7.1 Approach B (separate daemon + Tauri UI)
**Chosen** over singleton-Tauri or hybrid auto-spawn. Gives CLI companion from day 1, daemon survives all-windows-closed, cleanest path to OSS distribution.

### 7.2 Tmux-backed sessions
**Chosen** over dashboard-owned-pty or external-discovery. Free crash resilience, free shared input/scroll across windows, free CLI-↔-GUI attachability. Adds tmux as a runtime dependency (`brew install tmux`).

### 7.3 Worktrees as sub-rows under projects
**Chosen** over flattened or invisible. Matches `git worktree list` mental model, scales to dozens, lets us tag sessions/ports/plans with their worktree.

### 7.4 Port conflict policy: block + remediation
**Chosen** over warn-only or auto-rebind. UI offers three explicit choices on conflict (kill / change port / launch anyway). "Launch anyway" preserves user agency; user is never blocked from intentional concurrent runs.

### 7.5 Mirror Tier 2 + free Tier 3 via tmux
Explicit mirroring covers `{project, worktree, view, selectedSession, openPanes}`. Terminal content/scroll/input mirror automatically because both windows attach to the same tmux session.

### 7.6 Tmux session naming: hybrid
Sessions get human-readable names (`ccdash:loanplatform:main`) for `tmux ls` UX. Identity is tracked by tmux's stable `session_id` (`$0`, `$1`, ...) joined against `~/.ccdash/sessions.toml`. Rename-safe.

### 7.7 Session visibility: all tmux sessions running `claude`
Daemon shows any tmux pane whose `pane_current_command` is `claude`, regardless of session name. Preserves the "attach from anywhere" promise of tmux-backed.

### 7.8 Plan view: structured render, read-only
Parse markdown into `Phase`/`Task` cards with progress; never write back to files. Edits happen in the user's editor; daemon picks up changes via file watcher. Avoids dueling-writers conflicts with the editor.

### 7.9 Project detection: scan-with-confirm
First run scans configured root dirs (default `~/Documents/`) for git repos, surfaces them in a "Detected" tray. User bulk-approves. New worktrees auto-add once their parent project is approved.

### 7.10 Cross-platform: Mac + Linux v1
Both targets share POSIX socket, file watcher, tmux. Only platform-specific code: service file (launchd plist vs systemd unit) and socket path. Windows is a separate, larger project (no native tmux) — defer.

### 7.11 Auth: socket perms + shared secret
Socket `0600` plus a token at `~/.ccdash/auth` (`0600`) presented in JSON-RPC handshake. Closes the cross-user `/tmp` hole on multi-user Linux dev boxes. Per-client capability tokens deferred until/unless remote access is added.

### 7.12 Distribution: Homebrew + .deb + .pkg
`brew install ccdash` installs daemon, CLI, and Tauri app; post-install registers the launchd plist and loads it. Similar `.pkg` / `.deb` for non-brew users. Code-signing + notarization deferred to v1.5 (most early users will right-click → Open).

## 8. Error Handling

### Daemon-side

| Failure mode                       | Behavior                                                                                                                       |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Tmux not installed                 | Daemon fails fast with structured error; reported via `daemon.health` RPC; UI surfaces install instructions.                   |
| Tmux server crashes                | Control-mode connection drops; daemon reconnects with exponential backoff; sessions list rebuilt from `tmux ls` on reconnect. |
| Project deleted from disk          | File-watcher fires; project marked `state: missing`; UI shows greyed-out with "Reveal" / "Remove" actions.                    |
| `lsof` permission denied           | Other-user ports show as "in use, owner: other" without identification; acceptable degradation.                                |
| Malformed plan markdown            | Parser fails gracefully; raw markdown render with a small "couldn't parse phases" warning; daemon unaffected.                  |
| `~/.ccdash/auth` missing/unreadable | Daemon regenerates token on next start; logs warning.                                                                          |

### Client-side

| Failure mode                  | Behavior                                                                                                                          |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Daemon unreachable on start   | 3 retries with exponential backoff; if still failing, full diagnostic UI (socket path, last error, "view logs" button).            |
| Connection drops mid-session  | "Disconnected" banner; auto-reconnects in background; re-subscribes and reconciles state on reconnect.                            |
| Mirror target window closed   | Mirror window unsubscribes, reverts to independent mode with toast: "Mirrored window closed."                                     |

### Session lifecycle

| Event                                  | Behavior                                                                                                       |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `claude` exits inside tmux             | Tmux pane configured with `remain-on-exit on` so exit visible; session marked `state: exited`; relaunch action. |
| User kills tmux session externally     | Detected on next poll; broadcasts `session.removed`; all windows update.                                       |
| User renames tmux session externally   | Session_id unchanged → ccdash still tracks it; display name updated.                                            |

## 9. Testing

### 9.1 Unit (per crate)

- `ccdash-core` — serde round-trip for every protocol message and domain type.
- `ccdash-daemon` — plan parser (table-driven on real fixtures from existing superpowers plans); declared-port parser (fixtures for `package.json`/`.env`/`docker-compose.yml`/`Procfile`); auth token validation.

### 9.2 Integration

- **Daemon harness** — spin up daemon against a temp `$HOME`; black-box-test the JSON-RPC protocol over a temp socket.
- **Tmux integration** — CI creates a fresh tmux server (`tmux -L test-server -f /dev/null`); scenarios cover launch, kill, attach, external rename, server crash + reconnect.
- **Broadcast** — simulate two clients subscribing; trigger a state change; assert both receive the event in order.
- **Port conflict path** — launch project A on port 3000; attempt to launch project B (also declares 3000); assert `PortConflict` with valid `force_token`; verify each remediation path.

### 9.3 UI

- Svelte component tests (Vitest) for project list, port registry, plan view.
- Playwright e2e against a Tauri-in-dev-mode binary: launch session → assert visible → kill → assert removed. Single smoke test, not exhaustive.

### 9.4 Manual acceptance (pre-ship)

- Fresh `brew install ccdash` on a clean macOS user account → first-run wizard → register at least one project → launch a real `claude` session → kill it → quit app → relaunch → state recovered.
- Same on a fresh Ubuntu VM via `.deb`.
- Open two app windows, mirror one to the other, verify project/view/sessions sync; verify terminal pane content matches because both attach to same tmux session.

## 10. Open Items Deferred to v2+

- Code-signing + notarization for distribution.
- Windows support.
- Cloud sync of project registry across machines.
- File browser inside dashboard.
- Per-client capability tokens / remote daemon access.
- Live edit of plan markdown from the dashboard (deliberately deferred to avoid dueling-writers with the user's editor).
