# Phase 7: Verify + polish existing features — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sand off the rough edges from phases 4-6 — full port-conflict remediation in the launch dialog, window position clamping, automatic reconnect when the daemon dies, and a code-level audit of the Attach / +New window / mirror paths.

**Architecture:** Three concrete code changes (error.data plumbing, window clamp listener, reconnect-aware Tauri call wrapper + UI banner). The click-test items get a code review pass; bugs found get fixed, otherwise the audit is logged in EXECUTION-LOG with "no defects found in code; visual verification pending."

**Tech Stack:** Tauri 2 `WindowEvent` API, monitor enumeration via `app.available_monitors()`, plain Svelte 5 + setTimeout-driven exponential backoff.

---

## File map

### Rust
- Modify: `apps/ccdash-ui/src/commands.rs` — return a structured `{ message, data }` from `call_method` so frontend can extract `error.data`. Tauri serializes Result<T, E> where E:Serialize fine; we'll return a custom struct.
- Modify: `apps/ccdash-ui/src/main.rs` — wire a `WindowEvent::Moved` handler on every window (existing + future).
- Create: `apps/ccdash-ui/src/window_clamp.rs` — small module with the clamp logic.
- Modify: `apps/ccdash-ui/src/windows.rs` — apply clamp logic when creating new windows too.

### Frontend
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts` — change `invoke` wrappers to handle the new structured error, expose `RpcError` type, add reconnect helpers.
- Create: `apps/ccdash-ui/ui/src/lib/reconnect.ts` — exponential-backoff reconnect loop.
- Modify: `apps/ccdash-ui/ui/src/lib/components/LaunchDialog.svelte` — full conflict-remediation UI: list of conflicting holders + "Launch anyway" button using the force_token.
- Modify: `apps/ccdash-ui/ui/src/App.svelte` — "Reconnecting…" banner with manual Retry button; wire reconnect.ts on connect failure / event-bus drop.

---

## Task 1: Plumb error.data through the Tauri command bridge

Tauri serializes a `Result<T, E>` where E implements `Serialize` as the rejection value; the frontend's `invoke()` rejection becomes that E. We return a struct `{ message: String, data: Option<Value> }` so the LaunchDialog can extract `PortConflictData`.

**Files:**
- Modify: `apps/ccdash-ui/src/commands.rs`

- [ ] **Step 1: Add a structured error type at the top of commands.rs**

After the `use` imports, add:

```rust
#[derive(Debug, serde::Serialize)]
pub struct UiRpcError {
    pub message: String,
    pub data: Option<Value>,
}

impl UiRpcError {
    fn message(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), data: None }
    }
}
```

- [ ] **Step 2: Change `call_method` signature + body**

Replace the existing `call_method` function with:

```rust
async fn call_method(
    state: &State<'_, ClientState>,
    method: &str,
    params: Value,
) -> Result<Value, UiRpcError> {
    let mut guard = state.inner.lock().await;
    let client = guard.as_mut().ok_or_else(|| {
        UiRpcError::message("daemon not connected — call connect_and_handshake first")
    })?;
    let resp = client
        .call(method, params)
        .await
        .map_err(|e| UiRpcError::message(e.to_string()))?;
    if let Some(err) = resp.error {
        return Err(UiRpcError {
            message: err.message,
            data: err.data,
        });
    }
    Ok(resp.result.unwrap_or(Value::Null))
}
```

- [ ] **Step 3: Change every `Result<Value, String>` to `Result<Value, UiRpcError>` for commands that call `call_method`**

The affected commands are: `project_list`, `session_list`, `ports_list`, `plans_get`, `project_add`, `project_remove`, `session_launch`, `session_kill`. Each just propagates the call_method error via `?` so the change is purely in the return type.

For `connect_and_handshake`, leave the `String` error — handshake doesn't go through call_method.

For `terminal_*`, `open_new_window`, `list_windows`, `publish_window_state`, `log_from_frontend`: leave `String` (they don't go through call_method).

- [ ] **Step 4: Verify**

```bash
cargo clippy -p ccdash-ui --all-targets -- -D warnings
```

- [ ] **Step 5: Frontend types**

In `apps/ccdash-ui/ui/src/lib/tauri.ts`, add at the top of the file (after the imports):

```typescript
/** Structured error returned by every RPC-proxying Tauri command. */
export interface UiRpcError {
  message: string;
  data?: unknown;
}

/** Tauri's invoke() rejects with the unwrapped error object. Type guard. */
export function isUiRpcError(e: unknown): e is UiRpcError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'message' in e &&
    typeof (e as { message: unknown }).message === 'string'
  );
}
```

- [ ] **Step 6: Build + commit**

```bash
pnpm --dir apps/ccdash-ui/ui run build
cargo clippy --workspace --all-targets -- -D warnings
git add apps/ccdash-ui/src/commands.rs apps/ccdash-ui/ui/src/lib/tauri.ts
git commit -m "phase-7: plumb error.data through Tauri command bridge"
```

---

## Task 2: Full conflict remediation in LaunchDialog

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/LaunchDialog.svelte`

- [ ] **Step 1: Extract conflict data on launch failure**

In the dialog's `submit()` function, when the catch fires, attempt to extract `PortConflictData` from the error object. If found, set state variables `conflicts` and `forceToken`. The UI then renders a remediation section with:
- A list of `{port, holder}` rows
- A "Launch anyway" button that re-submits with `forceToken` set
- A Cancel button

- [ ] **Step 2: Build + commit**

(Full component code in implementation below.)

---

## Task 3: Window position clamping

When a window's position is outside any visible monitor's frame, snap it to the primary monitor's frame.

**Files:**
- Create: `apps/ccdash-ui/src/window_clamp.rs`
- Modify: `apps/ccdash-ui/src/main.rs` — register `on_window_event` for clamping
- Modify: `apps/ccdash-ui/src/windows.rs` — clamp at creation too

- [ ] **Step 1: Write window_clamp.rs**

```rust
//! Window position clamping: when a window's position is outside any
//! available monitor, snap it back onto the primary monitor.

use tauri::{LogicalPosition, Manager, PhysicalPosition, WebviewWindow};
use tracing::{debug, warn};

/// Inspect every available monitor. If the given window's outer position
/// is fully outside all of them, move the window to the primary monitor's
/// center.
pub fn clamp_window_position(window: &WebviewWindow) {
    let pos = match window.outer_position() {
        Ok(p) => p,
        Err(e) => {
            warn!("clamp: outer_position failed: {}", e);
            return;
        }
    };
    let size = match window.outer_size() {
        Ok(s) => s,
        Err(e) => {
            warn!("clamp: outer_size failed: {}", e);
            return;
        }
    };
    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(e) => {
            warn!("clamp: available_monitors failed: {}", e);
            return;
        }
    };
    let any_overlap = monitors.iter().any(|mon| {
        let mp = mon.position();
        let ms = mon.size();
        let mx2 = mp.x + ms.width as i32;
        let my2 = mp.y + ms.height as i32;
        let wx2 = pos.x + size.width as i32;
        let wy2 = pos.y + size.height as i32;
        pos.x < mx2 && wx2 > mp.x && pos.y < my2 && wy2 > mp.y
    });
    if any_overlap {
        return;
    }
    let primary = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next());
    if let Some(mon) = primary {
        let mp = mon.position();
        let ms = mon.size();
        let target_x = mp.x + ((ms.width as i32 - size.width as i32) / 2).max(0);
        let target_y = mp.y + ((ms.height as i32 - size.height as i32) / 2).max(0);
        debug!("clamp: window off-screen, snapping to {}x{}", target_x, target_y);
        let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
    }
}

/// Suppress an unused-import warning for LogicalPosition (kept for future use
/// when DPI-aware clamping lands).
#[allow(dead_code)]
fn _unused(_: LogicalPosition<i32>) {}
```

- [ ] **Step 2: Wire it in main.rs**

In `apps/ccdash-ui/src/main.rs`, after `mod windows;` add `mod window_clamp;`. Then change the `tauri::Builder::default()...` chain to use `on_window_event`:

```rust
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Moved(_) = event {
                window_clamp::clamp_window_position(window);
            }
        })
```

- [ ] **Step 3: Clamp on new-window creation too**

In `apps/ccdash-ui/src/windows.rs::open_new_window`, after the window is built, call `crate::window_clamp::clamp_window_position(&w)` (assigning the build result to `w`).

- [ ] **Step 4: Build + verify**

```bash
cargo clippy -p ccdash-ui --all-targets -- -D warnings
```

---

## Task 4: Reconnect UX

When `connect_and_handshake` fails or any RPC fails with the "daemon not connected" message, kick off an exponential-backoff reconnect loop: 5s → 10s → 20s → 30s (cap). Show a "Reconnecting…" banner with a manual "Retry now" button.

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/reconnect.ts`
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts`
- Modify: `apps/ccdash-ui/ui/src/App.svelte`

- [ ] **Step 1: Add reconnect state to stores.ts**

Append:

```typescript
export const reconnecting = writable<boolean>(false);
export const nextRetryAt = writable<number | null>(null);
```

- [ ] **Step 2: Write reconnect.ts**

```typescript
import { tauri } from './tauri';
import { connected, connectError, reconnecting, nextRetryAt } from './stores';

const BASE_DELAY_MS = 5_000;
const MAX_DELAY_MS = 30_000;

let attempt = 0;
let timer: number | null = null;
let stopped = false;

function delayMs(): number {
  return Math.min(BASE_DELAY_MS * Math.pow(2, attempt), MAX_DELAY_MS);
}

async function tryConnect(refreshAll: () => Promise<void>): Promise<boolean> {
  try {
    await tauri.connect();
    await refreshAll();
    connected.set(true);
    connectError.set(null);
    reconnecting.set(false);
    nextRetryAt.set(null);
    attempt = 0;
    return true;
  } catch (e) {
    connectError.set(String(e));
    return false;
  }
}

export async function startReconnectLoop(refreshAll: () => Promise<void>) {
  stopped = false;
  reconnecting.set(true);
  connected.set(false);
  while (!stopped) {
    if (await tryConnect(refreshAll)) return;
    attempt++;
    const wait = delayMs();
    nextRetryAt.set(Date.now() + wait);
    await new Promise<void>((resolve) => {
      timer = window.setTimeout(() => {
        timer = null;
        resolve();
      }, wait);
    });
  }
}

export function retryNow() {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
}

export function stopReconnectLoop() {
  stopped = true;
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  reconnecting.set(false);
}
```

- [ ] **Step 3: Wire it in App.svelte**

Import `startReconnectLoop`, `retryNow`, `stopReconnectLoop`, and `reconnecting` from the right modules. In `onMount`, if `tauri.connect()` fails, call `startReconnectLoop(refreshTopLevel)` instead of just setting `connectError`. Add the banner in the template above `<main>`.

---

## Task 5: Code review of Attach / +New window / mirror paths

Read each path, note any obvious bugs, fix them. If nothing found, log the review.

- [ ] **Step 1: Attach path review**

Walk `Terminal.svelte::onMount` → `pty.rs::open` → `run_reader_loop`. Check:
- `terminal-output::{id}` emission shape matches frontend listener (Vec<u8> ↔ number[]). ✓
- `xterm.onData` encodes user input via TextEncoder. ✓
- Resize event fires `terminal.resize` which calls `master.resize(PtySize)`. ✓
- onDestroy calls `terminal.close` which kills the child. ✓
- Lifecycle race: if onDestroy fires before onMount finishes awaiting `terminal.open`, ptyId is still null → close is a no-op. ✓ safe.

- [ ] **Step 2: +New window review**

`windows.rs::open_new_window` builds a fresh `WebviewWindowBuilder`, no center call, no inherits-from-config — relies on Tauri defaults. The clamp added in Task 3 covers off-screen restoration.

- [ ] **Step 3: Mirror review**

`windowSync.ts::startPublishing` posts state every 250ms — chatty but correct. `startMirroring` subscribes via `listen` for `window-state-broadcast::{target}`. Race: if `startMirroring` is called before any state has been published, the follower window won't update until the next 250ms tick. Acceptable for now.

- [ ] **Step 4: Document outcomes in EXECUTION-LOG.**

---

## Task 6: Release v0.3.0

(Same template as Phase 6's Task 9: version bump, release.sh, formula sha update, tap push, gh release, brew upgrade verification, EXECUTION-LOG, tags.)
