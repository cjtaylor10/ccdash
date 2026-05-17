# Phase 10: Polish + niceties — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Round out the v0.6 release with a batch of small UX wins — keyboard shortcuts, drag-reorder, markdown plan rendering, search/filter, theme toggle, real app icon, branch-name truncation, daemon-health indicator.

**Architecture:** Most are isolated frontend changes. Two cross-cutting additions: (1) a new `settings.toml` for theme persistence (frontend-only — no daemon protocol involvement, so persist via `localStorage`), (2) a small JS markdown renderer (`marked`).

---

## Items

### 10.1 Keyboard shortcuts
- `Cmd+N` → new window
- `Cmd+W` → close window
- `Cmd+K` → command palette (project switcher + actions)

**File:** create `apps/ccdash-ui/ui/src/lib/keybinds.ts` with a global event listener registered in `App.svelte::onMount`. Cmd+W close uses `getCurrentWindow().close()`. Cmd+N uses the existing `windowsApi.openNew()`. Cmd+K opens a new `CommandPalette.svelte` modal.

### 10.2 Command palette
Create `CommandPalette.svelte`: input + filtered list. Actions:
- "Switch to project: <name>" (sets `selectedProjectId`)
- "Launch session" (opens LaunchDialog)
- "Add project" (calls addProjectViaPicker)
- "New window" / "Close window"
- "Switch tab: Sessions/Ports/Plans/Browser"

### 10.3 Drag-and-drop reorder for projects in Sidebar
Native HTML5 drag/drop on `<li>` elements. On drop, send the new order to the daemon via a new RPC `project.reorder { ids: Vec<String> }`. Persists to projects.toml.

**Daemon changes:**
- New RPC method `project.reorder` in `dispatch.rs` + `handlers.rs`.
- `ProjectsRegistry::reorder(ids: &[ProjectId])` reorders the internal Vec and persists.

### 10.4 Markdown rendering for PlansView
Replace the bullet-list rendering with `marked` markdown render. Click-to-jump to file location via a separate "Open in VS Code" button per file path (vscode://file/{path} via `openExternal`).

**Dep add:** `marked` (small, ~30kb).

### 10.5 Session search/filter
In `SessionsView.svelte`: add a search input above the table that filters on session name + cwd + tmux_session_id. Show the input only when `$sessions.length > 10`. Live-filter.

### 10.6 Theme toggle
Create `apps/ccdash-ui/ui/src/lib/theme.ts`: read/write a `theme: 'light' | 'dark' | 'system'` from `localStorage`. Add a theme picker (small select) in App.svelte top bar. CSS variables already exist in `theme.css`; add `[data-theme="light"]` variants.

### 10.7 Worktree branch name middle-ellipsis
If `branch.length > 24`, render `branch.slice(0,12) + '…' + branch.slice(-9)`. Helper in `lib/format.ts`.

### 10.8 Daemon health indicator
A small colored dot in App.svelte's top bar:
- green: `$connected === true`
- yellow: `$reconnecting === true`
- red: `!$connected && !$reconnecting`

Tooltip explains the state.

### 10.9 Real app icon
Generate a 1024×1024 RGBA glyph via Python+PIL: a stylized terminal cursor block ("▮") in the accent color on a deep-dark background. Use `iconutil`/Tauri's bundler to derive the `.icns` and all the PNG sizes.

**Design choice:** filled rectangle with a tiny inset top-right notch, accent color `#ff7a2f` (warm orange, matching dark UI). Background `#1a1b1e`.

(If the user dislikes it post-ship, easy to swap.)

---

## Workspace gate + release v0.6.0

Same as prior phases: bump versions, release.sh, formula sha update, tap push, gh release, brew verify, exec log, tags.
