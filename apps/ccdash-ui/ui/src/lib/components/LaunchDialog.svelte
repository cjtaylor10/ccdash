<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { projects, selectedProjectId, sessions } from '$lib/stores';
  import { asPortConflict, isUiRpcError, sessionsApi, tauri, type PortConflict } from '$lib/tauri';

  export let open = false;

  const dispatch = createEventDispatcher<{ close: void }>();

  const SKIP_PERMS_KEY = 'ccdash.launchDefaults.skipPerms';

  function readSkipPerms(): boolean {
    try { return localStorage.getItem(SKIP_PERMS_KEY) === '1'; } catch { return false; }
  }

  let projectId: string | null = null;
  let worktree: string | null = null;
  let command = '';
  let skipPerms = readSkipPerms();
  let busy = false;
  let errMsg: string | null = null;
  let conflicts: PortConflict[] = [];
  let forceToken: string | null = null;

  $: if (open && projectId === null) {
    projectId = $selectedProjectId ?? $projects[0]?.id ?? null;
    const proj = $projects.find((p) => p.id === projectId);
    worktree = proj?.worktrees.find((w) => w.is_primary)?.branch
      ?? proj?.worktrees[0]?.branch
      ?? null;
    command = '';
    skipPerms = readSkipPerms();
    errMsg = null;
    conflicts = [];
    forceToken = null;
  }

  /** Resolve the actual command string to send to the daemon.
   *  - Empty command field + skipPerms checked → "claude --dangerously-skip-permissions"
   *  - Non-empty command starting with "claude" + skipPerms checked → append flag
   *  - Any other custom command + skipPerms checked → leave alone (user's override wins)
   */
  function resolveCommand(): string | undefined {
    const raw = command.trim();
    if (!raw) {
      return skipPerms ? 'claude --dangerously-skip-permissions' : undefined;
    }
    if (skipPerms && /^claude(\s|$)/.test(raw) && !/--dangerously-skip-permissions/.test(raw)) {
      return `${raw} --dangerously-skip-permissions`;
    }
    return raw;
  }

  function onSkipPermsChange() {
    try { localStorage.setItem(SKIP_PERMS_KEY, skipPerms ? '1' : '0'); } catch {}
  }

  $: currentProject = $projects.find((p) => p.id === projectId);

  function close() {
    open = false;
    projectId = null;
    worktree = null;
    command = '';
    errMsg = null;
    conflicts = [];
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
    conflicts = [];
    errMsg = null;
  }

  async function submit(useForce = false) {
    if (!projectId) return;
    busy = true;
    errMsg = null;
    try {
      await sessionsApi.launch({
        projectId,
        worktree: worktree ?? undefined,
        command: resolveCommand(),
        forceToken: useForce && forceToken ? forceToken : undefined,
      });
      const { sessions: ss } = await tauri.sessionList();
      sessions.set(ss);
      close();
    } catch (e) {
      const pc = asPortConflict(e);
      if (pc) {
        conflicts = pc.conflicts;
        forceToken = pc.force_token;
        errMsg = `Port conflict: ${pc.conflicts.map((c) => c.port).join(', ')}`;
      } else if (isUiRpcError(e)) {
        errMsg = e.message;
      } else {
        errMsg = String(e);
      }
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

        <label class="checkbox-row">
          <input
            type="checkbox"
            bind:checked={skipPerms}
            on:change={onSkipPermsChange}
            disabled={busy}
          />
          <span class="checkbox-text">
            <span class="checkbox-title">Skip permission prompts</span>
            <small class="warn">
              Adds <code>--dangerously-skip-permissions</code> — Claude won't ask
              before running tools (file edits, shell, network). Use with caution.
            </small>
          </span>
        </label>

        {#if conflicts.length > 0}
          <div class="conflict">
            <strong>Port conflict</strong>
            <ul>
              {#each conflicts as c (c.port)}
                <li><code>:{c.port}</code> held by <code>{c.holder}</code></li>
              {/each}
            </ul>
            <p>
              Kill the conflicting process (or use the Kill button on its
              session row), or click <em>Launch anyway</em> to ignore.
            </p>
          </div>
        {:else if errMsg}
          <div class="err">{errMsg}</div>
        {/if}
      </div>
      <footer>
        <button on:click={close} disabled={busy}>Cancel</button>
        {#if conflicts.length > 0 && forceToken}
          <button class="warn" on:click={() => submit(true)} disabled={busy}>
            {busy ? 'Launching…' : 'Launch anyway'}
          </button>
        {:else}
          <button class="primary" on:click={() => submit(false)} disabled={busy || !projectId}>
            {busy ? 'Launching…' : 'Launch'}
          </button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fadeIn 120ms ease-out;
  }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  .modal {
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    min-width: 480px;
    max-width: 560px;
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    animation: popIn 160ms ease-out;
  }
  @keyframes popIn { from { transform: scale(0.96); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 13px 18px;
    border-bottom: 1px solid var(--border);
  }
  header h3 { margin: 0; font-size: 14px; font-weight: 600; }
  .x {
    background: transparent;
    border: none;
    color: var(--fg-mute);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
    padding: 0 6px;
    border-radius: var(--r-sm);
  }
  .x:hover { color: var(--fg); background: var(--bg-elev-2); }
  .body {
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  label > span {
    font-size: 10px;
    color: var(--fg-mute);
    text-transform: uppercase;
    letter-spacing: 0.8px;
    font-weight: 600;
  }
  select, input[type="text"] {
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 6px 9px;
    font-size: 12.5px;
    font-family: var(--sans);
  }
  small { color: var(--fg-mute); font-size: 10.5px; }
  small code { font-family: var(--mono); background: var(--bg-elev-2); padding: 1px 4px; border-radius: 3px; }

  .checkbox-row {
    flex-direction: row;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
  }
  .checkbox-row input[type="checkbox"] {
    margin-top: 2px;
    accent-color: var(--state-warn);
    flex-shrink: 0;
  }
  .checkbox-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .checkbox-title {
    font-size: 12px;
    color: var(--fg);
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
  }
  .checkbox-row small.warn {
    color: var(--state-warn);
  }
  .checkbox-row small.warn code {
    background: rgba(0, 0, 0, 0.25);
    color: var(--state-warn);
  }
  .err {
    padding: 9px 12px;
    background: var(--state-error-bg);
    color: var(--state-error);
    border-radius: var(--r-sm);
    font-size: 11.5px;
  }
  .conflict {
    padding: 12px 14px;
    background: var(--state-warn-bg);
    border: 1px solid color-mix(in srgb, var(--state-warn) 35%, transparent);
    border-radius: var(--r-md);
    color: var(--state-warn);
    font-size: 11.5px;
  }
  .conflict strong { display: block; margin-bottom: 6px; font-size: 12px; }
  .conflict ul { margin: 0 0 8px 16px; padding: 0; }
  .conflict li { margin: 2px 0; }
  .conflict p { margin: 0; color: var(--fg-dim); }
  .conflict code {
    font-family: var(--mono);
    background: rgba(0, 0, 0, 0.25);
    padding: 1px 5px;
    border-radius: 3px;
    color: var(--state-warn);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    background: var(--bg);
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  footer button {
    padding: 5px 14px;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-size: 12.5px;
    font-weight: 500;
  }
  footer button:hover:not(:disabled) { background: var(--bg-elev-2); border-color: var(--border-strong); }
  footer .primary {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
    font-weight: 600;
  }
  footer .primary:hover:not(:disabled) { filter: brightness(1.08); background: var(--accent); border-color: var(--accent); }
  footer .warn {
    background: var(--state-warn);
    color: var(--bg);
    border-color: var(--state-warn);
    font-weight: 600;
  }
  footer .warn:hover:not(:disabled) { filter: brightness(1.08); background: var(--state-warn); }
  footer .primary:disabled, footer .warn:disabled { opacity: 0.5; }
</style>
