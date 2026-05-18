# Browser scoping & in-window panes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the global tab bar with a flat row/column of independent panes, scope the Browser view to the selected project, and expose explicit `+ Window` and `+ Pane` toolbar buttons.

**Architecture:** Two new Svelte components (`Pane`, `PaneContainer`) own the upper content area. Pane layout is a flat array persisted to localStorage. Per-pane state (currently only the Browser pane's selected sub-tab) lives in dedicated maps keyed by `paneId`. The bottom terminal panel and the sidebar are untouched. UI-only — no Rust changes.

**Tech Stack:** Svelte 5, TypeScript, Vite, Tauri (existing). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-17-browser-scope-and-panes-design.md`

**Testing approach:** This codebase has no Svelte component test framework. Each task's verification is (a) TypeScript / svelte-check passes, and (b) manual smoke test against `pnpm tauri dev`. The spec's "Test surface" section enumerates the 9 manual smokes covered across these tasks.

**Pre-flight assumptions to verify before Task 3 and Task 5:**

1. **`Splitter.svelte` API (used in Task 3).** App.svelte currently uses `bind:value={$sidebarWidth}` on it (around `App.svelte:329`). Task 3 below uses `on:change` for parity with a per-pane callback. Open `apps/ccdash-ui/ui/src/lib/components/Splitter.svelte` first — if it doesn't dispatch a `change` event, switch Task 3 to `bind:value` with a local writable per pane.
2. **`Session.project_id` field (used in Task 5).** Task 5's scope filter assumes `Session` has a `project_id` field. Open `apps/ccdash-ui/ui/src/lib/tauri.ts` and confirm. If the field is named differently (e.g. `projectId`, `pid`) or sessions are linked through a parent map, adjust the filter expression in Task 5 step 1 accordingly. If sessions truly don't carry a project link, Task 5 needs a small daemon-side change — out of scope for this plan; flag and stop.

---

## Task 1: Add new stores + persistence

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts`

This task introduces the state foundation. After this commit, nothing in the UI changes visually — the stores are defined but unused.

- [ ] **Step 1: Add the new types and stores to `stores.ts`**

Append the following to `apps/ccdash-ui/ui/src/lib/stores.ts` (after the existing exports, before the reconnect block):

```ts
// === Panes (upper content area) ===

export type PaneType = 'browser' | 'plans' | 'sessions' | 'ports';

/** A pane in the upper content area. `type === null` means the user clicked
 *  `+ Pane` but hasn't yet picked a content type — the pane renders an
 *  empty placeholder with the type-picker dropdown open. */
export type Pane = {
  id: string;
  type: PaneType | null;
};

function makePaneId(): string {
  // crypto.randomUUID exists in all modern browsers and Tauri's webview.
  // The cast handles tsconfig DOM lib versions that haven't added the type.
  return (globalThis.crypto as Crypto).randomUUID();
}

function readPanes(): Pane[] {
  try {
    const raw = localStorage.getItem('ccdash.panes');
    if (!raw) return [{ id: makePaneId(), type: 'browser' }];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0) {
      return [{ id: makePaneId(), type: 'browser' }];
    }
    // Validate shape; drop anything that doesn't match.
    const valid: Pane[] = [];
    for (const p of parsed) {
      if (
        p &&
        typeof p === 'object' &&
        typeof p.id === 'string' &&
        (p.type === null ||
          p.type === 'browser' ||
          p.type === 'plans' ||
          p.type === 'sessions' ||
          p.type === 'ports')
      ) {
        valid.push({ id: p.id, type: p.type });
      }
    }
    if (valid.length === 0) return [{ id: makePaneId(), type: 'browser' }];
    return valid;
  } catch {
    return [{ id: makePaneId(), type: 'browser' }];
  }
}

export const panes = writable<Pane[]>(readPanes());
panes.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.panes', JSON.stringify(v));
  } catch {}
});

function readPaneLayoutDirection(): 'row' | 'column' {
  try {
    const v = localStorage.getItem('ccdash.paneLayoutDirection');
    return v === 'column' ? 'column' : 'row';
  } catch {
    return 'row';
  }
}

export const paneLayoutDirection = writable<'row' | 'column'>(
  readPaneLayoutDirection(),
);
paneLayoutDirection.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.paneLayoutDirection', v);
  } catch {}
});

/** Width (row mode) or height (column mode) of each pane keyed by pane id.
 *  Missing entries → pane gets equal share of remaining space. NOT persisted
 *  in V1 — sizes reset on reload, layout shape does not. */
export const paneSizeById = writable<Map<string, number>>(new Map());

/** Which session sub-tab a given Browser pane is showing. Not persisted —
 *  recomputed from in-scope sessions on load. */
export const browserPaneSubtabByPaneId = writable<Map<string, string>>(
  new Map(),
);

/** Helper used by the toolbar `+ Pane` button. Appends an empty pane to the
 *  end of the layout. */
export function addPane(): void {
  panes.update((arr) => [...arr, { id: makePaneId(), type: null }]);
}

/** Helper used by each pane's `✕` button. No-op when called on the last
 *  remaining pane (the workspace always has at least one pane). */
export function removePane(id: string): void {
  panes.update((arr) => (arr.length <= 1 ? arr : arr.filter((p) => p.id !== id)));
  browserPaneSubtabByPaneId.update((m) => {
    if (!m.has(id)) return m;
    const next = new Map(m);
    next.delete(id);
    return next;
  });
  paneSizeById.update((m) => {
    if (!m.has(id)) return m;
    const next = new Map(m);
    next.delete(id);
    return next;
  });
}

export function setPaneType(id: string, type: PaneType): void {
  panes.update((arr) => arr.map((p) => (p.id === id ? { ...p, type } : p)));
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run from `apps/ccdash-ui/ui`:

```bash
pnpm check
```

Expected: zero errors, zero warnings related to `stores.ts`.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/stores.ts
git commit -m "feat(ui): pane stores + persistence scaffolding"
```

---

## Task 2: Create `Pane.svelte`

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/Pane.svelte`

A single pane: header with content-type dropdown and close button, body that mounts the chosen view. Renders nothing useful until `PaneContainer` wires it up in Task 3.

- [ ] **Step 1: Write the component**

Create `apps/ccdash-ui/ui/src/lib/components/Pane.svelte` with:

```svelte
<script lang="ts">
  import { panes, removePane, setPaneType, type PaneType } from '$lib/stores';
  import BrowserView from './BrowserView.svelte';
  import PlansView from './PlansView.svelte';
  import SessionsView from './SessionsView.svelte';
  import PortsView from './PortsView.svelte';

  /** Pane id this component is rendering. Used to look up per-pane state. */
  export let paneId: string;
  /** Current pane type (or null = empty). */
  export let type: PaneType | null;

  $: canClose = $panes.length > 1;

  function onTypeChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value as PaneType;
    setPaneType(paneId, v);
  }

  function onClose() {
    if (!canClose) return;
    removePane(paneId);
  }
</script>

<div class="pane">
  <header class="pane-header">
    <select
      class="type-picker"
      value={type ?? ''}
      on:change={onTypeChange}
      aria-label="Pane content type"
    >
      {#if type === null}
        <option value="" disabled>Pick content…</option>
      {/if}
      <option value="browser">Browser</option>
      <option value="plans">Plans</option>
      <option value="sessions">Sessions</option>
      <option value="ports">Ports</option>
    </select>
    <button
      class="close"
      on:click={onClose}
      disabled={!canClose}
      title={canClose ? 'Close pane' : 'Cannot close the last pane'}
      aria-label="Close pane"
    >✕</button>
  </header>
  <div class="pane-body">
    {#if type === 'browser'}
      <BrowserView {paneId} />
    {:else if type === 'plans'}
      <PlansView />
    {:else if type === 'sessions'}
      <SessionsView />
    {:else if type === 'ports'}
      <PortsView />
    {:else}
      <div class="empty">
        <p>Pick a content type from the dropdown above.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .pane {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg);
  }
  .pane-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .type-picker {
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 2px 6px;
    font-size: 11.5px;
  }
  .type-picker:hover { border-color: var(--border-strong); }
  .close {
    margin-left: auto;
    width: 22px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    color: var(--fg-mute);
    border-radius: var(--r-sm);
    font-size: 12px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .close:hover:not(:disabled) {
    color: var(--state-error);
    border-color: var(--border);
    background: var(--bg-elev-2);
  }
  .close:disabled { opacity: 0.3; cursor: not-allowed; }
  .pane-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-mute);
    font-size: 12px;
    padding: 24px;
  }
</style>
```

Note: this component imports `BrowserView` and passes a `paneId` prop. BrowserView doesn't yet accept that prop — it will at Task 5. Until then the prop is silently ignored (Svelte warns but compiles).

- [ ] **Step 2: Verify TypeScript / svelte-check compiles**

Run from `apps/ccdash-ui/ui`:

```bash
pnpm check
```

Expected: no errors. There may be a warning about the `paneId` prop being passed to `BrowserView` (which doesn't yet declare it). Acceptable until Task 5.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/Pane.svelte
git commit -m "feat(ui): Pane component — type picker + close button"
```

---

## Task 3: Create `PaneContainer.svelte`

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/PaneContainer.svelte`

Renders the array of panes with draggable `Splitter`s between them. Owns the flex direction and pane sizing logic.

- [ ] **Step 1: Write the component**

Create `apps/ccdash-ui/ui/src/lib/components/PaneContainer.svelte`:

```svelte
<script lang="ts">
  import { panes, paneLayoutDirection, paneSizeById } from '$lib/stores';
  import Pane from './Pane.svelte';
  import Splitter from './Splitter.svelte';

  /** Update the stored size for a pane after the user drags its splitter.
   *  We store explicit pixel sizes so panes don't reflow when a sibling
   *  resizes — only the dragged pane and its left/upper neighbour change. */
  function setSize(id: string, v: number) {
    paneSizeById.update((m) => {
      const next = new Map(m);
      next.set(id, v);
      return next;
    });
  }

  function sizeFor(id: string): number | null {
    return $paneSizeById.get(id) ?? null;
  }
</script>

<div class="container" class:row={$paneLayoutDirection === 'row'} class:column={$paneLayoutDirection === 'column'}>
  {#each $panes as pane, i (pane.id)}
    {#if i > 0}
      {@const prev = $panes[i - 1]}
      {@const initial = sizeFor(prev.id) ?? 320}
      <Splitter
        orientation={$paneLayoutDirection === 'row' ? 'horizontal' : 'vertical'}
        value={initial}
        min={160}
        max={2000}
        on:change={(e) => setSize(prev.id, (e as CustomEvent<number>).detail)}
      />
    {/if}
    {@const explicit = sizeFor(pane.id)}
    <div
      class="pane-slot"
      style={explicit !== null && i < $panes.length - 1
        ? `flex: 0 0 ${explicit}px;`
        : 'flex: 1 1 0;'}
    >
      <Pane paneId={pane.id} type={pane.type} />
    </div>
  {/each}
</div>

<style>
  .container {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .container.row { flex-direction: row; }
  .container.column { flex-direction: column; }
  .pane-slot {
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
</style>
```

**Splitter API check:** the existing `Splitter.svelte` uses `bind:value` in App.svelte (e.g. `bind:value={$sidebarWidth}` at `App.svelte:329`). If `Splitter` does not emit a `change` CustomEvent, replace the `<Splitter on:change=…>` line with `bind:value` against a local writable per pane and a `$: setSize(prev.id, localSize)` reactive statement. The user (or implementer) will discover this in step 2's compile/run.

- [ ] **Step 2: Verify svelte-check passes**

Run from `apps/ccdash-ui/ui`:

```bash
pnpm check
```

Expected: clean. If `Splitter` rejects `on:change`, switch to `bind:value` per the note above and re-run.

- [ ] **Step 3: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/PaneContainer.svelte
git commit -m "feat(ui): PaneContainer — flat row/column layout with splitters"
```

---

## Task 4: Wire panes into `App.svelte`, drop the global tab bar, add toolbar buttons

**Files:**
- Modify: `apps/ccdash-ui/ui/src/App.svelte`

After this task, the upper content area is the pane container. The global Sessions/Ports/Plans/Browser tab bar is gone. `+ Window`, `+ Pane`, and the row/column toggle live in the toolbar. The Browser pane still uses the pre-Task-5 BrowserView (no project scoping, no sub-tabs yet); that's intentional — Task 5 finishes Browser.

- [ ] **Step 1: Update the imports block in `App.svelte`**

In `apps/ccdash-ui/ui/src/App.svelte`, **remove** these imports (no longer used after this task):

```ts
import SessionsView from '$lib/components/SessionsView.svelte';
import PortsView from '$lib/components/PortsView.svelte';
import PlansView from '$lib/components/PlansView.svelte';
import BrowserView from '$lib/components/BrowserView.svelte';
```

(These views are now imported inside `Pane.svelte` instead.)

**Add** the pane-related imports:

```ts
import PaneContainer from '$lib/components/PaneContainer.svelte';
import { addPane, paneLayoutDirection } from '$lib/stores';
```

In the existing `$lib/stores` import, **remove** `activeTab` from the destructured import list (it's no longer used).

- [ ] **Step 2: Remove `setTab` and counts that drove the old tab bar**

In `App.svelte`, delete the `setTab` function and the `$:` reactive counts that fed the old tab badges:

Remove:
```ts
function setTab(t: 'sessions' | 'ports' | 'plans' | 'browser') {
  activeTab.set(t);
}

$: sessionsCount = $sessions.filter((s) => s.state === 'running').length;
$: portsCount = $ports.running.length;
$: plansCount = $plans.length;
$: totalDetectedUrls = (() => {
  let n = 0;
  for (const urls of $detectedUrlsBySession.values()) n += urls.size;
  return n;
})();
```

Also remove the `detectedUrlsBySession` import if it's no longer referenced elsewhere in `App.svelte` (search for other uses first — `refreshTopLevel` may still write to it; if so, keep the import).

- [ ] **Step 3: Replace the global tabs in the header markup**

In `App.svelte`, find the `<header>` block containing the `.tabs` div (the one with the Sessions/Ports/Plans/Browser buttons) and **replace the entire `.tabs` div** with a layout toggle:

```svelte
<button
  class="layout-toggle"
  on:click={() => paneLayoutDirection.update((d) => (d === 'row' ? 'column' : 'row'))}
  title={$paneLayoutDirection === 'row' ? 'Switch to column layout' : 'Switch to row layout'}
  aria-label="Toggle pane layout direction"
>{$paneLayoutDirection === 'row' ? '⇄' : '⇅'}</button>
```

- [ ] **Step 4: Update the action buttons in the toolbar**

In the `.actions` div, replace the existing `+ Launch` block and `⊞` icon button with labeled buttons. The new layout (full `.actions` block):

```svelte
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
```

- [ ] **Step 5: Replace `<section class="content">` body with PaneContainer**

Find the existing block:

```svelte
<section class="content">
  {#if $activeTab === 'sessions'}
    <SessionsView />
  {:else if $activeTab === 'ports'}
    <PortsView />
  {:else if $activeTab === 'plans'}
    <PlansView />
  {:else}
    <BrowserView />
  {/if}
</section>
```

Replace it with:

```svelte
<section class="content">
  <PaneContainer />
</section>
```

- [ ] **Step 6: Add styles for `.secondary` and `.layout-toggle`**

In the `<style>` block of `App.svelte`, add:

```css
.actions .secondary {
  background: transparent;
  color: var(--fg-dim);
  border: 1px solid var(--border);
  padding: 4px 10px;
  font-size: 12px;
  font-weight: 500;
  border-radius: var(--r-sm);
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
}
.actions .secondary:hover { color: var(--fg); border-color: var(--border-strong); background: var(--bg-elev-2); }
.actions .secondary .plus { font-weight: 400; font-size: 14px; line-height: 1; opacity: 0.9; }

.layout-toggle {
  width: 28px;
  height: 26px;
  padding: 0;
  background: var(--bg);
  border: 1px solid var(--border);
  color: var(--fg-dim);
  border-radius: var(--r-sm);
  font-size: 14px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.layout-toggle:hover { color: var(--fg); border-color: var(--border-strong); background: var(--bg-elev-2); }
```

Then remove the now-unused styles for the old tab bar: the `.tabs`, `.pill`, `.pill:hover`, `.pill.active`, `.count`, `.count.pulse`, and `@keyframes pulse-pop` rules. Keep `.health-*` and `.actions .primary` rules.

- [ ] **Step 7: Verify svelte-check passes**

```bash
cd apps/ccdash-ui/ui && pnpm check
```

Expected: clean. If anything references `$activeTab`, you missed a removal — search and delete those lines.

- [ ] **Step 8: Manual smoke test**

```bash
cd apps/ccdash-ui && pnpm tauri dev
```

(or whatever the project's dev command is — check `Cargo.toml`/`tauri.conf.json` if unsure)

Verify:
1. App opens to a single Browser pane (or empty pane if first launch hit the default elsewhere — should be Browser).
2. Toolbar has `+ Launch`, `+ Pane`, `+ Window`, `⇄` (or `⇅`), and the rest of the existing controls.
3. Click `+ Pane` — a new empty pane appears next to the first; its header dropdown is at "Pick content…".
4. Pick "Plans" from the dropdown — pane body renders the plans view.
5. The splitter between panes is draggable.
6. Click `⇄` — panes restack vertically; splitter axis flips.
7. Click `✕` on one pane — it disappears; the remaining pane's `✕` is now disabled.
8. Click `+ Window` — a second ccdash window opens, defaulted to a single Browser pane.
9. Reload the app — pane layout (count, types, direction) is restored from localStorage.

If any of 1–9 fail, fix before committing.

- [ ] **Step 9: Commit**

```bash
git add apps/ccdash-ui/ui/src/App.svelte
git commit -m "feat(ui): replace global tab bar with PaneContainer + toolbar buttons"
```

---

## Task 5: Refactor `BrowserView` to accept `paneId` + project-scoped session sub-tabs

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/BrowserView.svelte`

Drops the "Browser for:" dropdown and follow-active button. Adds a session sub-tab strip filtered by the sidebar's selected project. Each Browser pane tracks its own selected sub-tab via `browserPaneSubtabByPaneId`.

- [ ] **Step 1: Update the script block**

In `apps/ccdash-ui/ui/src/lib/components/BrowserView.svelte`, replace the existing imports and the `viewSession` / `userOverride` / `followActive` logic.

**Replace** this:

```ts
import {
  activeTerminalSessionId,
  attachedSessions,
  browserStateBySession,
  detectedUrlsBySession,
  sessions,
} from '$lib/stores';
```

**With:**

```ts
import {
  attachedSessions,
  browserPaneSubtabByPaneId,
  browserStateBySession,
  detectedUrlsBySession,
  selectedProjectId,
  sessions,
} from '$lib/stores';
```

**Add a prop near the top of the script block:**

```ts
export let paneId: string;
```

**Replace** this block:

```ts
let viewSession: string | null = $activeTerminalSessionId;

let userOverride = false;
$: if (!userOverride) viewSession = $activeTerminalSessionId;
```

**With:**

```ts
/** Attached sessions that belong to the currently-selected project. When
 *  no project is selected, fall back to every attached session. */
$: inScopeSessions = (() => {
  const pid = $selectedProjectId;
  const allSessions = $sessions;
  return $attachedSessions.filter((t) => {
    if (pid === null) return true;
    const sess = allSessions.find((s) => s.tmux_session_id === t.sessionId);
    return sess?.project_id === pid;
  });
})();

/** This pane's selected sub-tab. Reads from the per-pane map; if stale
 *  (no longer in scope) or unset, falls back to the first in-scope
 *  session. `null` if no sessions are in scope at all. */
$: viewSession = (() => {
  const stored = $browserPaneSubtabByPaneId.get(paneId);
  if (stored && inScopeSessions.some((t) => t.sessionId === stored)) return stored;
  if (inScopeSessions.length > 0) return inScopeSessions[0].sessionId;
  return null;
})();

function selectSubtab(sessionId: string) {
  browserPaneSubtabByPaneId.update((m) => {
    const next = new Map(m);
    next.set(paneId, sessionId);
    return next;
  });
}
```

**Delete** the `onContextChange` and `followActive` functions; they're replaced by `selectSubtab`.

The rest of the script — `defaultState`, `state`, `current`, `canBack`/`canForward`, `sortedUrls`, `updateState`, `navigate`, `go`, `back`, `forward`, `reload`, `external`, `iframeHost`, `snapshot`, `onAddressInput`, `onAddressKeydown`, and `contextLabel` — stays as-is. They all key off `viewSession`, which is now derived per-pane.

- [ ] **Step 2: Replace the context bar markup with a session sub-tab strip**

In the template section, find the `.context-bar` block:

```svelte
<div class="context-bar">
  <span class="ctx-label">Browser for:</span>
  <select class="ctx-select" value={viewSession ?? '__all__'} on:change={onContextChange}>
    <option value="__all__">All sessions</option>
    {#each $attachedSessions as t (t.sessionId)}
      {@const sess = $sessions.find((s) => s.tmux_session_id === t.sessionId)}
      <option value={t.sessionId}>
        {sess?.name ?? t.sessionId} ({t.sessionId})
      </option>
    {/each}
  </select>
  {#if userOverride && $activeTerminalSessionId && viewSession !== $activeTerminalSessionId}
    <button class="follow-btn" on:click={followActive} title="Follow the currently-attached session">
      ↺ follow active
    </button>
  {/if}
  <span class="ctx-name">{contextLabel}</span>
</div>
```

**Replace** with:

```svelte
{#if inScopeSessions.length > 0}
  <div class="subtabs">
    {#each inScopeSessions as t (t.sessionId)}
      {@const sess = $sessions.find((s) => s.tmux_session_id === t.sessionId)}
      <button
        class="subtab"
        class:active={t.sessionId === viewSession}
        on:click={() => selectSubtab(t.sessionId)}
        title={t.sessionId}
      >
        <span class="subtab-name">{sess?.name ?? t.sessionId}</span>
        <code>{t.sessionId}</code>
      </button>
    {/each}
  </div>
{:else}
  <div class="subtabs subtabs-empty">
    <span>No attached sessions in this project. Launch one to start browsing.</span>
  </div>
{/if}
```

- [ ] **Step 3: Update styles**

In the `<style>` block of `BrowserView.svelte`, **remove** `.context-bar`, `.ctx-label`, `.ctx-select`, `.follow-btn`, and `.ctx-name` rules.

**Add**:

```css
.subtabs {
  display: flex;
  gap: 2px;
  padding: 4px 8px;
  background: var(--bg-elev);
  border-bottom: 1px solid var(--border);
  overflow-x: auto;
  flex-shrink: 0;
}
.subtab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: 1px solid var(--border);
  color: var(--fg-dim);
  border-radius: var(--r-sm);
  padding: 3px 9px;
  font-size: 11px;
  font-family: var(--sans);
  cursor: pointer;
  flex-shrink: 0;
}
.subtab:hover {
  color: var(--fg);
  border-color: var(--border-strong);
  background: var(--bg-elev-2);
}
.subtab.active {
  background: var(--accent-bg-strong);
  color: var(--accent);
  border-color: var(--accent);
}
.subtab.active code { color: var(--accent); }
.subtab code {
  font-family: var(--mono);
  color: var(--fg-mute);
  font-size: 10.5px;
}
.subtab-name {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.subtabs-empty {
  padding: 8px 12px;
  color: var(--fg-mute);
  font-size: 11.5px;
  font-style: italic;
}
```

- [ ] **Step 4: Sanity-check downstream behavior**

In the same file, confirm:
- `state` still reads from `$browserStateBySession.get(viewSession) ?? defaultState()`. (`viewSession` may now be `null` when no in-scope session — `defaultState()` already covers that.)
- `sortedUrls` still merges `m.get(null)` (machine-wide) with `m.get(viewSession)` for the current sub-tab.
- `updateState` still writes to `next.set(viewSession, s)` — when `viewSession === null`, this writes to the same machine-wide bucket as before. That's still correct: typing a URL with no session selected is essentially a scratch buffer.

No code changes needed in step 4 — this is a read-through confirmation.

- [ ] **Step 5: Verify svelte-check passes**

```bash
cd apps/ccdash-ui/ui && pnpm check
```

Expected: clean. Any warnings about unused `userOverride` / `followActive` / `onContextChange` mean step 1's deletion missed something — fix.

- [ ] **Step 6: Manual smoke test**

```bash
cd apps/ccdash-ui && pnpm tauri dev
```

Verify the full spec test surface:

1. Fresh launch → single Browser pane.
2. Browser pane shows sub-tabs for every attached session (no project selected → all).
3. Select a project in the sidebar → sub-tabs narrow to that project's attached sessions.
4. Switch to a different project → if the previously selected sub-tab is out of scope, it falls back to the first in-scope session; URL/history of each session preserved.
5. Project with zero attached sessions → "No attached sessions in this project" placeholder; detected URLs rail still shows machine-wide ports.
6. Open two Browser panes (`+ Pane` → Browser). Each can pick a different sub-tab independently.
7. Detected URLs rail still surfaces machine-wide ports + the active session's per-session URLs.
8. Address bar, back/forward, reload, snapshot, "open in external" all still work per sub-tab.
9. Reload the app → pane layout restored; each Browser pane's selected sub-tab recomputes from current scope (does not persist by design).

If any fail, fix before committing.

- [ ] **Step 7: Commit**

```bash
git add apps/ccdash-ui/ui/src/lib/components/BrowserView.svelte
git commit -m "feat(ui): project-scoped session sub-tabs in BrowserView"
```

---

## Post-implementation: full smoke run

After Task 5, run the full spec test surface end-to-end one more time, ideally with two ccdash windows open simultaneously (verify they don't interfere — each has its own pane layout in localStorage).

If anything from the spec's "Test surface" section fails, file a follow-up commit; the V1 plan is done when all 9 manual smokes pass.

---

## Self-review checklist (for the implementer)

Before merging:

- [ ] No `activeTab` references remain anywhere in the codebase (`grep -rn activeTab apps/ccdash-ui/ui/src`).
- [ ] No `userOverride` / `followActive` / `onContextChange` references remain in `BrowserView.svelte`.
- [ ] Reloading the app preserves pane count, types, and layout direction.
- [ ] Reloading does NOT preserve which sub-tab was active inside each Browser pane (by design — recomputed from scope).
- [ ] Last-pane close button is disabled.
- [ ] `+ Window` opens a new ccdash window that itself has a single Browser pane.
- [ ] The bottom terminal panel works exactly as it did before this PR (open a session, attach to it, multi-tab, pop out, collapse).
