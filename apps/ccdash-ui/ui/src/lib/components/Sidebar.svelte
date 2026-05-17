<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { projects, selectedProjectId } from '$lib/stores';
  import { projectsApi, tauri } from '$lib/tauri';
  import { truncateBranch } from '$lib/format';

  let busy = false;
  let errMsg: string | null = null;

  let menuOpenForId: string | null = null;
  let menuX = 0;
  let menuY = 0;

  let dragId: string | null = null;
  let dragOverId: string | null = null;

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
  <header>
    <span class="title">Projects</span>
    <button class="add" on:click={addProject} disabled={busy} title="Add project">+</button>
  </header>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
  <ul>
    {#each $projects as p (p.id)}
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
        <button on:click={() => select(p.id)} on:contextmenu={(e) => openMenu(e, p.id)}>
          <span class="badge" style="--h: {hueForId(p.id)}">{initials(p.name)}</span>
          <span class="text">
            <span class="name">{p.name}</span>
            <span class="meta">
              {#if p.worktrees.length > 0}
                {p.worktrees.length} worktree{p.worktrees.length === 1 ? '' : 's'}
              {:else}
                <span class="muted">{p.path}</span>
              {/if}
            </span>
          </span>
        </button>
        {#if $selectedProjectId === p.id && p.worktrees.length > 1}
          <ul class="worktrees">
            {#each p.worktrees as wt (wt.path)}
              <li class:primary={wt.is_primary}>
                <span class="branch-mark"></span>
                <code title={wt.branch}>{truncateBranch(wt.branch)}</code>
                {#if wt.is_primary}<span class="tag">main</span>{/if}
              </li>
            {/each}
          </ul>
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
    width: 232px;
    min-width: 232px;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    overflow-y: auto;
    height: 100vh;
    position: relative;
    display: flex;
    flex-direction: column;
  }
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
  .add {
    width: 22px;
    height: 22px;
    padding: 0;
    font-size: 14px;
    line-height: 1;
    border-radius: var(--r-sm);
    color: var(--fg-dim);
  }
  .add:hover:not(:disabled) { color: var(--accent); border-color: var(--accent); }
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
  li > button {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 12px;
    background: transparent;
    border: none;
    border-radius: 0;
    color: var(--fg);
    text-align: left;
    border-left: 2px solid transparent;
  }
  li.active > button {
    background: var(--accent-bg);
    border-left-color: var(--accent);
  }
  li > button:hover { background: var(--bg-elev-2); }
  li.dragging > button { opacity: 0.4; }
  li.drag-over > button { box-shadow: inset 0 2px 0 var(--accent); }
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
  .worktrees li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 0;
    font-size: 11px;
    color: var(--fg-dim);
  }
  .worktrees li.primary { color: var(--fg); }
  .branch-mark {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--fg-mute);
    flex-shrink: 0;
  }
  .worktrees li.primary .branch-mark { background: var(--accent); }
  .worktrees code {
    font-family: var(--mono);
    font-size: 11px;
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
