<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    activeTerminalSessionId,
    attachedSessions,
    pathContains,
    projects,
    resolvedProjectByTmuxId,
    selectedProjectId,
    sessions as sessionsStore,
  } from '$lib/stores';
  import { projectsApi, tauri } from '$lib/tauri';
  import type { Project, Session } from '$lib/tauri';
  import { truncateBranch } from '$lib/format';
  import SidebarNav from './SidebarNav.svelte';

  /** Optional collapse callback — when provided, a button appears in the
   *  header that fully hides the sidebar (handled by parent). */
  export let onCollapse: (() => void) | null = null;

  let busy = false;
  let errMsg: string | null = null;

  let menuOpenForId: string | null = null;
  let menuX = 0;
  let menuY = 0;

  let dragId: string | null = null;
  let dragOverId: string | null = null;

  /** Per-project expanded state — defaults to true for the currently-selected
   *  project so its tree opens automatically. */
  let expanded: Record<string, boolean> = {};
  $: if ($selectedProjectId && expanded[$selectedProjectId] === undefined) {
    expanded[$selectedProjectId] = true;
  }

  function toggleExpand(id: string, ev: MouseEvent) {
    ev.stopPropagation();
    expanded[id] = !expanded[id];
  }

  function sessionsFor(projectId: string) {
    return $sessionsStore.filter(
      (s) => $resolvedProjectByTmuxId.get(s.tmux_session_id) === projectId,
    );
  }

  /** Pick the worktree a session belongs to within its owning project.
   *  Order: (1) match the daemon-stamped branch name `s.worktree` against
   *  `worktree.branch`; (2) longest-prefix `cwd` match on `worktree.path`;
   *  (3) fall back to the project's primary worktree so every session lands
   *  in exactly one bucket — orphans stay visible under `main` instead of
   *  silently disappearing. */
  function resolveWorktreePath(s: Session, p: Project): string {
    if (s.worktree) {
      const byBranch = p.worktrees.find((w) => w.branch === s.worktree);
      if (byBranch) return byBranch.path;
    }
    let bestPath: string | null = null;
    let bestLen = -1;
    for (const w of p.worktrees) {
      if (pathContains(s.cwd, w.path) && w.path.length > bestLen) {
        bestPath = w.path;
        bestLen = w.path.length;
      }
    }
    if (bestPath) return bestPath;
    return (p.worktrees.find((w) => w.is_primary) ?? p.worktrees[0])?.path ?? '';
  }

  /** projectId → worktreePath → sessions in that worktree. Recomputed
   *  reactively so newly-launched sessions appear in their bucket without
   *  user action. Only consulted for multi-worktree projects; single-
   *  worktree projects still render a flat sessions list. */
  $: sessionsByWorktree = (() => {
    const out: Record<string, Record<string, Session[]>> = {};
    for (const p of $projects) out[p.id] = {};
    for (const s of $sessionsStore) {
      const pid = $resolvedProjectByTmuxId.get(s.tmux_session_id);
      if (!pid) continue;
      const proj = $projects.find((p) => p.id === pid);
      if (!proj) continue;
      const wpath = resolveWorktreePath(s, proj);
      (out[pid][wpath] ??= []).push(s);
    }
    return out;
  })();

  /** Attach (or switch to) a session — same semantics as the SessionsView
   *  click handler. Centralized here so the sidebar tree and the main view
   *  share behavior. */
  function attachSession(sessionId: string) {
    const already = $attachedSessions.some((s) => s.sessionId === sessionId);
    if (!already) {
      attachedSessions.update((arr) => [
        ...arr,
        { sessionId, command: ['tmux', 'attach-session', '-t', sessionId] },
      ]);
    }
    activeTerminalSessionId.set(sessionId);
  }

  function select(id: string) {
    selectedProjectId.set(id);
  }

  /** Stable hash → hue mapping so every project gets a consistent color tag. */
  function hueForId(id: string): number {
    let h = 0;
    for (let i = 0; i < id.length; i++) {
      h = (h * 31 + id.charCodeAt(i)) >>> 0;
    }
    return h % 360;
  }

  function initials(name: string): string {
    const parts = name.replace(/[_-]/g, ' ').split(/\s+/).filter(Boolean);
    if (parts.length === 0) return '?';
    if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
    return (parts[0][0] + parts[1][0]).toUpperCase();
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

  function onDragStart(e: DragEvent, id: string) {
    dragId = id;
    e.dataTransfer?.setData('text/plain', id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }
  function onDragOver(e: DragEvent, id: string) {
    if (!dragId || dragId === id) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    dragOverId = id;
  }
  function onDragLeave(id: string) {
    if (dragOverId === id) dragOverId = null;
  }
  async function onDrop(e: DragEvent, targetId: string) {
    e.preventDefault();
    if (!dragId || dragId === targetId) {
      dragId = null;
      dragOverId = null;
      return;
    }
    const list = $projects.slice();
    const from = list.findIndex((p) => p.id === dragId);
    const to = list.findIndex((p) => p.id === targetId);
    if (from === -1 || to === -1) {
      dragId = null;
      dragOverId = null;
      return;
    }
    const [moved] = list.splice(from, 1);
    list.splice(to, 0, moved);
    projects.set(list);
    try {
      await projectsApi.reorder(list.map((p) => p.id));
    } catch (e) {
      errMsg = String(e);
      try {
        const { projects: ps } = await tauri.projectList();
        projects.set(ps);
      } catch {}
    } finally {
      dragId = null;
      dragOverId = null;
    }
  }
</script>

<svelte:window on:click={closeMenu} />

<aside>
  {#if onCollapse}
    <header class="sidebar-header">
      <button
        class="collapse-btn"
        on:click={onCollapse}
        title="Collapse sidebar (click ☰ to bring back)"
        aria-label="Collapse sidebar"
      >‹</button>
    </header>
  {/if}
  <SidebarNav />
  <header class="projects-header">
    <span class="title">Projects</span>
    <button class="add" on:click={addProject} disabled={busy} title="Add project">+</button>
  </header>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
  {#snippet sessionItem(s: Session, showBranchTag: boolean)}
    <li
      class:attached={$activeTerminalSessionId === s.tmux_session_id}
      on:click|stopPropagation={() => attachSession(s.tmux_session_id)}
      on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); attachSession(s.tmux_session_id); } }}
      tabindex="0"
      role="button"
      title="Attach to {s.name}"
    >
      <span class="dot {s.state}"></span>
      <span class="sess-name">{s.name}</span>
      {#if showBranchTag && s.worktree}
        <code class="sess-branch">{truncateBranch(s.worktree, 14)}</code>
      {/if}
    </li>
  {/snippet}

  <ul>
    {#each $projects as p (p.id)}
      {@const projSessions = sessionsFor(p.id)}
      <li
        class:active={$selectedProjectId === p.id}
        class:drag-over={dragOverId === p.id}
        class:dragging={dragId === p.id}
        draggable="true"
        on:dragstart={(e) => onDragStart(e, p.id)}
        on:dragover={(e) => onDragOver(e, p.id)}
        on:dragleave={() => onDragLeave(p.id)}
        on:drop={(e) => onDrop(e, p.id)}
      >
        <div
          class="project-row"
          role="button"
          tabindex="0"
          on:click={() => select(p.id)}
          on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); select(p.id); } }}
          on:contextmenu={(e) => openMenu(e, p.id)}
        >
          <button
            type="button"
            class="expand-chevron"
            class:open={!!expanded[p.id]}
            on:click={(e) => toggleExpand(p.id, e)}
            aria-label={expanded[p.id] ? 'Collapse' : 'Expand'}
            tabindex="-1"
          >›</button>
          <span class="badge" style="--h: {hueForId(p.id)}">{initials(p.name)}</span>
          <span class="text">
            <span class="name">{p.name}</span>
            <span class="meta">
              {#if p.worktrees.length > 0}
                {p.worktrees.length} worktree{p.worktrees.length === 1 ? '' : 's'}
              {:else}
                <span class="muted">{p.path}</span>
              {/if}
              {#if projSessions.length > 0}
                <span class="sep">·</span>
                <span class="session-count">{projSessions.length} session{projSessions.length === 1 ? '' : 's'}</span>
              {/if}
            </span>
          </span>
        </div>
        {#if expanded[p.id]}
          {#if p.worktrees.length > 1}
            <ul class="worktrees">
              {#each p.worktrees as wt (wt.path)}
                {@const wtSessions = sessionsByWorktree[p.id]?.[wt.path] ?? []}
                <li class:primary={wt.is_primary}>
                  <div class="worktree-row">
                    <span class="branch-mark"></span>
                    <code title={wt.branch}>{truncateBranch(wt.branch)}</code>
                    {#if wt.is_primary}<span class="tag">main</span>{/if}
                    {#if wtSessions.length > 0}
                      <span class="wt-count" title="{wtSessions.length} session{wtSessions.length === 1 ? '' : 's'}">{wtSessions.length}</span>
                    {/if}
                  </div>
                  {#if wtSessions.length > 0}
                    <ul class="sessions nested">
                      {#each wtSessions as s (s.tmux_session_id)}
                        {@render sessionItem(s, false)}
                      {/each}
                    </ul>
                  {/if}
                </li>
              {/each}
            </ul>
          {:else if projSessions.length > 0}
            <ul class="sessions">
              {#each projSessions as s (s.tmux_session_id)}
                {@render sessionItem(s, true)}
              {/each}
            </ul>
          {/if}
        {/if}
      </li>
    {:else}
      <li class="empty">No projects yet.<br>Click <kbd>+</kbd> to add one.</li>
    {/each}
  </ul>

  {#if menuOpenForId}
    <div class="ctxmenu" style="left:{menuX}px; top:{menuY}px;" on:click|stopPropagation role="menu">
      <button on:click={() => removeProject(menuOpenForId!)}>Remove project</button>
    </div>
  {/if}
</aside>

<style>
  aside {
    width: 100%;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    overflow-y: auto;
    height: 100vh;
    position: relative;
    display: flex;
    flex-direction: column;
  }
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
  .add,
  .collapse-btn {
    width: 22px;
    height: 22px;
    padding: 0;
    font-size: 14px;
    line-height: 1;
    border-radius: var(--r-sm);
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid var(--border);
  }
  .add:hover:not(:disabled),
  .collapse-btn:hover { color: var(--accent); border-color: var(--accent); }
  .collapse-btn { font-size: 13px; }
  .err {
    padding: 8px 12px;
    background: var(--state-error-bg);
    color: var(--state-error);
    font-size: 11px;
    margin: 8px 10px;
    border-radius: var(--r-sm);
  }
  ul { list-style: none; margin: 0; padding: 6px 0; }
  li {
    padding: 0;
  }
  .project-row {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 7px 12px;
    background: transparent;
    color: var(--fg);
    text-align: left;
    border-left: 2px solid transparent;
    cursor: pointer;
    transition: background var(--t-fast);
  }
  li.active > .project-row {
    background: var(--accent-bg);
    border-left-color: var(--accent);
  }
  .project-row:hover { background: var(--bg-elev-2); }
  li.dragging > .project-row { opacity: 0.4; }
  li.drag-over > .project-row { box-shadow: inset 0 2px 0 var(--accent); }
  .project-row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .expand-chevron {
    width: 14px;
    height: 14px;
    padding: 0;
    margin: 0;
    background: transparent;
    border: none;
    color: var(--fg-mute);
    font-size: 12px;
    line-height: 1;
    transition: transform var(--t-fast), color var(--t-fast);
    flex-shrink: 0;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .expand-chevron:hover { color: var(--fg); }
  .expand-chevron.open { transform: rotate(90deg); color: var(--fg-dim); }

  .meta .sep { color: var(--fg-mute); margin: 0 4px; }
  .meta .session-count { color: var(--state-running); font-weight: 500; }

  .badge {
    width: 24px;
    height: 24px;
    border-radius: var(--r-sm);
    background: hsl(var(--h), 60%, 22%);
    color: hsl(var(--h), 80%, 80%);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    flex-shrink: 0;
  }
  :global([data-theme="light"]) .badge {
    background: hsl(var(--h), 70%, 88%);
    color: hsl(var(--h), 60%, 30%);
  }
  .text {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 500;
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 10.5px;
    color: var(--fg-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta .muted { font-family: var(--mono); color: var(--fg-mute); }

  .worktrees {
    list-style: none;
    margin: 0 0 6px;
    padding: 2px 12px 4px 46px;
  }
  .worktrees > li {
    padding: 0;
    font-size: 11px;
    color: var(--fg-dim);
  }
  .worktrees > li.primary { color: var(--fg); }
  .worktree-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 0;
  }
  .branch-mark {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--fg-mute);
    flex-shrink: 0;
  }
  .worktrees > li.primary .branch-mark { background: var(--accent); }
  .worktrees code {
    font-family: var(--mono);
    font-size: 11px;
  }
  .wt-count {
    margin-left: auto;
    font-size: 9.5px;
    font-weight: 600;
    color: var(--state-running);
    background: var(--accent-bg);
    padding: 0 5px;
    border-radius: 8px;
    min-width: 14px;
    text-align: center;
  }
  .tag {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    background: var(--accent-bg);
    color: var(--accent);
    padding: 1px 5px;
    border-radius: 3px;
    margin-left: 2px;
  }

  /* Sessions list under each project (flat — single-worktree projects). */
  .sessions {
    list-style: none;
    margin: 0 0 6px;
    padding: 0 12px 4px 46px;
  }
  /* Sessions list nested inside a worktree group (multi-worktree projects).
   * Indented one extra step past the worktree row with a subtle left-guide
   * so the parent → child relationship reads at a glance. */
  .sessions.nested {
    margin: 1px 0 4px 1px;
    padding: 0 0 2px 16px;
    border-left: 1px solid var(--border);
  }
  .sessions li {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 8px;
    border-radius: var(--r-sm);
    font-size: 11px;
    color: var(--fg-dim);
    cursor: pointer;
    transition: background var(--t-fast), color var(--t-fast);
  }
  .sessions li:hover { background: var(--bg-elev-2); color: var(--fg); }
  .sessions li.attached {
    background: var(--accent-bg);
    color: var(--accent);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .sessions li.attached:hover { background: var(--accent-bg-strong); }
  .sessions .sess-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
  }
  .sessions .sess-branch {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-mute);
    background: var(--bg-elev-2);
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .sessions li.attached .sess-branch {
    background: rgba(0, 0, 0, 0.18);
    color: var(--accent);
  }

  .empty {
    padding: 30px 18px;
    text-align: center;
    color: var(--fg-dim);
    font-size: 12px;
    line-height: 1.7;
  }
  .empty kbd {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    font-family: var(--mono);
    font-size: 11px;
  }
  .ctxmenu {
    position: fixed;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-md);
    z-index: 1000;
    min-width: 160px;
    padding: 4px;
  }
  .ctxmenu button {
    display: block;
    width: 100%;
    padding: 6px 10px;
    background: transparent;
    border: none;
    color: var(--fg);
    text-align: left;
    font-size: 12px;
    border-radius: var(--r-sm);
  }
  .ctxmenu button:hover { background: var(--accent-bg); color: var(--accent); }
</style>
