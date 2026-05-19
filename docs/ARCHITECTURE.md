# ccdash architecture

A guided tour for contributors. Full design rationale (problem framing,
non-goals, alternatives considered) lives in
[`docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`](./superpowers/specs/2026-05-17-cc-dashboard-design.md).
This document is the *current state* as of v1.0.

## High-level

```
   ┌──────────────────────────────────────────────────────────────┐
   │  ccdash-daemon (long-lived Rust process)                     │
   │                                                              │
   │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐  │
   │  │ projects/   │  │ sessions/    │  │ ports/              │  │
   │  │ Registry +  │  │ Manager      │  │ Registry (lsof) +   │  │
   │  │ scanner     │  │ + tmux       │  │ declared parsers    │  │
   │  └─────────────┘  └──────────────┘  └─────────────────────┘  │
   │                                                              │
   │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐  │
   │  │ plans/      │  │ broadcast/   │  │ rpc/                │  │
   │  │ Manager     │  │ Bus          │  │ dispatch +          │  │
   │  │ + parser    │  │              │  │ handlers + server   │  │
   │  └─────────────┘  └──────────────┘  └─────────────────────┘  │
   │                                                              │
   │  state.rs: AppState (Arcs all the way down — cheap to clone) │
   └──────────────────────────────────────────────────────────────┘
                      ▲                          ▲
                      │   JSON-RPC 2.0           │   JSON-RPC 2.0
                      │   (Unix socket)          │
   ┌──────────────────┴───┐     ┌────────────────┴─────────────────┐
   │  ccdash-cli           │     │  ccdash-ui (Tauri 2)             │
   │  ────────────         │     │  ────────────                    │
   │  Rust binary using    │     │  Rust backend = thin proxy       │
   │  ccdash-core::Client  │     │  Tauri 2 commands invoke the     │
   │  Subcommands → call → │     │  same ccdash-core::Client +      │
   │  print result         │     │  emit broadcast events to JS     │
   │                       │     │                                  │
   │                       │     │  Frontend: plain Vite + Svelte 5 │
   │                       │     │  components, no SvelteKit        │
   └───────────────────────┘     └──────────────────────────────────┘
```

## Crates

### `crates/ccdash-core`

Pure library. Has zero ambient OS dependencies — safe to depend on from
any other crate, including tests.

- `protocol.rs` — JSON-RPC 2.0 envelope + every RPC method's `*Params`
  and `*Result` type. Domain types are imported from `domain.rs`. The
  `PROTOCOL_VERSION` constant ratchets when the wire format breaks.
- `domain.rs` — `Project`, `Worktree`, `Session`, `PortBinding`, `Plan`,
  `PlanPhase`, `PlanTask`, etc. All serde-derived.
- `client.rs` — `Client` struct that wraps the Unix-socket transport
  with the handshake + call/response loop. Reused by both `ccdash-cli`
  and `apps/ccdash-ui`.
- `auth.rs` — generates / loads the shared-secret token at
  `~/.ccdash/auth` (mode `0600`).
- `paths.rs` — OS-appropriate paths for the data dir and socket.

### `crates/ccdash-daemon`

The long-lived service. Its lifetime is bound to launchd / systemd, not
to any user session.

#### Module map

| Path | Responsibility |
|---|---|
| `main.rs` | Bootstrap, signal handling, server loop. |
| `state.rs` | `AppState` holding Arcs of every subsystem. Cheap to clone. Carries `first_run_pending: AtomicBool`. |
| `projects/registry.rs` | Persistent `BTreeMap<ProjectId, ProjectRow>` with `order: u32` for stable user-controlled ordering. |
| `projects/scanner.rs` | Recursive git-repo scan with `SKIP_DIRS` list, depth limit 4. |
| `sessions/manager.rs` | Joins live tmux state with `~/.ccdash/sessions.toml`. Keyed on tmux's stable `session_id` (`$N`), not name. |
| `tmux/mod.rs` | `tmux new-session -d`, `tmux kill-session`, `tmux display-message` shells. |
| `worktrees.rs` | `git worktree list --porcelain` parser. |
| `ports/lsof.rs` | `lsof -nP -iTCP -sTCP:LISTEN` shell + parser. macOS uses the system binary; on Linux it's a declared package dep. |
| `ports/declared.rs` | Per-project parsers for `package.json`, `.env`, `docker-compose.yml`, `Procfile`. |
| `ports/registry.rs` | Composite view + conflict detection. |
| `plans/mod.rs` | `pulldown-cmark` markdown parser + per-call refresh. |
| `rpc/server.rs` | Unix-socket listener + per-connection task. Translates `Event::*` into broadcast messages. |
| `rpc/dispatch.rs` | Method routing. New methods are added here + in `handlers.rs`. |
| `rpc/handlers.rs` | All RPC method implementations. |
| `broadcast.rs` | `tokio::sync::broadcast` event bus. |
| `tests/` | Integration tests against a spawned daemon over a temp socket. |

#### RPC surface (as of v1.0)

| Method | Params | Result |
|---|---|---|
| `handshake` | `{token, client}` | `{daemon_version, protocol_version}` |
| `subscribe` | `{topics}` | `{subscribed: true}` |
| `project.list` | `{}` | `{projects: [Project]}` |
| `project.add` | `{path, name?}` | `Project` |
| `project.remove` | `{id}` | `{ok: true}` |
| `project.reorder` | `{ids: [ProjectId]}` | `{ok: true}` |
| `session.list` | `{}` | `{sessions: [Session]}` |
| `session.launch` | `{project_id, worktree?, command?, force_token?}` | `{session: Session}` (or `error.code=-32002` w/ `PortConflictData`) |
| `session.kill` | `{tmux_session_id}` | `{ok: true}` |
| `ports.list` | `{}` | `{running, declared}` |
| `plans.get` | `{project_id}` | `{plans: [Plan]}` |
| `daemon.first_run_status` | `{}` | `{pending: bool}` |
| `daemon.first_run_complete` | `{}` | `{ok: true}` |
| `daemon.scan_paths` | `{roots: [PathBuf]}` | `{discovered: [DiscoveredRepo]}` |

#### Event bus

Broadcast events the daemon publishes:

- `project.added`, `project.removed`, `project.updated`
- `session.launched`, `session.removed`, `session.updated`
- `plan.updated`
- `ports.updated`

Subscribers (`ccdash-ui::event_bridge`) re-emit them as Tauri events keyed
by topic so all open windows can refresh in lockstep.

### `crates/ccdash-cli`

Thin wrapper. Each subcommand parses args with `clap`, builds a
`ccdash-core::Client`, calls the relevant RPC, and prints the result.
No state of its own.

### `apps/ccdash-ui`

Tauri 2 app — Rust backend + Svelte 5 frontend.

#### Rust backend (`src/`)

| File | Responsibility |
|---|---|
| `main.rs` | Tauri setup. Registers the dialog + shell plugins, the WindowEvent::Moved clamp, and the invoke handler list. |
| `commands.rs` | Tauri `#[tauri::command]` functions. Most are thin proxies to `ccdash_core::Client::call`. Returns `Result<Value, UiRpcError>` so daemon `error.data` flows to the frontend. |
| `client_state.rs` | Tauri-managed `Mutex<Option<Client>>` (the shared RPC connection). |
| `event_bridge.rs` | Subscribes to the daemon's broadcast bus and re-emits events to the frontend via `AppHandle::emit`. Owns its own `Client` to avoid blocking the shared one. |
| `pty.rs` | Per-terminal `PtyManager` using `portable-pty`. Spawns `tmux attach-session` and streams stdout bytes to the frontend via `terminal-output::{id}` events. |
| `windows.rs` | New-window factory. |
| `window_clamp.rs` | On `WindowEvent::Moved`, if window is fully off-screen, snap to primary monitor. |

#### Frontend (`ui/src/`)

| File | Responsibility |
|---|---|
| `App.svelte` | Top-level layout. Tab strip, top bar, terminal pane host, modal mounts (Launch, Welcome, CommandPalette). |
| `lib/stores.ts` | Svelte writables for projects, sessions, ports, plans, selectedProjectId, activeTab, terminalPane, mirrorTarget, reconnecting, nextRetryAt, detectedUrls. |
| `lib/tauri.ts` | Typed wrappers around every `invoke` call. Splits into `tauri.*`, `projectsApi.*`, `sessionsApi.*`, `daemonApi.*`. Type guards (`isUiRpcError`, `asPortConflict`). |
| `lib/reconnect.ts` | Exponential-backoff reconnect loop. |
| `lib/windowSync.ts` | 250ms tick publishes the current window's `{selectedProjectId, activeTab}` so mirror followers can subscribe. |
| `lib/keybinds.ts` | Global Cmd+N / Cmd+W / Cmd+K / Cmd+L handler. |
| `lib/theme.ts` | Auto/Dark/Light theme store with localStorage persistence + `matchMedia` watcher. |
| `lib/projectActions.ts` | `addProjectViaPicker()` — shared by Sidebar and EmptyState. |
| `lib/urlDetect.ts` | Pure regex for extracting loopback URLs from terminal output bytes. |
| `lib/format.ts` | `truncateBranch` middle-ellipsis helper. |
| `lib/components/Sidebar.svelte` | Projects list with add button, right-click context menu, drag-reorder. |
| `lib/components/SessionsView.svelte` | Sessions table with Attach/Kill + filter input >10. |
| `lib/components/PortsView.svelte` | Running + declared ports tables. |
| `lib/components/PlansView.svelte` | Plan markdown rendering + "Open in VS Code". |
| `lib/components/BrowserView.svelte` | Iframe preview with chrome bar + detected-URLs left rail. |
| `lib/components/Terminal.svelte` | `xterm.js` host. Hooks into output stream for URL detection. |
| `lib/components/LaunchDialog.svelte` | Launch modal + full conflict remediation. |
| `lib/components/WelcomeModal.svelte` | First-run scan + bulk-add flow. |
| `lib/components/CommandPalette.svelte` | Cmd+K type-ahead palette. |
| `lib/components/EmptyState.svelte` | Reusable empty-state with "Add a project" CTA. |

## Lifetimes & ownership

- **Daemon** outlives all UI windows. State lives in `~/.ccdash/`. Auth
  token is regenerated only if absent.
- **UI process** owns one `ccdash-core::Client` per app instance,
  shared across all windows via `tauri::State<ClientState>`.
- **Each terminal pane** owns one pty child process (running
  `tmux attach-session`). The pty is destroyed on `Terminal.svelte`'s
  `onDestroy`.
- **Each tmux session** outlives both the daemon and the UI. That's the
  whole point of going through tmux — Claude sessions survive process
  restarts.

## Data flow examples

### Launching a session (happy path)

1. UI: user clicks Launch session → opens LaunchDialog.
2. LaunchDialog: user picks project + worktree, clicks Launch.
3. Frontend → Tauri command `session_launch`.
4. Rust backend → `Client::call("session.launch", params)` over Unix socket.
5. Daemon: `dispatch.rs` routes to `handlers::handle_session_launch`.
6. Handler refreshes the port registry. No conflicts → tmux `new-session -d`.
7. Handler records the new session in `~/.ccdash/sessions.toml`, joins on
   tmux's stable `session_id`, broadcasts `Event::SessionLaunched`.
8. Every UI window receives the broadcast via `event_bridge`, re-fetches
   `session.list`, updates the `sessions` store.
9. LaunchDialog closes, the new row appears in SessionsView.

### Launching with a port conflict

5. Handler refreshes ports, finds a declared port already bound by another
   process. Generates a one-shot `force_token`, stashes it in
   `state.conflict_tokens`, returns `RpcError { code: -32002, data: PortConflictData {conflicts, force_token} }`.
6. Tauri command bridge propagates `error.data` (via the new `UiRpcError`
   type) to the frontend.
7. LaunchDialog renders the conflict list + "Launch anyway" button.
8. User clicks Launch anyway → re-submits with `force_token`.
9. Handler validates the token (one-shot — removed on use), proceeds.

### Multi-window mirror

1. Window A (the source) publishes `{selectedProjectId, activeTab}` to a
   local Tauri event channel `window-state-broadcast::A` every 250ms.
2. Window B (the follower) selects "follow A" from the mirror dropdown.
3. Window B subscribes to that channel, applies incoming state to its own
   Svelte stores.
4. Terminal panes auto-sync because both windows can attach to the same
   tmux `session_id` — tmux handles input/scroll/output sharing natively.

## Testing strategy

- **85 unit + integration tests** across the workspace. Run with `cargo
  test --workspace`. Single ignored test is the `tmux:smoke` integration
  test that needs an isolated tmux server.
- **Daemon integration harness** at `crates/ccdash-daemon/tests/common.rs`
  spawns the daemon binary against a temp `$HOME` and temp socket. Each
  test gets a fresh daemon instance.
- **Linux verification** via `packaging/linux/Dockerfile.test`: builds
  the daemon + CLI + core on ubuntu:22.04 and runs the full test suite.

## Adding a new RPC method

1. Add the `*Params` / `*Result` types to `crates/ccdash-core/src/protocol.rs`.
2. Add the handler in `crates/ccdash-daemon/src/rpc/handlers.rs`.
3. Add the route in `crates/ccdash-daemon/src/rpc/dispatch.rs`.
4. Write an integration test in `crates/ccdash-daemon/tests/<area>.rs`.
5. Add a Tauri command in `apps/ccdash-ui/src/commands.rs`.
6. Register the command in `apps/ccdash-ui/src/main.rs`'s `invoke_handler!`.
7. Add a typed wrapper in `apps/ccdash-ui/ui/src/lib/tauri.ts`.
8. Call it from a Svelte component.

The daemon-side flow follows a strict pattern: every handler returns
`Result<TypedResult, RpcError>` and `dispatch.rs` does the serde dance.
The Tauri-side flow propagates the `error.data` field via the
`UiRpcError` shape so error data can drive UI remediation.
