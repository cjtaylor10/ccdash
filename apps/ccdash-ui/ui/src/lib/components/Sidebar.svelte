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
      // Refresh from daemon to reset visual state on failure.
      try {
        const { projects: ps } = await tauri.projectList();
        projects.set(ps);
      } catch {}
    } finally {
      dragId = null;
      dragOverId = null;
    }
  }

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
          <span class="name">{p.name}</span>
          <span class="path">{p.path}</span>
          {#if p.worktrees.length > 1}
            <ul class="worktrees">
              {#each p.worktrees as wt (wt.path)}
                <li><code title={wt.branch}>{truncateBranch(wt.branch)}</code>{wt.is_primary ? ' (main)' : ''}</li>
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
  li.dragging { opacity: 0.4; }
  li.drag-over { box-shadow: inset 0 2px 0 var(--accent); }
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
