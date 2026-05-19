# Phase 8: First-run + onboarding — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** New users get a welcome flow on first launch: pick scan roots, see discovered repos, bulk-approve. The scanner module (built in Phase 1 but unused since then) finally gets a consumer. Empty states across tabs guide users when there's nothing to show.

**Architecture:** Daemon tracks first-run state via a single `AtomicBool` (set on bootstrap, cleared on user confirmation). Three new RPC methods plumb the flow. Frontend gets a WelcomeModal that pickers scan roots via the dialog plugin, runs the daemon scan, shows the results with checkboxes, and bulk-adds via existing `project.add`.

---

## Daemon

### Task 1: Track first_run_pending in AppState

**Files:**
- Modify: `crates/ccdash-daemon/src/projects/registry.rs` — add `was_new_on_disk` field tracking whether projects.toml existed at load time
- Modify: `crates/ccdash-daemon/src/state.rs` — expose `first_run_pending: Arc<AtomicBool>` on AppState

The atomic is set to `true` at bootstrap iff `projects.toml` didn't exist on disk. `first_run_complete` RPC clears it.

### Task 2: Protocol types

**Files:**
- Modify: `crates/ccdash-core/src/protocol.rs` — add `FirstRunStatusResult`, `ScanPathsParams`, `ScanPathsResult`

```rust
pub struct FirstRunStatusResult { pub pending: bool }
pub struct ScanPathsParams { pub roots: Vec<PathBuf> }
pub struct ScanPathsResult { pub discovered: Vec<DiscoveredRepo> }
pub struct DiscoveredRepo { pub path: PathBuf, pub suggested_name: String }
```

### Task 3: RPC methods

**Files:**
- Modify: `crates/ccdash-daemon/src/rpc/dispatch.rs`
- Modify: `crates/ccdash-daemon/src/rpc/handlers.rs`

Add: `daemon.first_run_status`, `daemon.scan_paths`, `daemon.first_run_complete`. Wire `daemon.scan_paths` to call `projects::scanner::scan` and convert results into `DiscoveredRepo` with `suggested_name` derived from the trailing path component.

### Task 4: Daemon tests

Add tests in `state.rs` for first_run_pending = true when projects.toml absent / false when present. Add a test in handlers.rs for scan_paths and first_run_complete.

---

## Frontend

### Task 5: Tauri command wrappers

**Files:**
- Modify: `apps/ccdash-ui/src/commands.rs` — three new commands: `first_run_status`, `scan_paths`, `first_run_complete`
- Modify: `apps/ccdash-ui/src/main.rs` — register them
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts` — typed API: `daemonApi.firstRunStatus()`, `daemonApi.scanPaths(roots)`, `daemonApi.firstRunComplete()`

### Task 6: WelcomeModal component

**Files:**
- Create: `apps/ccdash-ui/ui/src/lib/components/WelcomeModal.svelte`

Modal flow:
1. Welcome message + "We'll scan some directories for git repos" copy.
2. List of scan roots with "+ Add root" button (uses `@tauri-apps/plugin-dialog::open({directory:true})`).
3. Default suggested root: `~/Documents` (compute via `os.homeDir()` from `@tauri-apps/plugin-os`, OR hardcode by detecting the platform). To keep simple, hardcode `~/Documents` as the suggested root since this is a personal-first tool.
4. "Scan" button calls `scanPaths`. Loading spinner.
5. Result list with checkboxes (all checked by default). User unchecks any they don't want.
6. "Add selected" button bulk-calls `projectsApi.add` for each, then `firstRunComplete`, then closes.
7. "Skip for now" button just calls `firstRunComplete` and closes.

### Task 7: Wire WelcomeModal in App.svelte

After connect, call `daemonApi.firstRunStatus()`; if pending, mount `<WelcomeModal />`.

### Task 8: Empty states

**Files:**
- Modify: `apps/ccdash-ui/ui/src/lib/components/SessionsView.svelte` — when `$projects.length === 0`, show "Add a project to get started" with a button that opens the same Add-project dialog as the sidebar.
- Modify: `apps/ccdash-ui/ui/src/lib/components/PortsView.svelte` — same treatment.
- Modify: `apps/ccdash-ui/ui/src/lib/components/PlansView.svelte` — same.

For now, the button in each tab can simply call the same `addProject` flow (extract it into `$lib/projectActions.ts` so all three tabs + sidebar share it).

---

## Task 9: Workspace gate + release v0.4.0

(Same template as Phase 6/7: version bump, release.sh, formula sha update, tap push, gh release, brew verify, exec log, tags.)
