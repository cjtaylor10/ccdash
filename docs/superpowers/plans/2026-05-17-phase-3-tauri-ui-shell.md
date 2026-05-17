# ccdash Phase 3 — Tauri UI Shell + Static Views Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2.x desktop app that connects to ccdash-daemon over the Unix socket and presents the project list, session list, port registry, and plans in static views — no embedded terminals yet (Phase 4) and no multi-window mirror (Phase 4).

**Architecture:** A new Tauri crate at `apps/ccdash-ui/`. Rust backend holds one shared `ccdash_core::client::Client` (handshaked at startup), exposes thin `#[tauri::command]` proxies for `project.list`, `session.list`, `ports.list`, `plans.get`. SvelteKit frontend renders a sidebar (projects) + main pane with tabs (sessions / ports / plans). Svelte stores hydrate from Tauri command invocations + are refreshed on focus / interval. No live event subscription in this phase — refresh-on-demand keeps Phase 3 small; live `subscribe`/notification streaming is Phase 4.

**Tech Stack:**
- Tauri 2.10 (Rust + WebView)
- SvelteKit 2.x (frontend)
- Vite 6.x (dev server / bundler)
- Plain CSS with custom properties (no UI framework — keep deps small)
- TypeScript on the frontend

**Spec reference:** `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`
**Predecessor:** Phase 2 complete; tag `phase-2-done`.

**Design choices made by the executor (documented here, not in the spec):**
- **CSS approach:** Plain CSS + CSS custom properties (themed via `:root`). No Tailwind. Avoids JS framework lock-in inside the styles.
- **Color scheme:** Dark default (`--bg: #1a1b1e; --fg: #e6e6e6; --accent: #7aa2f7`). System-preference auto-switching is deferred to v2.
- **State:** Svelte stores (`writable`) per topic. No external state library.
- **Connection:** Tauri owns the Client; frontend never sees the socket. One connection per app instance.

---

## File Structure

```
ccdash/
├── apps/
│   └── ccdash-ui/
│       ├── Cargo.toml                          # Rust crate manifest
│       ├── tauri.conf.json                     # Tauri config
│       ├── build.rs                            # Tauri build script
│       ├── package.json                        # Node frontend deps
│       ├── svelte.config.js
│       ├── vite.config.ts
│       ├── tsconfig.json
│       ├── src-tauri/                          # Tauri convention puts Rust in src-tauri,
│       │                                       # but we keep Cargo.toml at the crate root
│       │                                       # since this whole dir IS the Rust crate.
│       ├── src/                                # Rust source
│       │   ├── main.rs                         # Tauri entry, builds the App
│       │   ├── client_state.rs                 # Arc<Mutex<Client>> wrapper
│       │   └── commands.rs                     # #[tauri::command] wrappers
│       └── ui/                                 # SvelteKit project
│           ├── src/
│           │   ├── app.html
│           │   ├── lib/
│           │   │   ├── tauri.ts                # Wraps `invoke()`
│           │   │   ├── stores.ts               # Svelte stores
│           │   │   ├── theme.css               # CSS variables + base styles
│           │   │   └── components/
│           │   │       ├── Sidebar.svelte
│           │   │       ├── SessionsView.svelte
│           │   │       ├── PortsView.svelte
│           │   │       └── PlansView.svelte
│           │   └── routes/
│           │       └── +page.svelte            # Single-page layout
│           ├── static/                         # public assets (favicon, etc)
│           ├── package.json
│           ├── tsconfig.json
│           └── svelte.config.js
└── Cargo.toml                                  # add apps/ccdash-ui to members
```

Tauri convention is `src-tauri/`, but because the WHOLE Tauri crate IS the Rust crate (no separate "wrapper" project), we put `Cargo.toml` at `apps/ccdash-ui/` directly and the SvelteKit project in `apps/ccdash-ui/ui/`.

---

## Task A1: Scaffold the Tauri crate manifest

**Files:**
- Modify: root `Cargo.toml` (add to members)
- Create: `apps/ccdash-ui/Cargo.toml`
- Create: `apps/ccdash-ui/build.rs`

- [ ] **Step 1: Add `apps/ccdash-ui` to workspace members**

Edit root `Cargo.toml`:
```toml
members = ["crates/ccdash-core", "crates/ccdash-daemon", "crates/ccdash-cli", "apps/ccdash-ui"]
```

Also add Tauri to workspace deps:
```toml
tauri = { version = "2", features = [] }
tauri-build = { version = "2", features = [] }
```

(Place both under `[workspace.dependencies]`.)

- [ ] **Step 2: Write `apps/ccdash-ui/Cargo.toml`**

```toml
[package]
name = "ccdash-ui"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
name = "ccdash_ui_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "ccdash-ui"
path = "src/main.rs"

[build-dependencies]
tauri-build = { workspace = true }

[dependencies]
ccdash-core = { path = "../../crates/ccdash-core" }
tauri = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 3: Write `apps/ccdash-ui/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4: Verify the workspace still type-checks (Tauri config isn't there yet, expect a failure or warning)**

Run: `cargo check --workspace 2>&1 | tail -5`

Expected: a Tauri-specific error about missing `tauri.conf.json`. That's fine — Task A2 adds it.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml apps/ccdash-ui/Cargo.toml apps/ccdash-ui/build.rs
git commit -m "ccdash-ui: scaffold cargo crate (Tauri config still missing)"
```

---

## Task A2: Tauri config + minimal main.rs

**Files:**
- Create: `apps/ccdash-ui/tauri.conf.json`
- Create: `apps/ccdash-ui/src/main.rs`
- Create: `apps/ccdash-ui/ui/` (SvelteKit project root, populated by Task A3)
- Create: `apps/ccdash-ui/icons/icon.png` (placeholder 512x512)

- [ ] **Step 1: Create a placeholder icon**

Tauri 2 requires icons. Use a 1x1 transparent PNG as a placeholder; real icons come in Phase 5.

```bash
mkdir -p apps/ccdash-ui/icons
python3 -c "
import base64
png = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=')
open('apps/ccdash-ui/icons/icon.png','wb').write(png)
"
```

- [ ] **Step 2: Write `apps/ccdash-ui/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ccdash",
  "version": "0.1.0",
  "identifier": "com.ccdash.app",
  "build": {
    "beforeDevCommand": "pnpm --filter ccdash-ui-frontend dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm --filter ccdash-ui-frontend build",
    "frontendDist": "ui/build"
  },
  "app": {
    "windows": [
      {
        "title": "ccdash",
        "width": 1100,
        "height": 720,
        "minWidth": 720,
        "minHeight": 480
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png"]
  }
}
```

> Note: `beforeDevCommand`/`beforeBuildCommand` invoke pnpm; if you use npm, replace with `npm --prefix ui run dev` / `npm --prefix ui run build`. We'll create the `ui/` package in Task A3.

- [ ] **Step 3: Write `apps/ccdash-ui/src/main.rs`**

```rust
//! ccdash desktop UI entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_state;
mod commands;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(client_state::ClientState::new())
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

- [ ] **Step 4: Write placeholder `client_state.rs` and `commands.rs` (real impls in B1)**

`apps/ccdash-ui/src/client_state.rs`:
```rust
//! Wraps an optional `ccdash_core::client::Client` behind a tokio mutex
//! so all Tauri commands can share one connection.

use ccdash_core::client::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ClientState {
    pub inner: Arc<Mutex<Option<Client>>>,
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}
```

`apps/ccdash-ui/src/commands.rs`:
```rust
//! Tauri command bindings — implemented in Task B1.

use tauri::State;

use crate::client_state::ClientState;

#[tauri::command]
pub async fn connect_and_handshake(_state: State<'_, ClientState>) -> Result<String, String> {
    Err("not yet implemented".into())
}

#[tauri::command]
pub async fn project_list(_state: State<'_, ClientState>) -> Result<serde_json::Value, String> {
    Err("not yet implemented".into())
}

#[tauri::command]
pub async fn session_list(_state: State<'_, ClientState>) -> Result<serde_json::Value, String> {
    Err("not yet implemented".into())
}

#[tauri::command]
pub async fn ports_list(_state: State<'_, ClientState>) -> Result<serde_json::Value, String> {
    Err("not yet implemented".into())
}

#[tauri::command]
pub async fn plans_get(
    _state: State<'_, ClientState>,
    _project_id: String,
) -> Result<serde_json::Value, String> {
    Err("not yet implemented".into())
}
```

- [ ] **Step 5: Commit (build still fails — frontend dir missing)**

```bash
git add apps/ccdash-ui
git commit -m "ccdash-ui: tauri config + Rust skeleton (frontend dir to come)"
```

---

## Task A3: Scaffold the SvelteKit frontend

**Files:**
- Create: `apps/ccdash-ui/ui/package.json`
- Create: `apps/ccdash-ui/ui/svelte.config.js`
- Create: `apps/ccdash-ui/ui/vite.config.ts`
- Create: `apps/ccdash-ui/ui/tsconfig.json`
- Create: `apps/ccdash-ui/ui/src/app.html`
- Create: `apps/ccdash-ui/ui/static/.gitkeep`
- Create: `apps/ccdash-ui/ui/src/routes/+layout.ts`
- Create: `apps/ccdash-ui/ui/src/routes/+page.svelte`

- [ ] **Step 1: Write `ui/package.json`**

```json
{
  "name": "ccdash-ui-frontend",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite dev --port 1420 --strictPort",
    "build": "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "@tauri-apps/api": "^2"
  },
  "devDependencies": {
    "@sveltejs/adapter-static": "^3",
    "@sveltejs/kit": "^2",
    "@sveltejs/vite-plugin-svelte": "^4",
    "svelte": "^5",
    "typescript": "^5",
    "vite": "^6"
  }
}
```

- [ ] **Step 2: Write `ui/svelte.config.js`**

```javascript
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: 'index.html',
      precompress: false,
    }),
  },
};
```

- [ ] **Step 3: Write `ui/vite.config.ts`**

```typescript
import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    hmr: { protocol: 'ws', host: '127.0.0.1', port: 1421 },
    watch: { ignored: ['**/src-tauri/**', '**/../src/**'] },
  },
  clearScreen: false,
});
```

- [ ] **Step 4: Write `ui/tsconfig.json`**

```json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": false,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

- [ ] **Step 5: Write `ui/src/app.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width" />
    <title>ccdash</title>
    %sveltekit.head%
  </head>
  <body data-sveltekit-preload-data="hover">
    <div style="display:contents">%sveltekit.body%</div>
  </body>
</html>
```

- [ ] **Step 6: Write `ui/src/routes/+layout.ts` (turn off SSR — Tauri is SPA-only)**

```typescript
export const prerender = true;
export const ssr = false;
```

- [ ] **Step 7: Write a stub `ui/src/routes/+page.svelte`**

```svelte
<script lang="ts">
  let mounted = false;
  import { onMount } from 'svelte';
  onMount(() => { mounted = true; });
</script>

<main>
  <h1>ccdash</h1>
  <p>{mounted ? 'frontend up' : 'mounting...'}</p>
</main>

<style>
  main { font-family: system-ui, sans-serif; padding: 2rem; }
</style>
```

- [ ] **Step 8: Create `ui/static/.gitkeep`**

```bash
mkdir -p apps/ccdash-ui/ui/static
touch apps/ccdash-ui/ui/static/.gitkeep
```

- [ ] **Step 9: Install + build the frontend once to verify**

```bash
cd apps/ccdash-ui/ui
pnpm install
pnpm run build
cd ../../..
```

Expected: `apps/ccdash-ui/ui/build/index.html` exists.

- [ ] **Step 10: Add `node_modules` and Vite/SvelteKit outputs to `.gitignore`**

Append to root `.gitignore`:
```
apps/ccdash-ui/ui/node_modules/
apps/ccdash-ui/ui/build/
apps/ccdash-ui/ui/.svelte-kit/
apps/ccdash-ui/target/
```

- [ ] **Step 11: Commit**

```bash
git add .gitignore apps/ccdash-ui/ui apps/ccdash-ui/icons
git commit -m "ccdash-ui: scaffold SvelteKit frontend (vite + static adapter)"
```

---

## Task A4: Build the full Tauri app and verify it runs

**Files:** none modified — verification step.

- [ ] **Step 1: Build the Tauri binary**

```bash
cargo build -p ccdash-ui
```

Expected: SUCCESS. Tauri compiles, links the bundled frontend.

- [ ] **Step 2: Run the app once to verify the window opens**

Skip the actual `pnpm tauri dev` step here — Tauri dev mode is interactive and the goal is to verify that the binary builds. Visual smoke test happens in Task B3 once commands are wired.

- [ ] **Step 3: Commit if any incidental changes from build process**

```bash
git status
# If nothing changed, no commit needed. Tauri build sometimes touches lockfiles.
```

---

## Task B1: Implement the Tauri command bridge to ccdash-core::Client

**Files:**
- Modify: `apps/ccdash-ui/src/client_state.rs`
- Modify: `apps/ccdash-ui/src/commands.rs`

- [ ] **Step 1: Replace `commands.rs` with real implementations**

```rust
//! Tauri commands that proxy to ccdash-daemon via a shared `Client`.

use crate::client_state::ClientState;
use ccdash_core::client::Client;
use ccdash_core::protocol::ClientKind;
use serde_json::Value;
use tauri::State;

async fn get_client(
    state: &State<'_, ClientState>,
) -> Result<tokio::sync::MutexGuard<'_, Option<Client>>, String> {
    let guard = state.inner.lock().await;
    if guard.is_none() {
        return Err(
            "daemon not connected — call connect_and_handshake first"
                .to_string(),
        );
    }
    Ok(guard)
}

#[tauri::command]
pub async fn connect_and_handshake(state: State<'_, ClientState>) -> Result<String, String> {
    let mut guard = state.inner.lock().await;
    if guard.is_some() {
        return Ok("already connected".into());
    }
    let mut client = Client::connect_default()
        .await
        .map_err(|e| format!("connect failed: {}", e))?;
    let resp = client
        .handshake(ClientKind::Ui)
        .await
        .map_err(|e| format!("handshake failed: {}", e))?;
    if let Some(err) = resp.error {
        return Err(format!("handshake rejected: {}", err.message));
    }
    *guard = Some(client);
    Ok("connected".into())
}

#[tauri::command]
pub async fn project_list(state: State<'_, ClientState>) -> Result<Value, String> {
    let mut guard = get_client(&state).await?;
    let client = guard.as_mut().unwrap();
    let resp = client
        .call("project.list", serde_json::json!({}))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(err) = resp.error {
        return Err(err.message);
    }
    Ok(resp.result.unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn session_list(state: State<'_, ClientState>) -> Result<Value, String> {
    let mut guard = get_client(&state).await?;
    let client = guard.as_mut().unwrap();
    let resp = client
        .call("session.list", serde_json::json!({}))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(err) = resp.error {
        return Err(err.message);
    }
    Ok(resp.result.unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn ports_list(state: State<'_, ClientState>) -> Result<Value, String> {
    let mut guard = get_client(&state).await?;
    let client = guard.as_mut().unwrap();
    let resp = client
        .call("ports.list", serde_json::json!({}))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(err) = resp.error {
        return Err(err.message);
    }
    Ok(resp.result.unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn plans_get(
    state: State<'_, ClientState>,
    project_id: String,
) -> Result<Value, String> {
    let mut guard = get_client(&state).await?;
    let client = guard.as_mut().unwrap();
    let resp = client
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(err) = resp.error {
        return Err(err.message);
    }
    Ok(resp.result.unwrap_or(Value::Null))
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p ccdash-ui
```

Expected: SUCCESS.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/src/commands.rs
git commit -m "ccdash-ui: wire Tauri commands to ccdash_core::Client"
```

---

## Task C1: Frontend — Tauri bridge module + Svelte stores

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/tauri.ts`
- Create: `apps/ccdash-ui/ui/src/lib/stores.ts`
- Create: `apps/ccdash-ui/ui/src/lib/theme.css`

- [ ] **Step 1: Write `ui/src/lib/tauri.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core';

export interface Project {
  id: string;
  name: string;
  path: string;
  worktrees: Worktree[];
  state: 'ok' | 'missing';
}

export interface Worktree {
  path: string;
  branch: string;
  is_primary: boolean;
}

export interface Session {
  tmux_session_id: string;
  name: string;
  project_id: string | null;
  worktree: string | null;
  cwd: string;
  pid: number;
  state: 'running' | 'exited';
  first_seen: number;
}

export interface PortBinding {
  port: number;
  protocol: string;
  pid: number | null;
  command: string | null;
  project_id: string | null;
}

export interface DeclaredPort {
  project_id: string;
  port: number;
  source: string;
}

export interface Plan {
  path: string;
  title: string;
  phases: PlanPhase[];
}

export interface PlanPhase {
  name: string;
  tasks: PlanTask[];
}

export interface PlanTask {
  title: string;
  done: boolean;
}

export const tauri = {
  connect: () => invoke<string>('connect_and_handshake'),
  projectList: () => invoke<{ projects: Project[] }>('project_list'),
  sessionList: () => invoke<{ sessions: Session[] }>('session_list'),
  portsList: () => invoke<{ running: PortBinding[]; declared: DeclaredPort[] }>('ports_list'),
  plansGet: (projectId: string) => invoke<{ plans: Plan[] }>('plans_get', { projectId }),
};
```

- [ ] **Step 2: Write `ui/src/lib/stores.ts`**

```typescript
import { writable } from 'svelte/store';
import type { Plan, PortBinding, Project, DeclaredPort, Session } from './tauri';

export const connected = writable<boolean>(false);
export const connectError = writable<string | null>(null);

export const projects = writable<Project[]>([]);
export const selectedProjectId = writable<string | null>(null);
export const sessions = writable<Session[]>([]);
export const ports = writable<{ running: PortBinding[]; declared: DeclaredPort[] }>({
  running: [],
  declared: [],
});
export const plans = writable<Plan[]>([]);

export const activeTab = writable<'sessions' | 'ports' | 'plans'>('sessions');
```

- [ ] **Step 3: Write `ui/src/lib/theme.css`**

```css
:root {
  --bg: #1a1b1e;
  --bg-elev: #25262b;
  --fg: #e6e6e6;
  --fg-dim: #909296;
  --accent: #7aa2f7;
  --accent-bg: #2b3553;
  --border: #2c2e33;
  --danger: #e15c5c;
  --success: #6ec38a;
  --mono: ui-monospace, "SF Mono", "Monaco", "Cascadia Code", monospace;
  --sans: system-ui, -apple-system, sans-serif;
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  padding: 0;
  height: 100%;
  font-family: var(--sans);
  background: var(--bg);
  color: var(--fg);
  font-size: 14px;
}

button {
  background: var(--bg-elev);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 4px 10px;
  font-size: 13px;
  cursor: pointer;
}
button:hover { background: var(--accent-bg); }

a { color: var(--accent); text-decoration: none; }
code, pre { font-family: var(--mono); }
```

- [ ] **Step 4: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib
git commit -m "ccdash-ui frontend: tauri bridge + svelte stores + theme.css"
```

---

## Task C2: Svelte components — Sidebar + SessionsView + PortsView + PlansView

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte`
- Create: `apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte`
- Create: `apps/ccdash-ui/ui/src/lib/components/PortsView.svelte`
- Create: `apps/ccdash-ui/ui/src/lib/components/PlansView.svelte`

- [ ] **Step 1: Write `Sidebar.svelte`**

```svelte
<script lang="ts">
  import { projects, selectedProjectId } from '$lib/stores';

  function select(id: string) {
    selectedProjectId.set(id);
  }
</script>

<aside>
  <header>
    <h2>Projects</h2>
  </header>
  <ul>
    {#each $projects as p (p.id)}
      <li class:active={$selectedProjectId === p.id}>
        <button on:click={() => select(p.id)}>
          <span class="name">{p.name}</span>
          <span class="path">{p.path}</span>
          {#if p.worktrees.length > 1}
            <ul class="worktrees">
              {#each p.worktrees as wt (wt.path)}
                <li><code>{wt.branch}</code>{wt.is_primary ? ' (main)' : ''}</li>
              {/each}
            </ul>
          {/if}
        </button>
      </li>
    {:else}
      <li class="empty">(no projects — add one via the CLI)</li>
    {/each}
  </ul>
</aside>

<style>
  aside {
    width: 260px;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    overflow-y: auto;
    height: 100vh;
  }
  header { padding: 12px 16px; border-bottom: 1px solid var(--border); }
  header h2 { margin: 0; font-size: 14px; text-transform: uppercase; color: var(--fg-dim); letter-spacing: 1px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    width: 100%;
    padding: 10px 16px;
    background: transparent;
    border: none;
    border-radius: 0;
    color: var(--fg);
    text-align: left;
  }
  li.active button { background: var(--accent-bg); border-left: 3px solid var(--accent); padding-left: 13px; }
  li button:hover { background: var(--accent-bg); }
  .name { font-weight: 600; }
  .path { font-family: var(--mono); font-size: 11px; color: var(--fg-dim); }
  .worktrees { margin: 6px 0 0; padding-left: 12px; }
  .worktrees li { font-size: 12px; color: var(--fg-dim); }
  .empty { padding: 16px; color: var(--fg-dim); font-style: italic; }
</style>
```

- [ ] **Step 2: Write `SessionsView.svelte`**

```svelte
<script lang="ts">
  import { sessions } from '$lib/stores';
</script>

<div>
  <table>
    <thead>
      <tr><th>tmux id</th><th>name</th><th>pid</th><th>cwd</th><th>state</th></tr>
    </thead>
    <tbody>
      {#each $sessions as s (s.tmux_session_id)}
        <tr>
          <td><code>{s.tmux_session_id}</code></td>
          <td>{s.name}</td>
          <td>{s.pid}</td>
          <td><code>{s.cwd}</code></td>
          <td class={s.state === 'running' ? 'running' : 'exited'}>{s.state}</td>
        </tr>
      {:else}
        <tr><td colspan="5" class="empty">(no sessions)</td></tr>
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

- [ ] **Step 3: Write `PortsView.svelte`**

```svelte
<script lang="ts">
  import { ports, selectedProjectId } from '$lib/stores';

  $: filteredRunning = $selectedProjectId
    ? $ports.running.filter((p) => p.project_id === $selectedProjectId)
    : $ports.running;
  $: filteredDeclared = $selectedProjectId
    ? $ports.declared.filter((p) => p.project_id === $selectedProjectId)
    : $ports.declared;
</script>

<div>
  <h3>Running listeners</h3>
  <table>
    <thead><tr><th>port</th><th>pid</th><th>command</th><th>project</th></tr></thead>
    <tbody>
      {#each filteredRunning as p (`${p.port}-${p.pid}`)}
        <tr>
          <td><code>{p.port}</code></td>
          <td>{p.pid ?? '?'}</td>
          <td>{p.command ?? '?'}</td>
          <td><code>{p.project_id ?? '-'}</code></td>
        </tr>
      {:else}
        <tr><td colspan="4" class="empty">(none)</td></tr>
      {/each}
    </tbody>
  </table>

  <h3>Declared</h3>
  <table>
    <thead><tr><th>port</th><th>project</th><th>source</th></tr></thead>
    <tbody>
      {#each filteredDeclared as p (`${p.project_id}-${p.port}-${p.source}`)}
        <tr>
          <td><code>{p.port}</code></td>
          <td><code>{p.project_id}</code></td>
          <td>{p.source}</td>
        </tr>
      {:else}
        <tr><td colspan="3" class="empty">(none)</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  h3 { margin: 16px 12px 8px; font-size: 13px; text-transform: uppercase; letter-spacing: 1px; color: var(--fg-dim); }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  th { color: var(--fg-dim); font-weight: 500; font-size: 12px; }
  .empty { text-align: center; color: var(--fg-dim); font-style: italic; padding: 16px; }
</style>
```

- [ ] **Step 4: Write `PlansView.svelte`**

```svelte
<script lang="ts">
  import { plans } from '$lib/stores';
</script>

<div>
  {#each $plans as p (p.path)}
    <section>
      <h3>{p.title}</h3>
      <div class="path"><code>{p.path}</code></div>
      {#each p.phases as phase (phase.name)}
        <div class="phase">
          <h4>{phase.name}</h4>
          <ul>
            {#each phase.tasks as t (t.title)}
              <li class:done={t.done}>
                <span class="check">{t.done ? '✓' : '○'}</span>
                {t.title}
              </li>
            {/each}
          </ul>
          {#if phase.tasks.length > 0}
            <div class="progress">
              {phase.tasks.filter((t) => t.done).length}/{phase.tasks.length} done
            </div>
          {/if}
        </div>
      {/each}
    </section>
  {:else}
    <div class="empty">(no plans found under docs/superpowers/&#123;specs,plans&#125;/)</div>
  {/each}
</div>

<style>
  section { padding: 16px 20px; border-bottom: 1px solid var(--border); }
  h3 { margin: 0 0 4px; font-size: 16px; }
  .path { font-family: var(--mono); font-size: 11px; color: var(--fg-dim); margin-bottom: 12px; }
  .phase { margin: 12px 0; }
  h4 { margin: 8px 0 4px; font-size: 13px; color: var(--accent); }
  ul { list-style: none; padding: 0; margin: 0; }
  li { padding: 2px 0; font-size: 13px; }
  li.done { color: var(--fg-dim); text-decoration: line-through; }
  .check { display: inline-block; width: 16px; text-align: center; color: var(--success); }
  li:not(.done) .check { color: var(--fg-dim); }
  .progress { font-size: 11px; color: var(--fg-dim); margin-top: 4px; padding-left: 16px; }
  .empty { padding: 32px; text-align: center; color: var(--fg-dim); font-style: italic; }
</style>
```

- [ ] **Step 5: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components
git commit -m "ccdash-ui frontend: Sidebar + SessionsView + PortsView + PlansView components"
```

---

## Task C3: Wire `+page.svelte` to use the components + load data

**Files:**
- Modify: `apps/ccdash-ui/ui/src/routes/+page.svelte`

- [ ] **Step 1: Replace the stub `+page.svelte`**

```svelte
<script lang="ts">
  import '$lib/theme.css';
  import { onMount } from 'svelte';
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
  } from '$lib/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionsView from '$lib/components/SessionsView.svelte';
  import PortsView from '$lib/components/PortsView.svelte';
  import PlansView from '$lib/components/PlansView.svelte';

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
    const handle = setInterval(refreshTopLevel, 5000);
    return () => clearInterval(handle);
  });

  function setTab(t: 'sessions' | 'ports' | 'plans') {
    activeTab.set(t);
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
  .content { flex: 1; overflow-y: auto; }
</style>
```

- [ ] **Step 2: Build the frontend**

```bash
cd apps/ccdash-ui/ui && pnpm run build && cd ../../..
```

Expected: SUCCESS, `build/index.html` regenerated.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/routes/+page.svelte
git commit -m "ccdash-ui frontend: layout with tabs (sessions / ports / plans) + data polling"
```

---

## Task D1: Build the Tauri binary and run a manual smoke test

**Files:** none — verification.

- [ ] **Step 1: Build the Tauri app in release mode**

```bash
cargo build -p ccdash-ui --release
```

Expected: SUCCESS. Final binary at `target/release/ccdash-ui`.

- [ ] **Step 2: Spawn the daemon in one terminal**

```bash
CCDASH_HOME=/tmp/ui-smoke CCDASH_SOCKET=/tmp/ccdash.sock \
  cargo run -p ccdash-daemon -- --log-level info
```

> Note: the UI uses `paths::default_socket_path()` which is `/tmp/ccdash.sock` on macOS, so the daemon must be on that path for the UI to find it. Use `/tmp/ccdash.sock` for this smoke test (the default).

- [ ] **Step 3: In a second terminal, register a project**

```bash
CCDASH_HOME=/tmp/ui-smoke cargo run -p ccdash-cli -- --socket /tmp/ccdash.sock project add /tmp
```

- [ ] **Step 4: Run the UI**

```bash
CCDASH_HOME=/tmp/ui-smoke target/release/ccdash-ui
```

Verify visually:
- Window opens at 1100×720.
- Sidebar shows the "/tmp" project.
- "Sessions" tab is empty.
- "Ports" tab lists currently-listening ports on the system.
- "Plans" tab shows `(no plans found...)` for the /tmp project.
- Closing the window cleanly exits the app.

- [ ] **Step 5: Commit any incidental changes**

```bash
git status
# Typically nothing changes. If lockfiles update, commit them.
```

---

## Task E1: Phase 3 full verification + tag

- [ ] **Step 1: fmt + clippy**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both succeed. Fix any new warnings inline.

- [ ] **Step 2: Full test run (no new unit tests in Phase 3 — UI smoke is manual)**

```bash
tmux kill-server 2>/dev/null; sleep 0.2
cargo test --workspace
```

Expected: same pass counts as Phase 2 (Phase 3 doesn't add unit tests; Rust commands are thin proxies that the daemon already tests; Svelte component testing would require Vitest setup which is deferred to a polish pass).

- [ ] **Step 3: Tag**

```bash
git tag phase-3-done
```

- [ ] **Step 4: Update execution log**

Append a "Phase 3 — Complete" section to `docs/superpowers/EXECUTION-LOG.md`. Note specifically that:
- No Svelte unit tests (decision: defer Vitest setup to v2 polish — Tauri commands are thin proxies and the Rust client they wrap is exhaustively tested in Phase 1+2).
- CSS approach was plain CSS + custom properties (no framework), color scheme dark-mode default (`#1a1b1e` bg + `#7aa2f7` accent).
- Refresh-on-interval (5s) instead of live `subscribe` notifications — live event streaming via Tauri's event bus is Phase 4.

```bash
git add docs/superpowers/EXECUTION-LOG.md
git commit -m "docs: phase-3 complete — Tauri UI shell + static views"
```

---

## What Phase 3 ships

- Tauri 2.x desktop app (`ccdash-ui`).
- SvelteKit frontend bundled into the app.
- One shared `ccdash_core::Client` connection per app instance.
- Tauri commands: `connect_and_handshake`, `project_list`, `session_list`, `ports_list`, `plans_get`.
- Three views: Sessions, Ports, Plans.
- Sidebar showing projects (with worktrees expanded inline).
- 5-second polling refresh for top-level state.
- Dark theme.

## What's NOT in Phase 3 (deferred)

- Embedded terminals (Phase 4).
- Multi-window mirror / sync (Phase 4).
- Live event subscription via daemon broadcast (Phase 4).
- Project CRUD UI (use the CLI for now — `ccdash project add <path>`).
- Session launch UI (use CLI — `ccdash launch <project>`).
- Themable / system-preference-following color scheme (v2 polish).
- Svelte component unit tests (v2 polish).

## Self-Review

**Spec coverage:**
- §5.4 Tauri 2.x + SvelteKit frontend ✓
- §6.1 startup flow (handshake on connect) → B1 / C3
- §6.2 launch flow — partial (UI doesn't have launch button yet; CLI covers that for Phase 3)
- Window dimensions reasonable per spec's "1100 width" hint → A2

**Placeholder scan:** none.

**Type consistency:** `Project`, `Worktree`, `Session`, `PortBinding`, `DeclaredPort`, `Plan`, `PlanPhase`, `PlanTask` mirror the Rust types from `ccdash-core::protocol` with snake_case fields. Used identically across `tauri.ts`, stores, and components.

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-phase-3-tauri-ui-shell.md`.**
