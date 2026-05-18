# Browser scoping & in-window panes — design

Status: approved for planning
Date: 2026-05-17
Branch: phase-1-foundation
Affected app: `apps/ccdash-ui`

## Problem

The current Browser tab is global: it lists every attached tmux session machine-wide and uses a dropdown to switch context. When the user is working inside a project, that's noise — they want to see only that project's sessions. Separately, the upper content area is a single-pane tab strip (Sessions / Ports / Plans / Browser); the user wants tmux-style splits so they can, for example, watch a Browser preview and a Plans list side-by-side. Today they have to flip tabs to do that.

There are also two distinct "new" actions the toolbar conflates: opening a new OS-level application window (today's `⊞` button) and creating an in-window pane. The user wants both, clearly labeled.

## Goals

- Browser view is scoped to the project selected in the sidebar.
- Each in-scope session appears as a sub-tab inside the Browser pane; each sub-tab keeps the independent browser state it already has today.
- The upper content area becomes a flat row or column of independent panes. Each pane independently picks its content type (Browser / Plans / Sessions / Ports) and tracks its own state.
- Two clearly labeled toolbar buttons: `+ Window` (opens a new app window — existing behavior, just renamed) and `+ Pane` (adds a pane to the current window).
- The bottom terminal panel and the sidebar are unchanged.

## Non-goals (V1)

- Nested splits / tmux tree. Flat row or flat column only, no recursion.
- Splitting the bottom terminal panel. It keeps its existing session-tabs UI.
- Drag-and-drop reordering of panes. Use the close button + `+ Pane` to rearrange.
- Saved layout presets.
- Backend / Rust changes. This is UI-only.

## Final layout

```
┌────────┬──────────────────────────────────────┐
│        │ [+Window] [+Pane] [⇄] [Launch] ⎙ ⤵   │ ← toolbar (no global tabs)
│ Side-  │ ┌──────────────┬─────────────────┐   │
│ bar    │ │[Browser ▾] ✕ │[Plans ▾]      ✕ │   │ ← per-pane header
│        │ │              │                 │   │
│        │ │  body        │     body        │   │
│        │ └──────────────┴─────────────────┘   │
│        │ ─── terminal panel (unchanged) ───   │
└────────┴──────────────────────────────────────┘
```

The `⇄` button toggles the upper area between row layout (panes side-by-side) and column layout (panes stacked vertically).

## Browser pane internal layout

```
┌────────────────────────────────────────┐
│ [Browser ▾]                         ✕  │ ← pane header
├────────────────────────────────────────┤
│ [api-dev*] [worker] [ui-dev]           │ ← session sub-tabs (project-scoped)
├────────────────────────────────────────┤
│ ‹  ›  ↻  http://localhost:3000      ⇗  │ ← chrome bar
├──────────────┬─────────────────────────┤
│ DETECTED     │                         │
│ :3000        │       (iframe)          │
│ :5173        │                         │
└──────────────┴─────────────────────────┘
```

Sub-tabs replace the existing "Browser for:" dropdown and the "follow active" button.

## Scope logic

- Project context: read `$selectedProjectId` from `stores.ts`.
- A session belongs to the scope if `session.project_id === $selectedProjectId`. If no project is selected, all attached sessions are in scope ("All" view).
- The sub-tab strip lists in-scope sessions in `$attachedSessions` order.
- Each Browser pane stores its own selected sub-tab session id. When `$selectedProjectId` changes, if the pane's selected session is no longer in scope, fall back to the first in-scope session (or `null` if none).
- Machine-wide detected URLs (the `null` key in `detectedUrlsBySession`) always appear in the rail, independent of project.

## Pane model

Each pane is:

```ts
type PaneType = 'browser' | 'plans' | 'sessions' | 'ports';

type Pane = {
  id: string;              // uuid, stable for the lifetime of the pane
  type: PaneType | null;   // null = "empty pane, awaiting type selection"
};
```

Per-pane state is kept in dedicated stores keyed by `pane.id` (not collapsed onto the `Pane` object) so the layout array stays small and JSON-serializable:

- `browserPaneSubtabByPaneId: Map<paneId, sessionId>` — which sub-tab a Browser pane is showing.
- (Future pane types add their own keyed stores as needed.)

Plans / Sessions / Ports panes derive everything from existing global stores (`$plans`, `$sessions`, `$ports`) plus `$selectedProjectId`, so they don't need per-pane state in V1.

## New / modified components

### New: `apps/ccdash-ui/ui/src/lib/components/PaneContainer.svelte`

Renders the row or column of panes with draggable `Splitter`s between them. Subscribes to `$panes` and `$paneLayoutDirection`. Owns the layout flex direction. Reuses the existing `Splitter.svelte`.

### New: `apps/ccdash-ui/ui/src/lib/components/Pane.svelte`

Pane chrome: a header with a content-type dropdown and a close (`✕`) button. The body renders the chosen view component based on `pane.type`:

- `browser` → `<BrowserView paneId={pane.id} />`
- `plans` → `<PlansView />`
- `sessions` → `<SessionsView />`
- `ports` → `<PortsView />`
- `null` → empty placeholder with the type-picker dropdown open

Close button is disabled when `$panes.length === 1`.

### Modified: `apps/ccdash-ui/ui/src/App.svelte`

- Remove the global tabs block (`<div class="tabs">…` and `setTab`).
- Replace `<section class="content">` body with `<PaneContainer />`.
- In `.actions`: rename the existing `⊞` icon button to a labeled `+ Window` button (still wired to `windowsApi.openNew()`); add a `+ Pane` button that appends `{ id: uuid(), type: null }` to `$panes`; add the `⇄` layout-direction toggle.
- Terminal panel block (lines ~410–473) is unchanged.
- Sidebar block is unchanged.

### Modified: `apps/ccdash-ui/ui/src/lib/components/BrowserView.svelte`

- Accept a `paneId: string` prop.
- Drop the `"Browser for:"` dropdown, the `userOverride`/`followActive` machinery, and the global `$activeTerminalSessionId`-follow behavior.
- Derive `inScopeSessions` reactively from `$attachedSessions`, `$sessions`, and `$selectedProjectId`.
- Render a sub-tab strip across the top. Selected sub-tab is read/written via `browserPaneSubtabByPaneId.get(paneId)`.
- `state`, navigation, `sortedUrls`, screenshot, etc. continue to key off the selected session id — internal logic stays.

### Modified: `apps/ccdash-ui/ui/src/lib/stores.ts`

Add:

```ts
export type PaneType = 'browser' | 'plans' | 'sessions' | 'ports';
export type Pane = { id: string; type: PaneType | null };

export const panes = writable<Pane[]>(/* loaded from localStorage, see below */);
export const paneLayoutDirection = writable<'row' | 'column'>(/* from LS */);
export const browserPaneSubtabByPaneId = writable<Map<string, string>>(new Map());
```

Remove (or repurpose) `activeTab`. If a sensible default for `+ Pane` is wanted, keep `activeTab` as "last-used pane type" so a new pane preselects something familiar; otherwise drop it. V1 leaves the dropdown open on `null` panes, so `activeTab` can be removed.

### Persistence

`panes` and `paneLayoutDirection` subscribe-to-write to localStorage in the same pattern as `terminalCollapsed` and `sidebarWidth`:

- Key `ccdash.panes` → JSON array of `Pane` objects.
- Key `ccdash.paneLayoutDirection` → `'row'` or `'column'`.

On first launch (no stored value): default to a single Browser pane in row layout.

`browserPaneSubtabByPaneId` does NOT persist — the pane's `paneId` is durable, but the selected sub-tab is recomputed from current scope on load.

## Data flow

1. Sidebar mutates `$selectedProjectId` (unchanged).
2. `PaneContainer` renders one `Pane` per entry in `$panes`, with `Splitter`s between.
3. Each `Pane` mounts the view component matching `pane.type`.
4. `BrowserView` reactively filters in-scope sessions from `$selectedProjectId` × `$attachedSessions`; it renders one sub-tab per in-scope session and shows the iframe for `browserPaneSubtabByPaneId.get(paneId)`.
5. `+ Pane` appends a new pane to `$panes`. Pane header dropdown picks the type and writes it back into the pane.
6. `✕` removes the pane from `$panes` (only when `panes.length > 1`).
7. `⇄` flips `$paneLayoutDirection`.

## Edge cases

- **Last pane can't be closed.** `✕` is disabled when `$panes.length === 1`. Closing the only Browser pane otherwise leaves an empty workspace.
- **Empty scope.** If no sessions match the selected project, the Browser pane shows the existing "No URL loaded" placeholder, and the sub-tab strip is empty. The detected-URLs rail still shows machine-wide ports (so the user can navigate without an attached session).
- **Project switch invalidates selected sub-tab.** When `$selectedProjectId` changes, each Browser pane checks whether its `browserPaneSubtabByPaneId` value is still in scope. If not, fall back to the first in-scope session (or `null`).
- **Many sub-tabs.** The sub-tab strip scrolls horizontally, same as `.term-tabs` does today (`App.svelte:730`).
- **localStorage corruption / stale pane ids.** On load, validate the stored `panes` shape; on parse failure, reset to the single-Browser-pane default. Per-pane state stores are populated on demand.
- **Restored panes whose data sources are empty.** A Plans pane in a project with no plans simply shows the existing empty state; no special handling needed.

## What is NOT changing

- The terminal panel below (`App.svelte:410-473`) including its tab strip for multiple attached sessions, pop-out, collapse, and splitter.
- The sidebar (`Sidebar.svelte`).
- All Rust / Tauri commands, the daemon protocol, `pty.rs`, `screenshot.rs`, etc.
- `browserStateBySession` shape — keyed by session id, untouched.
- `detectedUrlsBySession` shape — keyed by session id with `null` for machine-wide, untouched.
- The popped-out single-session window mode (`App.svelte:292-319`).

## Test surface

Manual smoke tests V1 must pass:

1. Fresh launch (no localStorage) → one Browser pane visible.
2. Click `+ Pane`, pick Plans → two panes side by side, splitter draggable.
3. Toggle `⇄` → two panes stacked, splitter draggable on the other axis.
4. Close one pane → only one remains; `✕` on the remaining pane is disabled.
5. Open multiple Browser panes; each can show a different session's sub-tab.
6. Select a project in the sidebar → Browser sub-tabs filter to that project's attached sessions; URL & history preserved per session.
7. Select a different project; previously selected sub-tab no longer in scope → falls back to first in-scope (or empty placeholder).
8. `+ Window` opens a second OS window with the same default (one Browser pane).
9. Reload the app → pane layout (count, types, direction) restored from localStorage; selected sub-tabs recomputed.

## Open questions (deferred, not blocking V1)

- Should the Ports view be project-scoped too? Currently global / machine-wide. Different conceptually — ports are OS resources, not project artifacts. Punt to V2 unless the user weighs in.
- Min/max pane widths and how to handle window-too-narrow with many panes. V1 uses the existing `Splitter` clamp behavior; revisit if it bites.
- Keyboard shortcuts for `+ Pane`, layout toggle, close-pane. Not in V1.
