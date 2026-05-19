# Prompts Library + Sidebar Navigation

**Date:** 2026-05-17
**Status:** Approved (design)
**Scope:** UI-only (Svelte). No daemon, RPC, or schema changes.

## Motivation

The dashboard currently has one sidebar (Projects) and one main content area (Sessions / Ports / Plans / Browser pill-tabs). The user wants a place to curate a personal library of reusable prompts that can be copied to the OS clipboard with one click. That feature doesn't fit cleanly into any existing pill-tab — it's not scoped to a project, session, port, or plan — so it needs its own top-level page.

This spec introduces a lightweight sidebar navigation block above the Projects list to switch between two pages: the existing **Workspace** (Sessions/Ports/Plans/Browser) and a new **Prompts** library. The mechanism is deliberately one-off for V1 — no view registry, no plugin system — but is shaped so a third or fourth page can be added later by appending one button and one store value.

## Out of scope (V1)

- Cross-window sync of prompts.
- Daemon-side persistence (prompts live in localStorage).
- Tags, folders, or any organizational hierarchy beyond a flat list.
- Variables, placeholders, or templating (e.g. `{{name}}` substitution).
- Importing or exporting prompts.
- A general view-registry / plugin system. The Workspace ↔ Prompts switch is a hardcoded two-option enum.
- Touching the daemon or any Rust code.
- The in-flight pane-and-browser-scope work (`docs/superpowers/specs/2026-05-17-browser-scope-and-panes-design.md`) — orthogonal.

## Architecture

### View model

A new store `activeView: Writable<'workspace' | 'prompts'>` is added to `apps/ccdash-ui/ui/src/lib/stores.ts`, persisted to localStorage under `ccdash.activeView`. Defaults to `'workspace'` so the first launch after the update is visually identical to the prior version.

The store follows the same read/subscribe pattern as `sidebarCollapsed` and `terminalCollapsed` already in `stores.ts`:

```ts
function readActiveView(): 'workspace' | 'prompts' {
  try {
    const v = localStorage.getItem('ccdash.activeView');
    return v === 'prompts' ? 'prompts' : 'workspace';
  } catch { return 'workspace'; }
}
export const activeView = writable<'workspace' | 'prompts'>(readActiveView());
activeView.subscribe((v) => {
  try { localStorage.setItem('ccdash.activeView', v); } catch {}
});
```

The existing `activeTab` store (`'sessions' | 'ports' | 'plans' | 'browser'`) is unchanged — it continues to govern which pill-tab is selected *within* the workspace view. When the user toggles to Prompts and back, the prior pill-tab is restored.

### Main column branching

`App.svelte` already renders `<main>` with three stacked pieces: a header (pill-tabs + actions), a content section, and an optional terminal panel. The branching is added at the level of the header + content section:

```svelte
{#if $activeView === 'workspace'}
  <header>…pill-tabs…</header>
  <section class="content">…SessionsView / PortsView / PlansView / BrowserView…</section>
{:else}
  <PromptsView />
{/if}
```

The terminal panel block (`{#if $attachedSessions.length > 0}…`) stays **outside** that branch — it renders unconditionally regardless of which view is active. Rationale: tmux owns the terminals, they exist independently of the UI, and hiding the panel when switching to Prompts would be more disruptive than helpful.

`PromptsView.svelte` is responsible for its own internal header (search + `+ New`), so it does not reuse `App.svelte`'s pill-tab `<header>`. The reconnect banner (`{#if $reconnecting}`) stays unconditional too — it's app-level chrome.

### Sidebar restructure

`Sidebar.svelte` currently has a single header (`Projects` title + `+` add button + `‹` collapse button) followed by the projects list. The new structure is:

```
┌──────────────────────────┐
│  ☰                     ‹ │   ← top-most header row (collapse button)
├──────────────────────────┤
│ 📁 Workspace          ◂  │   ← new SidebarNav block
│ 📝 Prompts               │
├──────────────────────────┤
│ Projects              +  │   ← existing projects header
│  • proj-a                │
│  • proj-b                │
└──────────────────────────┘
```

Changes inside `Sidebar.svelte`:

1. The top-most `<header>` keeps only the `‹` collapse button (driven by the existing `onCollapse` prop). The "Projects" title and the `+` add button are *removed* from this header.
2. A new component `<SidebarNav />` is rendered immediately below this header (see below).
3. A new mini-header is rendered for the projects block containing the "Projects" title and the `+` add button (the same `addProject()` handler stays). Below it, the existing `<ul>` of projects renders exactly as today.

All existing functionality (drag/drop reordering, expand/collapse trees, context menu, session attribution, worktree grouping) is preserved verbatim. The only structural change is two new horizontal divider lines and the insertion of the nav block.

### `SidebarNav.svelte` (new)

A small standalone component pulled out of `Sidebar.svelte` so the parent doesn't grow further (it's already 670+ lines). Renders two buttons:

```svelte
<nav class="sidebar-nav">
  <button class:active={$activeView === 'workspace'} on:click={() => activeView.set('workspace')}>
    📁 Workspace
  </button>
  <button class:active={$activeView === 'prompts'} on:click={() => activeView.set('prompts')}>
    📝 Prompts
  </button>
</nav>
```

Active-state styling reuses the existing accent treatment (`var(--accent-bg)`, left border accent) so the nav buttons feel consistent with active project rows.

### Prompts data model & storage

```ts
type Prompt = {
  id: string;         // crypto.randomUUID()
  title: string;
  body: string;
  createdAt: number;  // Date.now()
  updatedAt: number;  // Date.now()
};
```

Persisted in localStorage under `ccdash.prompts` as a JSON array. Read/write pattern matches the existing `panes` store in `stores.ts:121–152`:

- `readPrompts()` parses the stored JSON, validates each entry (`id` is string, `title` is string, `body` is string, `createdAt`/`updatedAt` are finite numbers), silently drops malformed entries, and returns `[]` if the stored value is missing or completely unparseable.
- `prompts.subscribe(...)` writes the array back on every change.

The list is sorted in the view layer (not in the store) by `updatedAt` descending — most-recently-edited first.

Helper functions exported alongside the store:

```ts
export function addPrompt(): string {
  const id = crypto.randomUUID();
  const now = Date.now();
  prompts.update((arr) => [
    { id, title: '', body: '', createdAt: now, updatedAt: now },
    ...arr,
  ]);
  return id;
}

export function updatePrompt(id: string, patch: Partial<Pick<Prompt, 'title' | 'body'>>): void {
  prompts.update((arr) =>
    arr.map((p) => (p.id === id ? { ...p, ...patch, updatedAt: Date.now() } : p)),
  );
}

export function deletePrompt(id: string): void {
  prompts.update((arr) => arr.filter((p) => p.id !== id));
}
```

Cross-window sync is out of scope. If two ccdash windows are open, each operates on its own in-memory copy; last write wins on reload. This matches the existing pattern for `panes`, `browserStateBySession`, etc.

### `PromptsView.svelte` (new)

A two-pane layout that fills the main content area:

```
┌──────────────────────────┬─────────────────────────────────────┐
│ 🔍 Search…       [+ New] │ Title:  [ Code review checklist  ]  │
├──────────────────────────┤                                     │
│ • Code review… [⎘]       │ Body:                               │
│ • Bug report… [⎘]      ◂ │ ┌─────────────────────────────────┐ │
│ • Standup update [⎘]     │ │ Review the PR and check…        │ │
│                          │ │ - Tests added                   │ │
│                          │ │ - Naming conventions            │ │
│                          │ └─────────────────────────────────┘ │
│                          │ [Copy]  [Save]  [Delete]            │
└──────────────────────────┴─────────────────────────────────────┘
```

**Left pane** — fixed width ~280px:

- Top row: search `<input>` (placeholder "Search prompts…") + `+ New` button.
- Below: the prompt list. Each row shows the prompt's title (or "Untitled" if blank), and a per-row copy button (`⎘`) at the right. Search filters by `title.includes(q) || body.includes(q)`, case-insensitive.
- Selected row highlighted with `var(--accent-bg)` and a left-border accent (same treatment as active project rows).
- Empty state when `$prompts` is empty: a centered hint reading "No prompts yet — click `+ New` to add one."

**Right pane** — flex:

- A `<input type="text">` for the title.
- A `<textarea>` for the body (monospace font, fills available vertical space).
- Below: three buttons — `[Copy]`, `[Save]`, `[Delete]`.
- Edits to title / body are kept in local component state ("dirty buffer") until `Save` is clicked. `Save` calls `updatePrompt(id, { title, body })` which bumps `updatedAt`.
- `Delete` triggers `confirm('Delete prompt?')`; on confirmation, calls `deletePrompt(id)` and clears selection.
- No-selection state: when no prompt is selected, the right pane shows a placeholder ("Select a prompt or click `+ New`").

**Interactions:**

- `+ New` calls `addPrompt()`, selects the new prompt, focuses the title input.
- Clicking a list row selects it; clicking another row when there are unsaved edits silently discards them (V1 behavior — explicit save model). The visual treatment makes unsaved state obvious: the right pane shows the dirty values but `Save` is enabled only when those differ from the stored copy.
- `Cmd+S` while the textarea or title input is focused triggers Save.
- `Esc` while editing reverts the dirty buffer to the stored copy.
- Per-row `⎘` button: copies that row's body to the clipboard and fires the toast, without changing which prompt is selected. The click handler stops propagation so the row's normal click (select) is suppressed.

### Clipboard

`navigator.clipboard.writeText(prompt.body)` — the standard browser Clipboard API works inside the Tauri webview without any Rust plumbing.

Success: `showToast('Prompt copied to clipboard')` using the existing `toast.ts` (`apps/ccdash-ui/ui/src/lib/toast.ts`).

Failure: `showToast(\`Copy failed: ${err.message}\`, 'err')`. Failure is rare — only triggers if the webview blocks clipboard access for some reason.

## File-level impact

**New files:**
- `apps/ccdash-ui/ui/src/lib/components/PromptsView.svelte`
- `apps/ccdash-ui/ui/src/lib/components/SidebarNav.svelte`

**Modified files:**
- `apps/ccdash-ui/ui/src/lib/stores.ts` — adds `Prompt` type, `prompts` store + helpers, `activeView` store.
- `apps/ccdash-ui/ui/src/lib/components/Sidebar.svelte` — refactors the sidebar header into header + nav block + projects block. Removes "Projects" title + `+` button from the top-most header; re-renders them inside a new mini-header above the projects `<ul>`. Imports and renders `<SidebarNav />`.
- `apps/ccdash-ui/ui/src/App.svelte` — branches the header + content section on `$activeView`. Terminal panel remains unconditional.

**Untouched:**
- All daemon / Rust code. No new RPC methods, no schema migrations.
- The panes / browser-scope spec work in flight.
- `activeTab` store and its consumers — they continue to govern the pill-tabs *within* the workspace view.

## Testing

The UI package (`apps/ccdash-ui/ui/package.json`) does not currently have a JS test runner installed. Adding one is out of scope for this spec — verification is by hand-testing against the built app.

**Hand-test acceptance** (post-build):

1. Add three prompts with distinct titles and bodies. Each appears in the list.
2. Edit one prompt's body, click Save, reload window — change survives.
3. Click `[Copy]` on the selected prompt's right pane. Paste into a terminal — body matches. Toast appears.
4. Click `⎘` on a different (unselected) row. Paste — that row's body matches. Toast appears. The selected prompt is unchanged.
5. Delete a prompt with confirmation — disappears from list and storage.
6. Toggle Workspace ↔ Prompts in the sidebar. The terminal panel stays attached if a session was open. Workspace's pill-tab selection is preserved across the toggle.
7. Collapse the sidebar — both the nav block and the projects block hide, and the floating `☰` button still expands the sidebar.
8. Search filters list reactively as you type.

## Risks & mitigations

- **localStorage quota:** prompts are plain text; even thousands of prompts won't approach the ~5MB quota. No mitigation needed.
- **Concurrent edits across windows:** two ccdash windows editing the prompts list will race. Same as today's `panes` store. Acceptable for V1 — flagged here for future work.
- **Clipboard API permission:** Tauri's webview grants clipboard access by default. If a future Tauri config tightens this, the copy button will fail gracefully with an error toast.
- **Existing sidebar layout regression:** the sidebar's drag/drop reordering, context menu, and worktree trees are intricate. The refactor only inserts new blocks above/around the existing `<ul>` — it does **not** edit the project-row or session-row markup. Hand-testing existing flows (drag a project, right-click → remove, attach a session) is part of the acceptance list.

## Future extensions (not in this spec)

- Tags / folders if the library grows past ~20 prompts.
- Cross-window sync via the daemon (moves storage from localStorage to `~/.ccdash/prompts.json`).
- Variables (`{{name}}`) substituted at copy time with a small inline form.
- Markdown preview in a third pane.
- A third sidebar nav item (e.g., Settings, Snippets) — the current shape supports this with one additional value in the `activeView` enum.
