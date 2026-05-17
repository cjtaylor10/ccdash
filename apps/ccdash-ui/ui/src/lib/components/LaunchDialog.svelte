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
  let forceToken: string | null = null;

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
      errMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if open}
  <div class="backdrop" on:click={close} role="presentation">
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
