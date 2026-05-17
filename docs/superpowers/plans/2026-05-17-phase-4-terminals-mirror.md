# ccdash Phase 4 — Embedded Terminals + Multi-Window Mirror Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Live event subscription from daemon, embedded interactive terminals via xterm.js + portable-pty wrapping `tmux attach-session`, and multi-window support with optional mirror mode.

**Architecture:** The Tauri Rust process owns one pty per terminal pane, spawned via `portable-pty` running `tmux attach-session -t <session_id>`. Bytes flow Rust↔xterm.js over Tauri events. The Tauri client subscribes to the daemon's broadcast (project/session updates), and the Rust side fans those events out to all windows. Mirror mode is a window-to-window state-sync built on Tauri's `emit_to` API — a "source" window publishes its UI state to a per-window event channel, and any "follower" window subscribes.

**Tech Stack:**
- `portable-pty` 0.8.x (cross-platform pty)
- `xterm` 5.x + `xterm-addon-fit` (frontend)
- `tokio` mpsc + Tauri's `App::emit` for byte streaming
- Tauri 2.x `WebviewWindow::emit_to` for cross-window IPC

**Spec reference:** `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`
**Predecessor:** Phase 3 complete; tag `phase-3-done`.

**Design choices (confirmed by user):**
- pty crate: `portable-pty` (mature, cross-platform).
- Live event delivery: Tauri events (frontend uses `@tauri-apps/api/event::listen`).
- Window-to-window: Tauri's `emit_to(window_label, ...)` instead of routing through the daemon.

---

## File Structure

```
ccdash/
├── Cargo.toml                                       # add portable-pty to workspace deps
├── apps/ccdash-ui/
│   ├── Cargo.toml                                   # add portable-pty
│   ├── src/
│   │   ├── main.rs                                  # register new commands + spawn event bridge
│   │   ├── commands.rs                              # add terminal + window commands
│   │   ├── event_bridge.rs                          # NEW — daemon notifications -> Tauri events
│   │   ├── pty.rs                                   # NEW — portable-pty session manager
│   │   └── windows.rs                               # NEW — open_new_window + mirror helpers
│   └── ui/
│       ├── package.json                             # add xterm, xterm-addon-fit
│       └── src/
│           ├── lib/
│           │   ├── stores.ts                        # add ptySessions, mirrorTarget stores
│           │   ├── tauri.ts                         # add terminal + window invokes + event listeners
│           │   ├── windowSync.ts                    # NEW — mirror-mode subscribe/publish helpers
│           │   └── components/
│           │       └── Terminal.svelte              # NEW — xterm.js wrapper
│           └── routes/
│               └── +page.svelte                     # wire live events + Terminal panel + window menu
```

---

## Task A1: Add daemon notification → Tauri event bridge

Subscribes the shared Client to daemon broadcasts and emits Tauri events to all windows.

**Files:**
- Create: `apps/ccdash-ui/src/event_bridge.rs`
- Modify: `apps/ccdash-ui/src/main.rs`

- [ ] **Step 1: Write `event_bridge.rs`**

```rust
//! Subscribes the shared Client to daemon broadcast notifications and
//! re-emits them as Tauri events (`daemon-event`) so all windows hear them.

use crate::client_state::ClientState;
use ccdash_core::protocol::Topic;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, warn};

/// Long-lived task spawned at startup.
/// Sends `subscribe { topics: [projects, sessions, ports, plans] }` then
/// forwards each incoming notification as a Tauri event with the same
/// payload shape: `{ method: string, params: any }`.
pub async fn run(app: AppHandle) {
    // Wait until connect_and_handshake has succeeded (the connection is created
    // there). Poll the state every 100ms for up to 30s.
    let state: tauri::State<'_, ClientState> = app.state();
    let mut waited = 0u64;
    loop {
        if state.inner.lock().await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        waited += 100;
        if waited >= 30_000 {
            warn!("daemon event bridge: client never connected — giving up");
            return;
        }
    }

    // Send subscribe.
    {
        let mut guard = state.inner.lock().await;
        let client = guard.as_mut().unwrap();
        match client
            .subscribe(vec![Topic::Projects, Topic::Sessions, Topic::Ports, Topic::Plans])
            .await
        {
            Ok(resp) if resp.error.is_some() => {
                warn!("subscribe error: {:?}", resp.error);
                return;
            }
            Err(e) => {
                warn!("subscribe failed: {}", e);
                return;
            }
            _ => {}
        }
    }

    // Forwarding loop: pull notifications, emit to all windows.
    loop {
        let next = {
            let mut guard = state.inner.lock().await;
            let client = guard.as_mut().unwrap();
            client.next_notification().await
        };
        match next {
            Ok(v) => {
                let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                debug!(method = %method, "forwarding daemon notification");
                if let Err(e) = app.emit("daemon-event", serde_json::json!({"method": method, "params": params})) {
                    warn!("emit failed: {}", e);
                }
            }
            Err(e) => {
                warn!("notification read failed: {} — bridge exiting", e);
                return;
            }
        }
    }
}
```

- [ ] **Step 2: Wire bridge into `main.rs`**

Replace the body of `main()` to also spawn the bridge after the app starts:

```rust
//! ccdash desktop UI entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_state;
mod commands;
mod event_bridge;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(client_state::ClientState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                event_bridge::run(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_and_handshake,
            commands::project_list,
            commands::session_list,
            commands::ports_list,
            commands::plans_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p ccdash-ui
```

Expected: SUCCESS.

- [ ] **Step 4: Commit**

```bash
git add apps/ccdash-ui/src/event_bridge.rs apps/ccdash-ui/src/main.rs
git commit -m "ccdash-ui: daemon notification -> Tauri event bridge"
```

---

## Task B1: Add `portable-pty` and pty manager

**Files:**
- Modify: workspace `Cargo.toml`
- Modify: `apps/ccdash-ui/Cargo.toml`
- Create: `apps/ccdash-ui/src/pty.rs`

- [ ] **Step 1: Workspace dep**

Edit root `Cargo.toml`, add under `[workspace.dependencies]`:

```toml
portable-pty = "0.8"
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 2: Crate dep**

Edit `apps/ccdash-ui/Cargo.toml`, add to `[dependencies]`:

```toml
portable-pty = { workspace = true }
uuid = { workspace = true }
```

- [ ] **Step 3: Write `pty.rs`**

```rust
//! Per-terminal pty manager.
//!
//! Each call to `open()` spawns a fresh pty, runs the requested command (typically
//! `tmux attach-session -t <session_id>`), and starts two tokio tasks:
//!   - reader: pull bytes from pty master, emit `terminal-output::<id>` events
//!   - writer: receive bytes via tokio mpsc, write to pty master
//!
//! The PtyHandle keeps the writer alive; dropping it kills the child.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub struct PtyHandle {
    pub id: String,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    #[allow(dead_code)]
    pub child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

pub struct PtyManager {
    pub handles: Mutex<HashMap<String, PtyHandle>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Spawn a fresh pty running `cmd` with optional initial size. Returns the
    /// terminal id used in subsequent `write`/`resize`/`close` calls.
    pub async fn open(
        &self,
        app: AppHandle,
        cmd: Vec<String>,
        rows: u16,
        cols: u16,
    ) -> Result<String, String> {
        if cmd.is_empty() {
            return Err("empty command".into());
        }
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty: {}", e))?;

        let mut builder = CommandBuilder::new(&cmd[0]);
        for arg in &cmd[1..] {
            builder.arg(arg);
        }
        builder.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(builder).map_err(|e| format!("spawn: {}", e))?;
        // drop slave so child holds the only ref
        drop(pair.slave);

        let id = uuid::Uuid::new_v4().to_string();
        let master = Arc::new(Mutex::new(pair.master));
        let writer = {
            let master_guard = master.lock().await;
            let w = master_guard.take_writer().map_err(|e| format!("take_writer: {}", e))?;
            Arc::new(Mutex::new(w))
        };

        // Spawn reader: take a clone of the master to read bytes.
        let reader_master = master.clone();
        let reader_id = id.clone();
        let reader_app = app.clone();
        std::thread::spawn(move || {
            // We can't move the master across threads safely via tokio::Mutex
            // because portable-pty's Reader is sync. Workaround: lock briefly
            // to get a separate reader handle.
            let reader_handle = {
                let mut master = futures::executor::block_on(reader_master.lock());
                match master.try_clone_reader() {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("try_clone_reader: {}", e);
                        return;
                    }
                }
            };
            run_reader_loop(reader_handle, reader_id, reader_app);
        });

        let handle = PtyHandle {
            id: id.clone(),
            writer,
            master,
            child: Arc::new(Mutex::new(child)),
        };
        self.handles.lock().await.insert(id.clone(), handle);
        Ok(id)
    }

    pub async fn write(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let handles = self.handles.lock().await;
        let h = handles.get(id).ok_or_else(|| format!("no such terminal: {}", id))?;
        let mut w = h.writer.lock().await;
        w.write_all(bytes).map_err(|e| format!("write: {}", e))?;
        w.flush().map_err(|e| format!("flush: {}", e))?;
        Ok(())
    }

    pub async fn resize(&self, id: &str, rows: u16, cols: u16) -> Result<(), String> {
        let handles = self.handles.lock().await;
        let h = handles.get(id).ok_or_else(|| format!("no such terminal: {}", id))?;
        let m = h.master.lock().await;
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("resize: {}", e))?;
        Ok(())
    }

    pub async fn close(&self, id: &str) -> Result<(), String> {
        let mut handles = self.handles.lock().await;
        if let Some(h) = handles.remove(id) {
            let mut child = h.child.lock().await;
            let _ = child.kill();
        }
        Ok(())
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

fn run_reader_loop(mut reader: Box<dyn Read + Send>, id: String, app: AppHandle) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                debug!(id = %id, "pty eof");
                let _ = app.emit(&format!("terminal-eof::{}", id), serde_json::json!({}));
                return;
            }
            Ok(n) => {
                let slice = &buf[..n];
                // xterm.js wants raw bytes — send as a Vec<u8> via JSON array (Tauri
                // will base64 if too large; for now JSON array of bytes works).
                let v: Vec<u8> = slice.to_vec();
                if let Err(e) = app.emit(&format!("terminal-output::{}", id), v) {
                    warn!("emit terminal-output failed: {}", e);
                    return;
                }
            }
            Err(e) => {
                warn!(id = %id, error = %e, "pty read error");
                let _ = app.emit(&format!("terminal-eof::{}", id), serde_json::json!({}));
                return;
            }
        }
    }
}
```

Note: the reader uses `futures::executor::block_on` to lock the tokio mutex briefly from a std thread. Add `futures = "0.3"` to workspace deps.

- [ ] **Step 4: Add `futures` to workspace deps**

Edit root `Cargo.toml`, under `[workspace.dependencies]`:
```toml
futures = "0.3"
```

Edit `apps/ccdash-ui/Cargo.toml`, under `[dependencies]`:
```toml
futures = { workspace = true }
```

- [ ] **Step 5: Build**

```bash
cargo build -p ccdash-ui
```

Expected: SUCCESS. (Module isn't wired into Tauri yet; just verifying it compiles.)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml apps/ccdash-ui/Cargo.toml apps/ccdash-ui/src/pty.rs
git commit -m "ccdash-ui: portable-pty Manager (open/write/resize/close)"
```

---

## Task B2: Wire PtyManager + terminal commands

**Files:**
- Modify: `apps/ccdash-ui/src/commands.rs`
- Modify: `apps/ccdash-ui/src/main.rs`

- [ ] **Step 1: Add Tauri command wrappers**

Edit `apps/ccdash-ui/src/commands.rs`. Add the following at the end of the file:

```rust
// === Terminal commands ===

use crate::pty::PtyManager;

#[tauri::command]
pub async fn terminal_open(
    app: tauri::AppHandle,
    pty: tauri::State<'_, PtyManager>,
    command: Vec<String>,
    rows: u16,
    cols: u16,
) -> Result<String, String> {
    pty.open(app, command, rows, cols).await
}

#[tauri::command]
pub async fn terminal_write(
    pty: tauri::State<'_, PtyManager>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    pty.write(&id, &data).await
}

#[tauri::command]
pub async fn terminal_resize(
    pty: tauri::State<'_, PtyManager>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    pty.resize(&id, rows, cols).await
}

#[tauri::command]
pub async fn terminal_close(pty: tauri::State<'_, PtyManager>, id: String) -> Result<(), String> {
    pty.close(&id).await
}
```

- [ ] **Step 2: Register the PtyManager + commands**

Edit `apps/ccdash-ui/src/main.rs`. Add `mod pty;` and register state + new commands:

```rust
//! ccdash desktop UI entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_state;
mod commands;
mod event_bridge;
mod pty;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(client_state::ClientState::new())
        .manage(pty::PtyManager::new())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                event_bridge::run(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_and_handshake,
            commands::project_list,
            commands::session_list,
            commands::ports_list,
            commands::plans_get,
            commands::terminal_open,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p ccdash-ui
```

Expected: SUCCESS.

- [ ] **Step 4: Commit**

```bash
git add apps/ccdash-ui/src/commands.rs apps/ccdash-ui/src/main.rs
git commit -m "ccdash-ui: wire pty manager + terminal_open/write/resize/close commands"
```

---

## Task C1: xterm.js frontend integration

**Files:**
- Modify: `apps/ccdash-ui/ui/package.json` (add xterm + addon-fit)
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts` (add invoke helpers + event listener types)
- Create: `apps/ccdash-ui/ui/src/lib/components/Terminal.svelte`

- [ ] **Step 1: Add xterm deps**

Edit `apps/ccdash-ui/ui/package.json` — update `"dependencies"`:

```json
"dependencies": {
  "@tauri-apps/api": "^2",
  "@xterm/xterm": "^5.5",
  "@xterm/addon-fit": "^0.10"
}
```

Run:
```bash
cd apps/ccdash-ui/ui && pnpm install && cd ../../..
```

- [ ] **Step 2: Extend `tauri.ts` with terminal invokes**

Append to `apps/ccdash-ui/ui/src/lib/tauri.ts`:

```typescript
export const terminal = {
  open: (command: string[], rows: number, cols: number) =>
    invoke<string>('terminal_open', { command, rows, cols }),
  write: (id: string, data: Uint8Array) =>
    invoke<void>('terminal_write', { id, data: Array.from(data) }),
  resize: (id: string, rows: number, cols: number) =>
    invoke<void>('terminal_resize', { id, rows, cols }),
  close: (id: string) => invoke<void>('terminal_close', { id }),
};
```

- [ ] **Step 3: Write the Terminal component**

Create `apps/ccdash-ui/ui/src/lib/components/Terminal.svelte`:

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Terminal as XTerm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { listen } from '@tauri-apps/api/event';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { terminal } from '$lib/tauri';
  import '@xterm/xterm/css/xterm.css';

  /** Command to run inside the pty (e.g. ['tmux','attach-session','-t','$3']). */
  export let command: string[];

  let containerEl: HTMLDivElement | undefined = $state();
  let xterm: XTerm | undefined;
  let fit: FitAddon | undefined;
  let ptyId: string | null = null;
  let unlistenOutput: UnlistenFn | null = null;
  let unlistenEof: UnlistenFn | null = null;

  onMount(async () => {
    if (!containerEl) return;
    xterm = new XTerm({
      convertEol: true,
      fontFamily: 'ui-monospace, "SF Mono", Monaco, monospace',
      fontSize: 13,
      theme: {
        background: '#1a1b1e',
        foreground: '#e6e6e6',
      },
    });
    fit = new FitAddon();
    xterm.loadAddon(fit);
    xterm.open(containerEl);
    fit.fit();

    const { rows, cols } = xterm;
    ptyId = await terminal.open(command, rows, cols);

    unlistenOutput = await listen<number[]>(`terminal-output::${ptyId}`, (e) => {
      const bytes = new Uint8Array(e.payload);
      xterm!.write(bytes);
    });
    unlistenEof = await listen(`terminal-eof::${ptyId}`, () => {
      xterm!.write('\r\n\x1b[31m[process exited]\x1b[0m\r\n');
    });

    xterm.onData((data) => {
      if (ptyId) terminal.write(ptyId, new TextEncoder().encode(data));
    });

    xterm.onResize(({ rows, cols }) => {
      if (ptyId) terminal.resize(ptyId, rows, cols);
    });

    const onResize = () => fit?.fit();
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  onDestroy(async () => {
    unlistenOutput?.();
    unlistenEof?.();
    if (ptyId) await terminal.close(ptyId).catch(() => {});
    xterm?.dispose();
  });
</script>

<div bind:this={containerEl} class="terminal"></div>

<style>
  .terminal {
    width: 100%;
    height: 100%;
    background: #1a1b1e;
    padding: 4px;
  }
</style>
```

- [ ] **Step 4: Build the frontend**

```bash
cd apps/ccdash-ui/ui && pnpm run build && cd ../../..
```

Expected: SUCCESS.

- [ ] **Step 5: Commit**

```bash
git add apps/ccdash-ui/ui/package.json apps/ccdash-ui/ui/src/lib
git commit -m "ccdash-ui frontend: xterm.js Terminal component + invoke helpers"
```

---

## Task C2: Add "Open terminal" panel + wire live events

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts`
- Modify: `apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte`
- Modify: `apps/ccdash-ui/ui/src/routes/+page.svelte`

- [ ] **Step 1: Extend stores**

Append to `apps/ccdash-ui/ui/src/lib/stores.ts`:

```typescript
export type TerminalPaneState = {
  command: string[];
  // 'live' = full attach; 'monitor' would be pipe-pane (Phase 4 ships live only)
  mode: 'live';
};
export const terminalPane = writable<TerminalPaneState | null>(null);
```

- [ ] **Step 2: Update SessionsView to add an attach button**

Replace `apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte` with:

```svelte
<script lang="ts">
  import { sessions, terminalPane } from '$lib/stores';

  function attach(sessionId: string) {
    terminalPane.set({
      command: ['tmux', 'attach-session', '-t', sessionId],
      mode: 'live',
    });
  }
</script>

<div>
  <table>
    <thead>
      <tr><th>tmux id</th><th>name</th><th>pid</th><th>cwd</th><th>state</th><th></th></tr>
    </thead>
    <tbody>
      {#each $sessions as s (s.tmux_session_id)}
        <tr>
          <td><code>{s.tmux_session_id}</code></td>
          <td>{s.name}</td>
          <td>{s.pid}</td>
          <td><code>{s.cwd}</code></td>
          <td class={s.state === 'running' ? 'running' : 'exited'}>{s.state}</td>
          <td><button on:click={() => attach(s.tmux_session_id)}>Attach</button></td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty">(no sessions)</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  th { color: var(--fg-dim); font-weight: 500; font-size: 12px; text-transform: uppercase; letter-spacing: 1px; }
  .running { color: var(--success); }
  .exited { color: var(--fg-dim); }
  .empty { text-align: center; color: var(--fg-dim); font-style: italic; padding: 24px; }
</style>
```

- [ ] **Step 3: Wire Terminal panel + live events in `+page.svelte`**

Replace `apps/ccdash-ui/ui/src/routes/+page.svelte` with:

```svelte
<script lang="ts">
  import '$lib/theme.css';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { tauri } from '$lib/tauri';
  import {
    activeTab,
    connectError,
    connected,
    plans,
    ports,
    projects,
    selectedProjectId,
    sessions,
    terminalPane,
  } from '$lib/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionsView from '$lib/components/SessionsView.svelte';
  import PortsView from '$lib/components/PortsView.svelte';
  import PlansView from '$lib/components/PlansView.svelte';
  import Terminal from '$lib/components/Terminal.svelte';

  async function refreshTopLevel() {
    try {
      const { projects: ps } = await tauri.projectList();
      projects.set(ps);
      if ($selectedProjectId === null && ps.length > 0) {
        selectedProjectId.set(ps[0].id);
      }
      const { sessions: ss } = await tauri.sessionList();
      sessions.set(ss);
      const ports_ = await tauri.portsList();
      ports.set(ports_);
    } catch (e) {
      connectError.set(String(e));
    }
  }

  async function refreshPlansFor(pid: string | null) {
    if (!pid) {
      plans.set([]);
      return;
    }
    try {
      const { plans: ps } = await tauri.plansGet(pid);
      plans.set(ps);
    } catch (e) {
      console.warn('plans.get failed', e);
      plans.set([]);
    }
  }

  $: refreshPlansFor($selectedProjectId);

  onMount(async () => {
    try {
      await tauri.connect();
      connected.set(true);
      await refreshTopLevel();
    } catch (e) {
      connectError.set(String(e));
    }
    // Live event subscription: refresh affected topic on daemon notification.
    const unlisten = await listen<{ method: string; params: any }>('daemon-event', (e) => {
      const m = e.payload.method;
      if (m.startsWith('project.') || m.startsWith('projects.')) {
        tauri.projectList().then(({ projects: ps }) => projects.set(ps)).catch(() => {});
      } else if (m.startsWith('session.') || m.startsWith('sessions.')) {
        tauri.sessionList().then(({ sessions: ss }) => sessions.set(ss)).catch(() => {});
      }
    });
    return () => unlisten();
  });

  function setTab(t: 'sessions' | 'ports' | 'plans') {
    activeTab.set(t);
  }

  function closeTerminal() {
    terminalPane.set(null);
  }
</script>

<div class="root">
  <Sidebar />
  <main>
    <header>
      <div class="tabs">
        <button class:active={$activeTab === 'sessions'} on:click={() => setTab('sessions')}>Sessions</button>
        <button class:active={$activeTab === 'ports'} on:click={() => setTab('ports')}>Ports</button>
        <button class:active={$activeTab === 'plans'} on:click={() => setTab('plans')}>Plans</button>
      </div>
      <div class="status">
        {#if !$connected}
          <span class="error">{$connectError ?? 'connecting...'}</span>
        {/if}
      </div>
    </header>
    <section class="content">
      {#if $activeTab === 'sessions'}
        <SessionsView />
      {:else if $activeTab === 'ports'}
        <PortsView />
      {:else}
        <PlansView />
      {/if}
    </section>
    {#if $terminalPane}
      <section class="terminal-panel">
        <div class="terminal-header">
          <span>Terminal: {$terminalPane.command.join(' ')}</span>
          <button on:click={closeTerminal}>Close</button>
        </div>
        <div class="terminal-host">
          {#key $terminalPane.command.join(' ')}
            <Terminal command={$terminalPane.command} />
          {/key}
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  .root { display: flex; height: 100vh; }
  main { flex: 1; display: flex; flex-direction: column; }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .tabs { display: flex; gap: 4px; }
  .tabs button { border-radius: 4px; }
  .tabs button.active { background: var(--accent-bg); color: var(--accent); border-color: var(--accent); }
  .status .error { color: var(--danger); font-size: 12px; }
  .content { flex: 1; overflow-y: auto; min-height: 200px; }
  .terminal-panel {
    height: 340px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: #1a1b1e;
  }
  .terminal-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 6px 12px; background: var(--bg-elev); border-bottom: 1px solid var(--border);
    font-family: var(--mono); font-size: 12px; color: var(--fg-dim);
  }
  .terminal-host { flex: 1; overflow: hidden; }
</style>
```

- [ ] **Step 4: Build the frontend**

```bash
cd apps/ccdash-ui/ui && pnpm run build && cd ../../..
```

- [ ] **Step 5: Build Tauri binary**

```bash
cargo build -p ccdash-ui
```

- [ ] **Step 6: Commit**

```bash
git add apps/ccdash-ui/ui
git commit -m "ccdash-ui frontend: attach-button + terminal panel + live event refresh"
```

---

## Task D1: Multi-window — "Open new window" command

**Files:**
- Create: `apps/ccdash-ui/src/windows.rs`
- Modify: `apps/ccdash-ui/src/commands.rs`
- Modify: `apps/ccdash-ui/src/main.rs`
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts`
- Modify: `apps/ccdash-ui/ui/src/routes/+page.svelte`

- [ ] **Step 1: Write `windows.rs`**

```rust
//! Helpers for creating + addressing additional app windows.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_new_window(app: &AppHandle) -> Result<(), String> {
    let count = app.webview_windows().len();
    let label = format!("ccdash-{}", count + 1);
    WebviewWindowBuilder::new(app, &label, WebviewUrl::default())
        .title(format!("ccdash ({})", count + 1))
        .inner_size(1100.0, 720.0)
        .build()
        .map_err(|e| format!("window: {}", e))?;
    Ok(())
}
```

- [ ] **Step 2: Add `open_new_window` command**

Append to `apps/ccdash-ui/src/commands.rs`:

```rust
#[tauri::command]
pub async fn open_new_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::windows::open_new_window(&app)
}

#[tauri::command]
pub async fn list_windows(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Manager;
    Ok(app
        .webview_windows()
        .into_keys()
        .collect::<Vec<_>>())
}

/// Emit `window-state-broadcast::<from>` carrying arbitrary JSON state to ALL
/// windows. Follower windows filter to a chosen source.
#[tauri::command]
pub async fn publish_window_state(
    app: tauri::AppHandle,
    from: String,
    state: serde_json::Value,
) -> Result<(), String> {
    use tauri::Emitter;
    app.emit(&format!("window-state-broadcast::{}", from), state)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register `mod windows;` + new commands in main.rs**

Edit `apps/ccdash-ui/src/main.rs`. Add `mod windows;` and append to `invoke_handler`:

```rust
commands::open_new_window,
commands::list_windows,
commands::publish_window_state,
```

- [ ] **Step 4: Add JS bindings**

Append to `apps/ccdash-ui/ui/src/lib/tauri.ts`:

```typescript
import { listen as tauriListen, UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const windows = {
  openNew: () => invoke<void>('open_new_window'),
  list: () => invoke<string[]>('list_windows'),
  publishState: (from: string, state: any) =>
    invoke<void>('publish_window_state', { from, state }),
  listenState: (from: string, handler: (state: any) => void): Promise<UnlistenFn> =>
    tauriListen<any>(`window-state-broadcast::${from}`, (e) => handler(e.payload)),
  currentLabel: () => getCurrentWindow().label,
};
```

- [ ] **Step 5: Add a "New window" button in the page header**

Edit `apps/ccdash-ui/ui/src/routes/+page.svelte`. In the `<header>` `.status` div, add:

```svelte
<button on:click={() => tauri.windows ? tauri.windows.openNew() : void 0}>+ New window</button>
```

Or — to keep the import structure clean — at the top of the script, add `import { windows as windowsApi } from '$lib/tauri';` and use `<button on:click={() => windowsApi.openNew()}>+ New window</button>` in the header.

Actually, since `tauri.ts` exports `windows` directly, the cleanest edit is to import it once. Add at the top of the script:

```typescript
import { windows as windowsApi } from '$lib/tauri';
```

And in the header markup, just before `.status`:

```svelte
<div class="actions">
  <button on:click={() => windowsApi.openNew()}>+ New window</button>
</div>
```

Plus a small style block:

```css
.actions { display: flex; gap: 4px; margin-left: auto; margin-right: 12px; }
```

- [ ] **Step 6: Build everything**

```bash
cd apps/ccdash-ui/ui && pnpm run build && cd ../../..
cargo build -p ccdash-ui
```

Expected: SUCCESS.

- [ ] **Step 7: Commit**

```bash
git add apps/ccdash-ui/src/windows.rs apps/ccdash-ui/src/commands.rs apps/ccdash-ui/src/main.rs apps/ccdash-ui/ui
git commit -m "ccdash-ui: + open_new_window/list_windows/publish_window_state + UI new-window button"
```

---

## Task E1: Mirror mode — window-to-window state sync

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/windowSync.ts`
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts`
- Modify: `apps/ccdash-ui/ui/src/routes/+page.svelte`

- [ ] **Step 1: Add `mirrorTarget` store**

Append to `apps/ccdash-ui/ui/src/lib/stores.ts`:

```typescript
/** When set to a window label, this window mirrors that one's UI state. */
export const mirrorTarget = writable<string | null>(null);
```

- [ ] **Step 2: Write `windowSync.ts`**

Create `apps/ccdash-ui/ui/src/lib/windowSync.ts`:

```typescript
import { get } from 'svelte/store';
import { windows as windowsApi } from './tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  activeTab,
  mirrorTarget,
  selectedProjectId,
} from './stores';

type MirroredState = {
  selectedProjectId: string | null;
  activeTab: 'sessions' | 'ports' | 'plans';
};

let publishHandle: number | null = null;
let unlistenMirror: UnlistenFn | null = null;

/// Start publishing this window's relevant UI state every 250ms.
export function startPublishing() {
  const myLabel = windowsApi.currentLabel();
  publishHandle = window.setInterval(() => {
    const state: MirroredState = {
      selectedProjectId: get(selectedProjectId),
      activeTab: get(activeTab),
    };
    windowsApi.publishState(myLabel, state).catch(() => {});
  }, 250);
}

export function stopPublishing() {
  if (publishHandle !== null) {
    clearInterval(publishHandle);
    publishHandle = null;
  }
}

export async function startMirroring(target: string) {
  if (unlistenMirror) unlistenMirror();
  unlistenMirror = await windowsApi.listenState(target, (state: MirroredState) => {
    if (state.selectedProjectId !== undefined) selectedProjectId.set(state.selectedProjectId);
    if (state.activeTab !== undefined) activeTab.set(state.activeTab);
  });
  mirrorTarget.set(target);
}

export function stopMirroring() {
  if (unlistenMirror) {
    unlistenMirror();
    unlistenMirror = null;
  }
  mirrorTarget.set(null);
}
```

- [ ] **Step 3: Add mirror UI in `+page.svelte`**

Edit `+page.svelte`. Add imports at the top of the script:

```typescript
import { startPublishing, stopPublishing, startMirroring, stopMirroring } from '$lib/windowSync';
import { mirrorTarget } from '$lib/stores';
```

Inside `onMount`, after the existing daemon-event setup, add:

```typescript
startPublishing();
const otherWindows = (await windowsApi.list()).filter((l) => l !== windowsApi.currentLabel());
// Expose `otherWindows` as a local-state value so UI can dropdown them.
otherWindowList.set(otherWindows);
```

…and add a local store at the top of the script:

```typescript
import { writable } from 'svelte/store';
const otherWindowList = writable<string[]>([]);
```

In the header markup, add the mirror dropdown next to `.actions`:

```svelte
<select bind:value={$mirrorTarget} on:change={(e) => {
  const v = (e.target as HTMLSelectElement).value;
  if (v) startMirroring(v); else stopMirroring();
}}>
  <option value="">— independent —</option>
  {#each $otherWindowList as w (w)}
    <option value={w}>follow {w}</option>
  {/each}
</select>
```

Return value of `onMount` cleanup should also call `stopPublishing()` and `stopMirroring()`. Update the cleanup:

```typescript
return () => {
  unlisten();
  stopPublishing();
  stopMirroring();
};
```

- [ ] **Step 4: Build + verify**

```bash
cd apps/ccdash-ui/ui && pnpm run build && cd ../../..
cargo build -p ccdash-ui
```

- [ ] **Step 5: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/windowSync.ts apps/ccdash-ui/ui/src/lib/stores.ts apps/ccdash-ui/ui/src/routes/+page.svelte
git commit -m "ccdash-ui: mirror mode (publish/listen window state via Tauri events)"
```

---

## Task F1: Phase 4 full verification + tag

- [ ] **Step 1: fmt + clippy**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both succeed.

- [ ] **Step 2: Full test run**

```bash
tmux kill-server 2>/dev/null; sleep 0.2
cargo test --workspace
```

Expected: existing 82 tests still pass. No new automated tests in Phase 4 (terminal + window behavior is end-to-end visual).

- [ ] **Step 3: Tag**

```bash
git tag phase-4-done
```

- [ ] **Step 4: Update execution log**

Append a "Phase 4 — Complete" section to `docs/superpowers/EXECUTION-LOG.md`. Note:
- pty crate is `portable-pty` (sync API; reader runs in a std thread).
- Reader-thread → tokio bridge uses `futures::executor::block_on` on the master lock.
- Live event subscription uses topic-keyed daemon notifications fanned out via Tauri's `app.emit`.
- Mirror mode uses Tauri events directly (`emit_to` would be more targeted; we use `emit` + per-source filter for simplicity).

```bash
git add docs/superpowers/EXECUTION-LOG.md
git commit -m "docs: phase-4 complete — terminals + multi-window mirror"
```

---

## What Phase 4 ships

- Live event subscription from daemon → Tauri events → frontend stores (no more 5s polling for the topics covered).
- Embedded terminals via `portable-pty` + xterm.js. Click "Attach" on a session row to attach to that tmux session inside the UI.
- Multi-window: "+ New window" button opens another instance against the same daemon (one shared `Client`).
- Mirror mode: each window can pick another window from a dropdown to follow its `selectedProjectId` + `activeTab`. Independent mode is default.

## What's NOT in Phase 4 (deferred)

- Monitor-mode terminals (read-only, `tmux pipe-pane`). Phase 4 ships live-attach only.
- Tier 3 mirror (scrollback / cursor sync) — emerges naturally via shared tmux session, no UI work needed.
- Session launch button in UI (still done via CLI in Phase 4).
- File browser (out of scope per spec §3).

## Self-Review

**Spec coverage:**
- §5.4 Tauri Rust = thin proxy; SvelteKit frontend; xterm.js + Rust pty bridge ✓
- §6.4 mirror semantics → E1 (Tier 2 with `selectedProjectId` + `activeTab`)
- §7.7 terminals visible in UI → C1+C2

**Placeholder scan:** none.

**Type consistency:** `MirroredState`, `TerminalPaneState` defined once. Tauri event names follow `<topic>::<id>` convention consistently.

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-phase-4-terminals-mirror.md`.**
