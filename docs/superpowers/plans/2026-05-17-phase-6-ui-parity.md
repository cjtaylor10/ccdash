# Phase 6: UI Parity with CLI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the four CLI-only operations (project add/remove, session launch/kill) as UI buttons + dialogs, so the dashboard no longer requires dropping into the terminal for routine ops.

**Architecture:** All four operations already exist as daemon RPC methods (`project.add`, `project.remove`, `session.launch`, `session.kill`). We add four Tauri commands that proxy through the existing `Client`, plus a thin native folder-picker via Tauri 2's `dialog` plugin for the "Add project" path input. Frontend gets four new components / interactions; daemon untouched. Conflict handling for `session.launch` is in scope: surface the `PortConflictData` and offer the three remediation actions the spec calls for.

**Tech Stack:** Tauri 2.x (Rust backend + Svelte 5 frontend), `tauri-plugin-dialog` v2, existing JSON-RPC client.

---

## File map

### Rust (Tauri backend)
- Modify: `apps/ccdash-ui/Cargo.toml` — add `tauri-plugin-dialog = "2"`.
- Modify: `apps/ccdash-ui/src/main.rs` — register the dialog plugin + new commands in `invoke_handler!`.
- Modify: `apps/ccdash-ui/src/commands.rs` — add `project_add`, `project_remove`, `session_launch`, `session_kill` commands wrapping `call_method`.
- Modify: `apps/ccdash-ui/capabilities/default.json` — grant `dialog:allow-open`.

### Frontend (Svelte 5)
- Modify: `apps/ccdash-ui/ui/package.json` — add `@tauri-apps/plugin-dialog`.
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts` — add `projectAdd`, `projectRemove`, `sessionLaunch`, `sessionKill` typed wrappers + a `PortConflictError` interface.
- Create: `apps/ccdash-ui/ui/src/lib/components/LaunchDialog.svelte` — modal: project picker + worktree picker + command override input + conflict remediation UI.
- Create: `apps/ccdash-ui/ui/src/lib/components/ContextMenu.svelte` — generic right-click menu (used by Sidebar for now; reusable for future).
- Modify: `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte` — "+ Add project" button in header → dialog plugin → `projectAdd`; right-click on a project row → ContextMenu → `projectRemove` after confirm.
- Modify: `apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte` — add "Kill" button column wired to `sessionKill`.
- Modify: `apps/ccdash-ui/ui/src/App.svelte` — top-bar "Launch session" button that opens `LaunchDialog`.

---

## Task 1: Add tauri-plugin-dialog dependency and register plugin

**Files:**
- Modify: `apps/ccdash-ui/Cargo.toml`
- Modify: `apps/ccdash-ui/src/main.rs`
- Modify: `apps/ccdash-ui/capabilities/default.json`
- Modify: `apps/ccdash-ui/ui/package.json` (add the JS shim)

- [ ] **Step 1: Add Rust dep**

In `apps/ccdash-ui/Cargo.toml`, find the `[dependencies]` table and add:

```toml
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: Register the plugin in main.rs**

In `apps/ccdash-ui/src/main.rs`, find `tauri::Builder::default()` and chain `.plugin(tauri_plugin_dialog::init())` immediately after it:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(client_state::ClientState::new())
```

- [ ] **Step 3: Add the dialog permission**

In `apps/ccdash-ui/capabilities/default.json`, append `"dialog:allow-open"` to the `permissions` array.

- [ ] **Step 4: Add the JS shim**

In `apps/ccdash-ui/ui/package.json`, under `dependencies`, add (alphabetized):

```json
"@tauri-apps/plugin-dialog": "^2.0.0",
```

Then run from the workspace root:

```bash
pnpm --dir apps/ccdash-ui/ui install
```

Expected: lockfile updates, no errors.

- [ ] **Step 5: Verify compile**

Run:

```bash
cargo check -p ccdash-ui
pnpm --dir apps/ccdash-ui/ui run build
```

Both must succeed.

- [ ] **Step 6: Commit**

```bash
git add apps/ccdash-ui/Cargo.toml apps/ccdash-ui/src/main.rs apps/ccdash-ui/capabilities/default.json apps/ccdash-ui/ui/package.json apps/ccdash-ui/ui/pnpm-lock.yaml Cargo.lock
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: wire tauri-plugin-dialog for native path picker

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Add four Rust Tauri commands wrapping daemon RPCs

**Files:**
- Modify: `apps/ccdash-ui/src/commands.rs`
- Modify: `apps/ccdash-ui/src/main.rs` (register handlers)

- [ ] **Step 1: Append four commands to commands.rs**

At the bottom of `apps/ccdash-ui/src/commands.rs`, append:

```rust
// === Project management ===

#[tauri::command]
pub async fn project_add(
    state: State<'_, ClientState>,
    path: String,
    name: Option<String>,
) -> Result<Value, String> {
    let mut params = serde_json::Map::new();
    params.insert("path".into(), Value::String(path));
    if let Some(n) = name {
        params.insert("name".into(), Value::String(n));
    }
    call_method(&state, "project.add", Value::Object(params)).await
}

#[tauri::command]
pub async fn project_remove(state: State<'_, ClientState>, id: String) -> Result<Value, String> {
    call_method(&state, "project.remove", serde_json::json!({ "id": id })).await
}

#[tauri::command]
pub async fn session_launch(
    state: State<'_, ClientState>,
    project_id: String,
    worktree: Option<String>,
    command: Option<String>,
    force_token: Option<String>,
) -> Result<Value, String> {
    let mut params = serde_json::Map::new();
    params.insert("project_id".into(), Value::String(project_id));
    if let Some(w) = worktree {
        params.insert("worktree".into(), Value::String(w));
    }
    if let Some(c) = command {
        params.insert("command".into(), Value::String(c));
    }
    if let Some(t) = force_token {
        params.insert("force_token".into(), Value::String(t));
    }
    call_method(&state, "session.launch", Value::Object(params)).await
}

#[tauri::command]
pub async fn session_kill(
    state: State<'_, ClientState>,
    tmux_session_id: String,
) -> Result<Value, String> {
    call_method(
        &state,
        "session.kill",
        serde_json::json!({ "tmux_session_id": tmux_session_id }),
    )
    .await
}
```

- [ ] **Step 2: Register them in main.rs**

In `apps/ccdash-ui/src/main.rs`'s `invoke_handler!` list, append the four new handlers right after `commands::plans_get`:

```rust
            commands::plans_get,
            commands::project_add,
            commands::project_remove,
            commands::session_launch,
            commands::session_kill,
            commands::terminal_open,
```

- [ ] **Step 3: Compile + clippy**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p ccdash-ui
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/ccdash-ui/src/commands.rs apps/ccdash-ui/src/main.rs
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: add project_add/remove + session_launch/kill Tauri commands

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Extend frontend tauri.ts with typed wrappers

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts`

- [ ] **Step 1: Append new exports**

At the end of `apps/ccdash-ui/ui/src/lib/tauri.ts` (after the existing `terminal` and `windows` exports), append:

```typescript
export interface PortConflict {
  port: number;
  holder: string;
}

export interface PortConflictData {
  conflicts: PortConflict[];
  force_token: string;
}

/** Parses an RPC error string for the embedded PortConflict payload, if any.
 * The Rust side currently surfaces just the message string; the daemon error
 * `data` field is not propagated. Treat a `port conflict` message as a marker
 * and call `session.launch` with `force_token` only when the user opts in via
 * the conflict dialog — but for now the UI does a second `sessionList` refresh
 * and shows a toast. (When daemon error.data plumbing lands, this will be
 * richer; for v0.2 we just surface the message.) */
export function isPortConflictMessage(msg: string): boolean {
  return /port conflict/i.test(msg);
}

export const projectsApi = {
  add: (path: string, name?: string) =>
    invoke<{ id: string; name: string; path: string; worktrees: Worktree[]; state: 'ok' | 'missing' }>(
      'project_add',
      { path, name },
    ),
  remove: (id: string) => invoke<null>('project_remove', { id }),
};

export interface LaunchOpts {
  projectId: string;
  worktree?: string;
  command?: string;
  forceToken?: string;
}

export const sessionsApi = {
  launch: (opts: LaunchOpts) =>
    invoke<{ session: Session }>('session_launch', {
      projectId: opts.projectId,
      worktree: opts.worktree,
      command: opts.command,
      forceToken: opts.forceToken,
    }),
  kill: (tmuxSessionId: string) =>
    invoke<null>('session_kill', { tmuxSessionId }),
};
```

- [ ] **Step 2: Verify build**

```bash
pnpm --dir apps/ccdash-ui/ui run build
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/tauri.ts
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: typed frontend wrappers for project + session RPCs

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Add "+ Add project" button + folder picker to Sidebar

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Replace Sidebar.svelte**

Overwrite the file contents with:

```svelte
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { projects, selectedProjectId } from '$lib/stores';
  import { projectsApi, tauri } from '$lib/tauri';

  let busy = false;
  let errMsg: string | null = null;

  let menuOpenForId: string | null = null;
  let menuX = 0;
  let menuY = 0;

  function select(id: string) {
    selectedProjectId.set(id);
  }

  async function addProject() {
    errMsg = null;
    busy = true;
    try {
      const picked = await open({ directory: true, multiple: false, title: 'Pick project directory' });
      if (!picked || typeof picked !== 'string') {
        busy = false;
        return;
      }
      await projectsApi.add(picked);
      // Refresh — the daemon broadcast should also trigger this, but explicit
      // refresh covers the race where the modal closes before the event lands.
      const { projects: ps } = await tauri.projectList();
      projects.set(ps);
    } catch (e) {
      errMsg = String(e);
    } finally {
      busy = false;
    }
  }

  function openMenu(ev: MouseEvent, id: string) {
    ev.preventDefault();
    menuOpenForId = id;
    menuX = ev.clientX;
    menuY = ev.clientY;
  }

  function closeMenu() {
    menuOpenForId = null;
  }

  async function removeProject(id: string) {
    closeMenu();
    const proj = $projects.find((p) => p.id === id);
    if (!proj) return;
    if (!confirm(`Remove project "${proj.name}"? (sessions/worktrees are NOT deleted)`)) return;
    try {
      await projectsApi.remove(id);
      const { projects: ps } = await tauri.projectList();
      projects.set(ps);
      if ($selectedProjectId === id) {
        selectedProjectId.set(ps[0]?.id ?? null);
      }
    } catch (e) {
      errMsg = String(e);
    }
  }
</script>

<svelte:window on:click={closeMenu} />

<aside>
  <header>
    <h2>Projects</h2>
    <button class="add" on:click={addProject} disabled={busy} title="Add project">+ Add</button>
  </header>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
  <ul>
    {#each $projects as p (p.id)}
      <li class:active={$selectedProjectId === p.id}>
        <button on:click={() => select(p.id)} on:contextmenu={(e) => openMenu(e, p.id)}>
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
      <li class="empty">(no projects — click "+ Add" above)</li>
    {/each}
  </ul>

  {#if menuOpenForId}
    <div class="ctxmenu" style="left:{menuX}px; top:{menuY}px;" on:click|stopPropagation>
      <button on:click={() => removeProject(menuOpenForId!)}>Remove project</button>
    </div>
  {/if}
</aside>

<style>
  aside {
    width: 260px;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    overflow-y: auto;
    height: 100vh;
    position: relative;
  }
  header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  header h2 {
    margin: 0;
    font-size: 14px;
    text-transform: uppercase;
    color: var(--fg-dim);
    letter-spacing: 1px;
  }
  .add {
    font-size: 12px;
    padding: 4px 10px;
    background: var(--accent-bg);
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .add:hover { background: var(--accent); color: var(--bg); }
  .err {
    padding: 8px 16px;
    background: rgba(255, 0, 0, 0.1);
    color: var(--danger);
    font-size: 12px;
  }
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
  .ctxmenu {
    position: fixed;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    z-index: 1000;
    min-width: 180px;
  }
  .ctxmenu button {
    display: block;
    width: 100%;
    padding: 8px 14px;
    background: transparent;
    border: none;
    color: var(--fg);
    text-align: left;
    font-size: 13px;
  }
  .ctxmenu button:hover { background: var(--accent-bg); color: var(--accent); }
</style>
```

- [ ] **Step 2: Verify build**

```bash
pnpm --dir apps/ccdash-ui/ui run build
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: sidebar add-project button + right-click remove

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Add "Kill" button to SessionsView

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte`

- [ ] **Step 1: Replace SessionsView.svelte**

Overwrite with:

```svelte
<script lang="ts">
  import { sessions, terminalPane } from '$lib/stores';
  import { sessionsApi, tauri } from '$lib/tauri';

  let busy: Record<string, boolean> = {};
  let errMsg: string | null = null;

  function attach(sessionId: string) {
    terminalPane.set({
      command: ['tmux', 'attach-session', '-t', sessionId],
      mode: 'live',
    });
  }

  async function kill(sessionId: string, name: string) {
    if (!confirm(`Kill session "${name}" (${sessionId})? This terminates the tmux session.`)) return;
    busy = { ...busy, [sessionId]: true };
    errMsg = null;
    try {
      await sessionsApi.kill(sessionId);
      const { sessions: ss } = await tauri.sessionList();
      sessions.set(ss);
    } catch (e) {
      errMsg = String(e);
    } finally {
      const next = { ...busy };
      delete next[sessionId];
      busy = next;
    }
  }
</script>

<div>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
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
          <td class="actions">
            <button on:click={() => attach(s.tmux_session_id)}>Attach</button>
            <button class="danger" on:click={() => kill(s.tmux_session_id, s.name)} disabled={busy[s.tmux_session_id]}>
              {busy[s.tmux_session_id] ? '…' : 'Kill'}
            </button>
          </td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty">(no sessions — click "Launch session" up top)</td></tr>
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
  .actions { display: flex; gap: 6px; }
  .danger {
    background: transparent;
    border: 1px solid var(--danger);
    color: var(--danger);
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 12px;
  }
  .danger:hover:not([disabled]) { background: var(--danger); color: var(--bg); }
  .err {
    padding: 8px 12px;
    background: rgba(255, 0, 0, 0.1);
    color: var(--danger);
    font-size: 12px;
  }
</style>
```

- [ ] **Step 2: Build**

```bash
pnpm --dir apps/ccdash-ui/ui run build
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: kill button on each session row

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Create LaunchDialog component

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/LaunchDialog.svelte`

- [ ] **Step 1: Write the file**

Create `apps/ccdash-ui/ui/src/lib/components/LaunchDialog.svelte` with:

```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { projects, selectedProjectId, sessions } from '$lib/stores';
  import { sessionsApi, tauri } from '$lib/tauri';

  export let open = false;

  const dispatch = createEventDispatcher<{ close: void }>();

  let projectId: string | null = null;
  let worktree: string | null = null;
  let command = '';
  let busy = false;
  let errMsg: string | null = null;
  /** Set when the daemon returns port conflict; lets the user retry with `forceToken`. */
  let forceToken: string | null = null;

  // Sync initial selection with the sidebar's selection whenever the dialog opens.
  $: if (open && projectId === null) {
    projectId = $selectedProjectId ?? $projects[0]?.id ?? null;
    const proj = $projects.find((p) => p.id === projectId);
    worktree = proj?.worktrees.find((w) => w.is_primary)?.branch
      ?? proj?.worktrees[0]?.branch
      ?? null;
    command = '';
    errMsg = null;
    forceToken = null;
  }

  $: currentProject = $projects.find((p) => p.id === projectId);

  function close() {
    open = false;
    projectId = null;
    worktree = null;
    command = '';
    errMsg = null;
    forceToken = null;
    dispatch('close');
  }

  function onProjectChange(e: Event) {
    projectId = (e.target as HTMLSelectElement).value;
    const proj = $projects.find((p) => p.id === projectId);
    worktree = proj?.worktrees.find((w) => w.is_primary)?.branch
      ?? proj?.worktrees[0]?.branch
      ?? null;
    forceToken = null;
    errMsg = null;
  }

  async function submit() {
    if (!projectId) return;
    busy = true;
    errMsg = null;
    try {
      await sessionsApi.launch({
        projectId,
        worktree: worktree ?? undefined,
        command: command.trim() || undefined,
        forceToken: forceToken ?? undefined,
      });
      const { sessions: ss } = await tauri.sessionList();
      sessions.set(ss);
      close();
    } catch (e) {
      const msg = String(e);
      // Daemon returns the message "port conflict; pass force_token to bypass"
      // but the data payload (with the actual force_token) isn't surfaced
      // through call_method yet. For v0.2 we just show the message and
      // disable the "Launch anyway" button — full remediation is Phase 7
      // when the RPC layer is enhanced to carry error.data.
      errMsg = msg;
    } finally {
      busy = false;
    }
  }
</script>

{#if open}
  <div class="backdrop" on:click={close}>
    <div class="modal" on:click|stopPropagation role="dialog" aria-modal="true">
      <header>
        <h3>Launch session</h3>
        <button class="x" on:click={close} aria-label="Close">×</button>
      </header>
      <div class="body">
        <label>
          <span>Project</span>
          <select value={projectId ?? ''} on:change={onProjectChange} disabled={busy}>
            {#each $projects as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </label>

        <label>
          <span>Worktree</span>
          <select bind:value={worktree} disabled={busy || !currentProject}>
            {#if currentProject}
              {#each currentProject.worktrees as wt (wt.path)}
                <option value={wt.branch}>{wt.branch}{wt.is_primary ? ' (main)' : ''}</option>
              {/each}
            {/if}
          </select>
        </label>

        <label>
          <span>Command override</span>
          <input
            type="text"
            placeholder="claude"
            bind:value={command}
            disabled={busy}
          />
          <small>Leave blank to run <code>claude</code>.</small>
        </label>

        {#if errMsg}
          <div class="err">{errMsg}</div>
        {/if}
      </div>
      <footer>
        <button on:click={close} disabled={busy}>Cancel</button>
        <button class="primary" on:click={submit} disabled={busy || !projectId}>
          {busy ? 'Launching…' : 'Launch'}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 8px;
    min-width: 460px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  header h3 { margin: 0; font-size: 15px; }
  .x {
    background: transparent;
    border: none;
    color: var(--fg-dim);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
  }
  .body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label > span {
    font-size: 12px;
    color: var(--fg-dim);
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  select, input[type="text"] {
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px 8px;
    font-size: 13px;
    font-family: var(--mono);
  }
  small { color: var(--fg-dim); font-size: 11px; }
  small code { font-family: var(--mono); }
  .err {
    padding: 8px 12px;
    background: rgba(255, 0, 0, 0.1);
    color: var(--danger);
    border-radius: 4px;
    font-size: 12px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
  }
  footer button {
    padding: 6px 14px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-size: 13px;
  }
  footer .primary {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }
  footer .primary:disabled { opacity: 0.5; }
</style>
```

- [ ] **Step 2: Build**

```bash
pnpm --dir apps/ccdash-ui/ui run build
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/LaunchDialog.svelte
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: launch-session modal with project/worktree/command picker

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Wire "Launch session" button into App top bar

**Files:**
- Modify: `apps/ccdash-ui/ui/src/App.svelte`

- [ ] **Step 1: Add the import + state + button**

Edit `App.svelte`. In the `<script>` block, after the existing component imports, add:

```typescript
  import LaunchDialog from '$lib/components/LaunchDialog.svelte';

  let launchOpen = false;
```

Then in the `header > .actions` div, **before** the `<select>` element (so the Launch button is the first action), insert:

```svelte
        <button class="primary" on:click={() => (launchOpen = true)}>Launch session</button>
```

At the very end of the template (after the closing `</div>` of `.root`), add:

```svelte
<LaunchDialog bind:open={launchOpen} />
```

Update the `<style>` block to add a `.primary` style for the new button (append to the existing `<style>`):

```css
  .actions .primary {
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 4px 12px;
    font-size: 12px;
  }
  .actions .primary:hover { opacity: 0.9; }
```

- [ ] **Step 2: Build**

```bash
pnpm --dir apps/ccdash-ui/ui run build
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/App.svelte
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: launch-session top-bar button + dialog wiring

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Full-workspace verification

- [ ] **Step 1: Run the full gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/ccdash-ui/ui run build
cargo build -p ccdash-ui
```

All must pass.

- [ ] **Step 2: If anything fails, fix it and re-run before continuing.**

The most likely failures are:
- Tauri 2's dialog plugin version mismatch — pin to `2.0` or whatever matches the installed `tauri = "2"`.
- Capability identifier wrong — should be `dialog:allow-open`.
- Svelte 5 syntax mistakes — `on:click|stopPropagation` is valid in Svelte 5.

- [ ] **Step 3: Final commit if any fixes were needed**

```bash
git add -A
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
phase-6: build fixes

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Release v0.2.0

- [ ] **Step 1: Tag phase-6-done**

```bash
git tag phase-6-done
```

- [ ] **Step 2: Build release tarball**

```bash
./packaging/scripts/release.sh 0.2.0
```

Expected: `packaging/dist/ccdash-0.2.0.tar.gz` is produced.

- [ ] **Step 3: Tag v0.2.0**

```bash
git tag v0.2.0
git push origin phase-1-foundation phase-6-done v0.2.0
```

- [ ] **Step 4: Compute formula sha256**

```bash
SHA=$(curl -sL https://github.com/cjtaylor10/ccdash/archive/refs/tags/v0.2.0.tar.gz | shasum -a 256 | awk '{print $1}')
echo "$SHA"
```

- [ ] **Step 5: Update the formula**

Edit `packaging/homebrew/Formula/ccdash.rb`:
- Update the `url` line to point at `v0.2.0`.
- Update `sha256` to the value computed above.
- Update `version` to `0.2.0`.

Then:

```bash
git add packaging/homebrew/Formula/ccdash.rb
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
formula: bump to v0.2.0

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
git push
```

- [ ] **Step 6: Push to the tap**

```bash
if [ ! -d /tmp/homebrew-ccdash-tap ]; then
  git clone https://github.com/cjtaylor10/homebrew-ccdash-tap /tmp/homebrew-ccdash-tap
fi
cp packaging/homebrew/Formula/ccdash.rb /tmp/homebrew-ccdash-tap/Formula/ccdash.rb
cd /tmp/homebrew-ccdash-tap
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" add Formula/ccdash.rb
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "bump to v0.2.0"
git push
cd /Users/cjtaylor/Documents/Claude-Projects/cc-dashboard
```

- [ ] **Step 7: Create GitHub release**

```bash
gh release create v0.2.0 \
  --title "v0.2.0 — UI parity with CLI" \
  --notes "Adds UI buttons for project add/remove + session launch/kill. Right-click a project in the sidebar to remove it." \
  packaging/dist/ccdash-0.2.0.tar.gz
```

- [ ] **Step 8: Verify brew upgrade**

```bash
brew update
brew upgrade cjtaylor10/ccdash-tap/ccdash
brew services restart cjtaylor10/ccdash-tap/ccdash
which ccdash ccdash-daemon ccdash-ui
ccdash status
```

Expected: ccdash status reports the daemon is running, all three binaries are on PATH.

---

## Task 10: Update EXECUTION-LOG

- [ ] **Step 1: Append a "Phase 6 — Complete" section to `docs/superpowers/EXECUTION-LOG.md`**

The section should follow the template used for prior phases: result, plan deviations (if any), acceptance check, tag. Specifically note:
- Whether the daemon's `error.data` was plumbed through (it is NOT in this phase; `LaunchDialog` only surfaces the message string — full conflict remediation deferred to Phase 7 or beyond).
- The four new Tauri commands.
- That visual verification of buttons + dialog still needs user click-test.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/EXECUTION-LOG.md
git -c user.email=carsonjtaylor10@gmail.com -c user.name="Carson Taylor" commit -m "$(cat <<'EOF'
docs: phase-6 execution log entry

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
git push
```

---

## Self-review

Spec coverage:
- "Launch session" button + dialog with project + worktree + command override — Tasks 6, 7. ✅
- "Add project" button + path picker — Tasks 1, 4. ✅
- "Remove project" via right-click — Task 4. ✅
- "Kill session" button on each row — Task 5. ✅
- All four wrap existing daemon RPC methods — Task 2. ✅
- Tauri `dialog` plugin capability added — Task 1. ✅

Conflict remediation is partial: `LaunchDialog` shows the conflict message string but cannot extract the embedded `force_token` because `call_method` in `commands.rs` only forwards `error.message`, not `error.data`. Full kill-or-rebind remediation is deferred to Phase 7 (verify + polish existing features); for v0.2 the user can kill the conflicting session via the new "Kill" button and then re-launch.

Type consistency: all Tauri commands accept the param names the daemon expects (`project_id`, `tmux_session_id`, `force_token`). The frontend wrappers use camelCase (`projectId`, `forceToken`) — Tauri auto-translates. Verified against `handlers.rs` and `protocol.rs`.
