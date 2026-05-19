# Prompts Library + Sidebar Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a sidebar navigation block above the Projects list that switches the main content area between the existing Workspace and a new localStorage-backed Prompts library with one-click copy-to-clipboard.

**Architecture:** All UI-only changes. Two new Svelte components (`SidebarNav.svelte`, `PromptsView.svelte`), two new stores in `stores.ts` (`activeView`, `prompts` + helpers), and structural edits to `Sidebar.svelte` (split header into header + nav block + projects block) and `App.svelte` (branch the main column on `$activeView`). The terminal panel renders unconditionally regardless of which view is active. No daemon / Rust changes.

**Tech Stack:** Svelte 5, Vite 6, TypeScript 5, Tauri 2 webview (uses `navigator.clipboard.writeText`), existing `toast.ts` for feedback. `pnpm` for package management.

**Spec:** `docs/superpowers/specs/2026-05-17-prompts-library-and-sidebar-nav-design.md`

**Verification commands** (run from `apps/ccdash-ui/ui/`):
- Type-check: `pnpm exec svelte-check --tsconfig ./tsconfig.json`
- Build: `pnpm run build`
- Dev (manual UI test): `pnpm run dev` (then load the Tauri shell from the parent dir)

---

## File Structure

| Path | Action | Responsibility |
| ---- | ------ | -------------- |
| `apps/ccdash-ui/ui/src/lib/stores.ts` | Modify | Add `activeView` store + `Prompt` type + `prompts` store + `addPrompt` / `updatePrompt` / `deletePrompt` helpers. |
| `apps/ccdash-ui/ui/src/lib/components/SidebarNav.svelte` | Create | Two-button nav strip (`Workspace` / `Prompts`) bound to `activeView`. |
| `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte` | Modify | Top header keeps only the collapse button; mount `<SidebarNav />` below; move the "Projects" title + `+` button into a new mini-header above the existing project `<ul>`. |
| `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte` | Create | Two-pane Prompts UI: list + search + `+ New` on the left, editor + Copy/Save/Delete on the right. Per-row copy buttons. |
| `apps/ccdash-ui/ui/src/App.svelte` | Modify | Branch the `<main>` pill-tabs header + content section on `$activeView`. Terminal panel stays unconditional. |

---

## Task 1: Add `activeView` store

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts`

- [ ] **Step 1: Append the `activeView` store to `stores.ts`**

Open `apps/ccdash-ui/ui/src/lib/stores.ts` and append (at the bottom of the file, after the existing exports):

```ts
// === Top-level view (workspace vs. prompts) ===

function readActiveView(): 'workspace' | 'prompts' {
  try {
    const v = localStorage.getItem('ccdash.activeView');
    return v === 'prompts' ? 'prompts' : 'workspace';
  } catch {
    return 'workspace';
  }
}

export const activeView = writable<'workspace' | 'prompts'>(readActiveView());
activeView.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.activeView', v);
  } catch {}
});
```

- [ ] **Step 2: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: same warning/error count as before this task (no new errors). If the project was clean before, this should report `0 errors, 0 warnings`.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/stores.ts
git commit -m "feat(ui): activeView store for top-level page switching"
```

---

## Task 2: Add `Prompt` type, `prompts` store, and helpers

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts`

- [ ] **Step 1: Append the `Prompt` type and `prompts` store to `stores.ts`**

Append below the `activeView` block added in Task 1:

```ts
// === Prompts library ===

export type Prompt = {
  id: string;
  title: string;
  body: string;
  createdAt: number;
  updatedAt: number;
};

function readPrompts(): Prompt[] {
  try {
    const raw = localStorage.getItem('ccdash.prompts');
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    const out: Prompt[] = [];
    for (const p of parsed) {
      if (
        p &&
        typeof p === 'object' &&
        typeof p.id === 'string' &&
        typeof p.title === 'string' &&
        typeof p.body === 'string' &&
        Number.isFinite(p.createdAt) &&
        Number.isFinite(p.updatedAt)
      ) {
        out.push({
          id: p.id,
          title: p.title,
          body: p.body,
          createdAt: p.createdAt,
          updatedAt: p.updatedAt,
        });
      }
    }
    return out;
  } catch {
    return [];
  }
}

export const prompts = writable<Prompt[]>(readPrompts());
prompts.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.prompts', JSON.stringify(v));
  } catch {}
});

export function addPrompt(): string {
  const id = (globalThis.crypto as Crypto).randomUUID();
  const now = Date.now();
  prompts.update((arr) => [
    { id, title: '', body: '', createdAt: now, updatedAt: now },
    ...arr,
  ]);
  return id;
}

export function updatePrompt(
  id: string,
  patch: Partial<Pick<Prompt, 'title' | 'body'>>,
): void {
  prompts.update((arr) =>
    arr.map((p) => (p.id === id ? { ...p, ...patch, updatedAt: Date.now() } : p)),
  );
}

export function deletePrompt(id: string): void {
  prompts.update((arr) => arr.filter((p) => p.id !== id));
}
```

- [ ] **Step 2: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/stores.ts
git commit -m "feat(ui): prompts store + add/update/delete helpers"
```

---

## Task 3: Create `SidebarNav.svelte`

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/SidebarNav.svelte`

- [ ] **Step 1: Create the file**

Create `apps/ccdash-ui/ui/src/lib/components/SidebarNav.svelte` with the complete content:

```svelte
<script lang="ts">
  import { activeView } from '$lib/stores';
</script>

<nav class="sidebar-nav" aria-label="Primary">
  <button
    class="nav-item"
    class:active={$activeView === 'workspace'}
    on:click={() => activeView.set('workspace')}
    title="Workspace (Sessions, Ports, Plans, Browser)"
    aria-current={$activeView === 'workspace' ? 'page' : undefined}
  >
    <span class="icon" aria-hidden="true">📁</span>
    <span class="label">Workspace</span>
  </button>
  <button
    class="nav-item"
    class:active={$activeView === 'prompts'}
    on:click={() => activeView.set('prompts')}
    title="Prompts library"
    aria-current={$activeView === 'prompts' ? 'page' : undefined}
  >
    <span class="icon" aria-hidden="true">📝</span>
    <span class="label">Prompts</span>
  </button>
</nav>

<style>
  .sidebar-nav {
    display: flex;
    flex-direction: column;
    padding: 6px 6px 8px;
    border-bottom: 1px solid var(--border);
    gap: 2px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    background: transparent;
    color: var(--fg-dim);
    border: none;
    border-left: 2px solid transparent;
    border-radius: var(--r-sm);
    font-size: 12.5px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background var(--t-fast), color var(--t-fast);
  }
  .nav-item:hover { background: var(--bg-elev-2); color: var(--fg); }
  .nav-item.active {
    background: var(--accent-bg);
    color: var(--accent);
    border-left-color: var(--accent);
  }
  .nav-item .icon { font-size: 13px; line-height: 1; }
  .nav-item .label { flex: 1; }
</style>
```

- [ ] **Step 2: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors. (`SidebarNav` is not yet imported anywhere — that's fine.)

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/SidebarNav.svelte
git commit -m "feat(ui): SidebarNav component for Workspace/Prompts switch"
```

---

## Task 4: Restructure `Sidebar.svelte` to host the nav block

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte`

The current `<header>` (lines 261-274) holds: `<span class="title">Projects</span>` + `<button class="add">+</button>` + `<button class="collapse-btn">‹</button>`. After this task: the top `<header>` keeps only the collapse button. `<SidebarNav />` renders directly below. A new mini-header for the projects block holds the "Projects" title + `+` add button immediately above the project `<ul>`.

- [ ] **Step 1: Import `SidebarNav` in `Sidebar.svelte`**

Add this line in the `<script>` block of `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte`, alongside the other imports (after the `import { truncateBranch } from '$lib/format';` line):

```ts
  import SidebarNav from './SidebarNav.svelte';
```

- [ ] **Step 2: Replace the top `<header>` with a slim collapse-only header, mount the nav, and add a projects mini-header**

In `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte`, locate this block (currently lines ~260-277):

```svelte
<aside>
  <header>
    <span class="title">Projects</span>
    <div class="header-actions">
      <button class="add" on:click={addProject} disabled={busy} title="Add project">+</button>
      {#if onCollapse}
        <button
          class="collapse-btn"
          on:click={onCollapse}
          title="Collapse sidebar (click ☰ to bring back)"
          aria-label="Collapse sidebar"
        >‹</button>
      {/if}
    </div>
  </header>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
```

Replace it with:

```svelte
<aside>
  <header class="sidebar-header">
    {#if onCollapse}
      <button
        class="collapse-btn"
        on:click={onCollapse}
        title="Collapse sidebar (click ☰ to bring back)"
        aria-label="Collapse sidebar"
      >‹</button>
    {/if}
  </header>
  <SidebarNav />
  <header class="projects-header">
    <span class="title">Projects</span>
    <button class="add" on:click={addProject} disabled={busy} title="Add project">+</button>
  </header>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
```

- [ ] **Step 3: Adjust the CSS for the two header styles**

In the same file, locate the existing `header { … }` rule (currently around line 396). Replace it with two rules — one for the sidebar-header (collapse-only) and one for the projects-header (title + add). Find and replace this block:

```css
  header {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--bg-elev);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  header .title {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1.4px;
    color: var(--fg-dim);
    font-weight: 600;
  }
  .header-actions { display: flex; gap: 4px; }
```

With:

```css
  .sidebar-header {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    background: var(--bg-elev);
    position: sticky;
    top: 0;
    z-index: 2;
  }
  .projects-header {
    padding: 10px 12px 6px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--bg-elev);
  }
  .projects-header .title {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1.4px;
    color: var(--fg-dim);
    font-weight: 600;
  }
```

(The `.header-actions` rule is no longer needed because the two buttons are split across two headers; delete that rule entirely.)

- [ ] **Step 4: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors.

- [ ] **Step 5: Visual hand-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm run dev
```
Then start the Tauri shell (or reload an already-running one). Verify:
- The sidebar's top row contains only the `‹` collapse button (right-aligned).
- Directly below: two nav buttons (`📁 Workspace`, `📝 Prompts`) — Workspace is highlighted (active).
- Directly below that: a "PROJECTS" mini-header with the `+` add button.
- The projects list looks identical to before. Clicking `+` still opens the add-project dialog. Right-click on a project still opens the remove-project context menu. Drag-and-drop reordering still works.

- [ ] **Step 6: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte
git commit -m "feat(ui): sidebar header split + mount SidebarNav above projects"
```

---

## Task 5: Create `PromptsView.svelte` — left list pane

This task builds the left pane (list + search + `+ New`) and the no-selection placeholder on the right. The right-pane editor is added in Task 6.

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`

- [ ] **Step 1: Create the file with the list pane and a no-selection placeholder**

Create `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`:

```svelte
<script lang="ts">
  import { prompts, addPrompt } from '$lib/stores';
  import type { Prompt } from '$lib/stores';

  let selectedId: string | null = null;
  let query = '';

  $: sorted = [...$prompts].sort((a, b) => b.updatedAt - a.updatedAt);
  $: filtered = (() => {
    const q = query.trim().toLowerCase();
    if (!q) return sorted;
    return sorted.filter(
      (p) =>
        p.title.toLowerCase().includes(q) || p.body.toLowerCase().includes(q),
    );
  })();
  $: selected = $prompts.find((p) => p.id === selectedId) ?? null;

  function selectPrompt(p: Prompt) {
    selectedId = p.id;
  }

  function newPrompt() {
    const id = addPrompt();
    selectedId = id;
  }
</script>

<div class="prompts-root">
  <aside class="list-pane">
    <div class="list-toolbar">
      <input
        type="search"
        class="search"
        placeholder="Search prompts…"
        bind:value={query}
      />
      <button class="new-btn" on:click={newPrompt} title="New prompt">+ New</button>
    </div>
    {#if filtered.length === 0}
      <div class="empty">
        {#if $prompts.length === 0}
          No prompts yet — click <kbd>+ New</kbd> to add one.
        {:else}
          No prompts match "{query}".
        {/if}
      </div>
    {:else}
      <ul class="list">
        {#each filtered as p (p.id)}
          <li
            class:active={p.id === selectedId}
            role="button"
            tabindex="0"
            on:click={() => selectPrompt(p)}
            on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectPrompt(p); } }}
          >
            <span class="row-title">{p.title || 'Untitled'}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </aside>
  <section class="editor-pane">
    {#if selected}
      <div class="placeholder">Editor coming in next task — selected: {selected.title || 'Untitled'}</div>
    {:else}
      <div class="placeholder">Select a prompt or click <kbd>+ New</kbd>.</div>
    {/if}
  </section>
</div>

<style>
  .prompts-root {
    display: flex;
    flex: 1;
    min-height: 0;
    background: var(--bg);
  }
  .list-pane {
    width: 280px;
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .list-toolbar {
    display: flex;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .search {
    flex: 1;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 4px 8px;
    font-size: 12px;
  }
  .search:focus { outline: none; border-color: var(--accent); }
  .new-btn {
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    border-radius: var(--r-sm);
    padding: 4px 10px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
  }
  .new-btn:hover { filter: brightness(1.08); }
  .list { list-style: none; margin: 0; padding: 4px 0; overflow-y: auto; }
  .list li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    font-size: 12.5px;
    color: var(--fg);
    cursor: pointer;
    border-left: 2px solid transparent;
    transition: background var(--t-fast);
  }
  .list li:hover { background: var(--bg-elev-2); }
  .list li.active {
    background: var(--accent-bg);
    color: var(--accent);
    border-left-color: var(--accent);
  }
  .row-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 24px 16px;
    color: var(--fg-dim);
    font-size: 12px;
    text-align: center;
    line-height: 1.6;
  }
  .empty kbd {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    font-family: var(--mono);
    font-size: 11px;
  }
  .editor-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 13px;
  }
  .placeholder kbd {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    font-family: var(--mono);
    font-size: 11px;
    margin: 0 2px;
  }
</style>
```

- [ ] **Step 2: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte
git commit -m "feat(ui): PromptsView list pane + search + new button"
```

---

## Task 6: Add the editor pane to `PromptsView.svelte`

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`

The right pane gains a title input, body textarea, and Save / Delete buttons. Save and Delete operate on the selected prompt. Edits are kept in a local dirty buffer until the user hits Save. (`Copy` button is wired in Task 7 alongside the per-row copy.)

- [ ] **Step 1: Add a dirty-buffer reactive block to the `<script>`**

In `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`, modify the `<script>` block:

Replace this line:
```ts
  import { prompts, addPrompt } from '$lib/stores';
```
with:
```ts
  import { prompts, addPrompt, updatePrompt, deletePrompt } from '$lib/stores';
  import { tick } from 'svelte';
```

Then add the dirty-buffer logic immediately after the existing `$: selected = …` line:

```ts
  let titleBuf = '';
  let bodyBuf = '';
  let lastSyncedId: string | null = null;

  $: if (selected && selected.id !== lastSyncedId) {
    titleBuf = selected.title;
    bodyBuf = selected.body;
    lastSyncedId = selected.id;
  } else if (!selected && lastSyncedId !== null) {
    titleBuf = '';
    bodyBuf = '';
    lastSyncedId = null;
  }

  $: isDirty =
    !!selected && (titleBuf !== selected.title || bodyBuf !== selected.body);

  let titleInput: HTMLInputElement | null = null;

  async function focusTitle() {
    await tick();
    titleInput?.focus();
  }

  function saveSelected() {
    if (!selected || !isDirty) return;
    updatePrompt(selected.id, { title: titleBuf, body: bodyBuf });
  }

  function revertSelected() {
    if (!selected) return;
    titleBuf = selected.title;
    bodyBuf = selected.body;
  }

  function deleteSelected() {
    if (!selected) return;
    if (!confirm('Delete prompt?')) return;
    const id = selected.id;
    deletePrompt(id);
    if (selectedId === id) selectedId = null;
  }

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      saveSelected();
    } else if (e.key === 'Escape') {
      revertSelected();
    }
  }
```

Also replace the existing `newPrompt` function:

```ts
  function newPrompt() {
    const id = addPrompt();
    selectedId = id;
  }
```

with:

```ts
  function newPrompt() {
    const id = addPrompt();
    selectedId = id;
    focusTitle();
  }
```

- [ ] **Step 2: Replace the editor pane markup**

In the same file, find the `<section class="editor-pane">…</section>` block and replace it with:

```svelte
  <section class="editor-pane">
    {#if selected}
      <div class="editor" on:keydown={onKey} role="form">
        <label class="field">
          <span class="field-label">Title</span>
          <input
            type="text"
            bind:value={titleBuf}
            bind:this={titleInput}
            placeholder="Prompt title"
          />
        </label>
        <label class="field body-field">
          <span class="field-label">Body</span>
          <textarea
            bind:value={bodyBuf}
            placeholder="Prompt body — copied to your clipboard when you hit Copy."
            spellcheck="false"
          ></textarea>
        </label>
        <div class="actions">
          <button class="copy" disabled title="Wired up in Task 7">Copy</button>
          <button class="save" on:click={saveSelected} disabled={!isDirty}>Save{isDirty ? ' *' : ''}</button>
          <button class="delete" on:click={deleteSelected}>Delete</button>
        </div>
      </div>
    {:else}
      <div class="placeholder">Select a prompt or click <kbd>+ New</kbd>.</div>
    {/if}
  </section>
```

- [ ] **Step 3: Add editor styles**

In the same file's `<style>` block, append (at the end, before `</style>`):

```css
  .editor {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 18px;
    min-height: 0;
  }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .field-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1.2px;
    color: var(--fg-dim);
    font-weight: 600;
  }
  .field input,
  .field textarea {
    width: 100%;
    background: var(--bg-elev);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 7px 10px;
    font-size: 13px;
    font-family: var(--sans);
  }
  .field input:focus,
  .field textarea:focus { outline: none; border-color: var(--accent); }
  .body-field { flex: 1; min-height: 0; }
  .body-field textarea {
    flex: 1;
    min-height: 0;
    height: 100%;
    resize: none;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.55;
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  .actions button {
    padding: 5px 14px;
    font-size: 12px;
    font-weight: 600;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--fg);
    cursor: pointer;
  }
  .actions button:disabled { opacity: 0.5; cursor: not-allowed; }
  .actions button:hover:not(:disabled) { background: var(--bg-elev-2); border-color: var(--border-strong); }
  .actions .copy:not(:disabled) {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }
  .actions .delete {
    color: var(--state-error);
    border-color: color-mix(in srgb, var(--state-error) 40%, var(--border));
  }
  .actions .delete:hover { background: var(--state-error-bg); }
```

- [ ] **Step 4: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors.

- [ ] **Step 5: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte
git commit -m "feat(ui): PromptsView editor pane with dirty buffer + save/delete"
```

---

## Task 7: Wire copy-to-clipboard (right pane + per-row buttons)

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`

- [ ] **Step 1: Import `showToast` and add the copy helper**

In `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`, add this import alongside the others in the `<script>` block:

```ts
  import { showToast } from '$lib/toast';
```

Add this function inside the `<script>` block (anywhere after the existing function declarations):

```ts
  async function copyToClipboard(body: string) {
    try {
      await navigator.clipboard.writeText(body);
      showToast('Prompt copied to clipboard');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      showToast(`Copy failed: ${msg}`, 'err');
    }
  }

  function copySelected() {
    if (!selected) return;
    void copyToClipboard(selected.body);
  }

  function copyRow(ev: MouseEvent, body: string) {
    ev.stopPropagation();
    void copyToClipboard(body);
  }
```

- [ ] **Step 2: Activate the right-pane `Copy` button**

Find this line in the editor markup (added in Task 6):

```svelte
          <button class="copy" disabled title="Wired up in Task 7">Copy</button>
```

Replace with:

```svelte
          <button class="copy" on:click={copySelected} title="Copy body to clipboard">Copy</button>
```

- [ ] **Step 3: Add the per-row copy button to the list**

Find the list `<li>` markup added in Task 5:

```svelte
          <li
            class:active={p.id === selectedId}
            role="button"
            tabindex="0"
            on:click={() => selectPrompt(p)}
            on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectPrompt(p); } }}
          >
            <span class="row-title">{p.title || 'Untitled'}</span>
          </li>
```

Replace with:

```svelte
          <li
            class:active={p.id === selectedId}
            role="button"
            tabindex="0"
            on:click={() => selectPrompt(p)}
            on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectPrompt(p); } }}
          >
            <span class="row-title">{p.title || 'Untitled'}</span>
            <button
              class="row-copy"
              on:click={(ev) => copyRow(ev, p.body)}
              title="Copy body to clipboard"
              aria-label="Copy {p.title || 'Untitled'} to clipboard"
            >⎘</button>
          </li>
```

- [ ] **Step 4: Style the per-row copy button**

Append to the `<style>` block:

```css
  .row-copy {
    width: 22px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    color: var(--fg-mute);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .list li:hover .row-copy { border-color: var(--border); color: var(--fg-dim); }
  .row-copy:hover { background: var(--bg-elev-2); color: var(--fg) !important; border-color: var(--border-strong) !important; }
  .list li.active .row-copy { color: var(--accent); border-color: transparent; }
```

- [ ] **Step 5: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors.

- [ ] **Step 6: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte
git commit -m "feat(ui): copy-to-clipboard on right pane + per-row button"
```

---

## Task 8: Branch `App.svelte` main column on `$activeView`

**Files:**
- Modify: `apps/ccdash-ui/ui/src/App.svelte`

> **Context for the implementer:** the workspace is no longer pill-tabs (Sessions/Ports/Plans/Browser). The main column now renders `<PaneContainer />` inside `<section class="content">`, with a slim `<header>` above it containing the layout-direction toggle and the actions group. The branch wraps both the `<header>` and the `<section class="content">` so that switching to Prompts hides the workspace chrome entirely. The terminal panel block below stays outside the branch.

- [ ] **Step 1: Add `activeView` to the `$lib/stores` import**

In `apps/ccdash-ui/ui/src/App.svelte`, locate the import block from `$lib/stores` (currently around lines 7-25). It looks like this:

```ts
  import {
    connectError,
    connected,
    mirrorTarget,
    nextRetryAt,
    plans,
    ports,
    projects,
    reconnecting,
    selectedProjectId,
    sessions,
    attachedSessions,
    activeTerminalSessionId,
    detectedUrlsBySession,
    terminalCollapsed,
    sidebarWidth,
    sidebarCollapsed,
    terminalPanelHeight,
  } from '$lib/stores';
```

Add `activeView,` at the top of this list (one new line). After the edit it should read:

```ts
  import {
    activeView,
    connectError,
    connected,
    mirrorTarget,
    nextRetryAt,
    plans,
    ports,
    projects,
    reconnecting,
    selectedProjectId,
    sessions,
    attachedSessions,
    activeTerminalSessionId,
    detectedUrlsBySession,
    terminalCollapsed,
    sidebarWidth,
    sidebarCollapsed,
    terminalPanelHeight,
  } from '$lib/stores';
```

Do not touch the separate line `import { addPane, paneLayoutDirection } from '$lib/stores';` — it stays as-is.

- [ ] **Step 2: Add the `PromptsView` import**

In the same file, find the line:

```ts
  import Sidebar from '$lib/components/Sidebar.svelte';
```

Add immediately below it:

```ts
  import PromptsView from '$lib/components/PromptsView.svelte';
```

- [ ] **Step 3: Branch the workspace header + content on `$activeView`**

Locate this block inside `<main>` (currently around lines 337-373, immediately below the `{#if $reconnecting}` reconnect-banner block):

```svelte
    <header>
      <button
        class="layout-toggle"
        on:click={() => paneLayoutDirection.update((d) => (d === 'row' ? 'column' : 'row'))}
        title={$paneLayoutDirection === 'row' ? 'Switch to column layout' : 'Switch to row layout'}
        aria-label="Toggle pane layout direction"
      >{$paneLayoutDirection === 'row' ? '⇄' : '⇅'}</button>
      <div class="actions">
        <button class="primary" on:click={() => (launchOpen = true)} title="Launch session (⌘L)">
          <span class="plus">+</span> Launch
        </button>
        <button class="secondary" on:click={addPane} title="Add a pane to this window">
          <span class="plus">+</span> Pane
        </button>
        <button class="secondary" on:click={() => windowsApi.openNew()} title="Open a new ccdash window (⌘N)">
          <span class="plus">+</span> Window
        </button>
        <button class="icon-btn" on:click={takeWindowScreenshot} title="Screenshot window to clipboard" aria-label="Screenshot window">⎙</button>
        {#if $otherWindowList.length > 0}
          <select value={$mirrorTarget ?? ''} on:change={onMirrorChange} title="Mirror another window">
            <option value="">independent</option>
            {#each $otherWindowList as w (w)}
              <option value={w}>follow {w}</option>
            {/each}
          </select>
        {/if}
        <select class="theme-select" value={$theme} on:change={onThemeChange} title="Theme">
          <option value="system">auto</option>
          <option value="dark">dark</option>
          <option value="light">light</option>
        </select>
        <span class="health health-{healthColor}" title={healthTitle} aria-label={healthTitle}></span>
      </div>
    </header>
    <section class="content">
      <PaneContainer />
    </section>
```

Wrap the entire block in an `{#if $activeView === 'workspace'} … {:else} <PromptsView /> {/if}` branch. After the edit it should read:

```svelte
    {#if $activeView === 'workspace'}
      <header>
        <button
          class="layout-toggle"
          on:click={() => paneLayoutDirection.update((d) => (d === 'row' ? 'column' : 'row'))}
          title={$paneLayoutDirection === 'row' ? 'Switch to column layout' : 'Switch to row layout'}
          aria-label="Toggle pane layout direction"
        >{$paneLayoutDirection === 'row' ? '⇄' : '⇅'}</button>
        <div class="actions">
          <button class="primary" on:click={() => (launchOpen = true)} title="Launch session (⌘L)">
            <span class="plus">+</span> Launch
          </button>
          <button class="secondary" on:click={addPane} title="Add a pane to this window">
            <span class="plus">+</span> Pane
          </button>
          <button class="secondary" on:click={() => windowsApi.openNew()} title="Open a new ccdash window (⌘N)">
            <span class="plus">+</span> Window
          </button>
          <button class="icon-btn" on:click={takeWindowScreenshot} title="Screenshot window to clipboard" aria-label="Screenshot window">⎙</button>
          {#if $otherWindowList.length > 0}
            <select value={$mirrorTarget ?? ''} on:change={onMirrorChange} title="Mirror another window">
              <option value="">independent</option>
              {#each $otherWindowList as w (w)}
                <option value={w}>follow {w}</option>
              {/each}
            </select>
          {/if}
          <select class="theme-select" value={$theme} on:change={onThemeChange} title="Theme">
            <option value="system">auto</option>
            <option value="dark">dark</option>
            <option value="light">light</option>
          </select>
          <span class="health health-{healthColor}" title={healthTitle} aria-label={healthTitle}></span>
        </div>
      </header>
      <section class="content">
        <PaneContainer />
      </section>
    {:else}
      <PromptsView />
    {/if}
```

The terminal panel block immediately below (`{#if $attachedSessions.length > 0}…`) is **not** moved — it stays outside the branch and renders unconditionally, exactly as it does today.

- [ ] **Step 3: Type-check**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm exec svelte-check --tsconfig ./tsconfig.json
```
Expected: no new errors.

- [ ] **Step 4: Build**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm run build
```
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add apps/ccdash-ui/ui/src/App.svelte
git commit -m "feat(ui): branch main column on activeView (workspace vs prompts)"
```

---

## Task 9: Hand-test acceptance

This task is not code — it is the verification gate before declaring the work complete.

- [ ] **Step 1: Start dev**

Run from `apps/ccdash-ui/ui/`:
```bash
pnpm run dev
```
Then start the Tauri shell from the parent (`apps/ccdash-ui/`):
```bash
cargo tauri dev
```
(Or whatever the project's normal dev-launch command is — `Cargo.toml` and `tauri.conf.json` define it.)

- [ ] **Step 2: Walk the acceptance list**

Confirm each item visually / by interaction:

1. **Sidebar shape:** top header has only the `‹` collapse button. Below it, the Workspace / Prompts nav block (Workspace highlighted). Below that, the "PROJECTS" mini-header with the `+` button. Below that, the projects list. All existing project interactions (drag-reorder, right-click → remove, expand chevron, click-to-select) still work.

2. **Switch to Prompts:** click `📝 Prompts`. The workspace header (layout-toggle + actions row) and `<PaneContainer />` disappear; PromptsView appears in their place. The terminal panel at the bottom (if a session is attached) stays mounted.

3. **Add three prompts:** click `+ New` three times. Each adds a row labeled "Untitled" at the top of the list and selects it. Type a unique title and body for each. Click `Save` after each — the asterisk on the Save button vanishes when the buffer matches the stored copy.

4. **Reload window:** ⌘R (or restart the Tauri shell). All three prompts persist and re-appear in the list. The view is still Prompts (activeView persisted too).

5. **Edit + Save:** select one prompt, change its body, click `Save`. Switch to another prompt and back — the change survives. Now make another edit and hit `⌘S` — same outcome.

6. **Esc revert:** select a prompt, edit the body, hit `Esc`. The textarea reverts to the stored copy. Save button greys out.

7. **Copy (right pane):** select a prompt, click `Copy`. Paste into a terminal — body matches. Toast appears at the bottom-right ("Prompt copied to clipboard").

8. **Copy (per-row):** click the `⎘` button on a different (unselected) row. Paste — body matches that row. The selected prompt is unchanged. Toast appears.

9. **Delete:** click `Delete` on a selected prompt. Confirm. The prompt disappears from the list. Reload — still gone.

10. **Search:** type into the search box. The list filters case-insensitively by title and body. Clear the search — full list returns. Search for a string that matches nothing — placeholder reads `No prompts match "foo".`

11. **Sidebar collapse:** click the `‹` button. The entire sidebar (nav + projects + everything) hides; the floating `☰` appears. Click it — sidebar reappears with state intact.

12. **Workspace return:** click `📁 Workspace`. The workspace header (layout-toggle + actions) reappears and `<PaneContainer />` is mounted again with the panes in the layout they had before the switch (the `panes` store is persisted, so the same pane shape comes back).

- [ ] **Step 3: If anything in the acceptance list fails — file the failure as a fix-up task and address it before declaring complete.**

No commit for this task — it's a verification gate.
