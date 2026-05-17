<script lang="ts">
  import { projects, sessions, terminalPane } from '$lib/stores';
  import { sessionsApi, tauri } from '$lib/tauri';
  import EmptyState from './EmptyState.svelte';

  let busy: Record<string, boolean> = {};
  let errMsg: string | null = null;

  function attach(sessionId: string) {
    terminalPane.set({
      command: ['tmux', 'attach-session', '-t', sessionId],
      mode: 'live',
    });
  }

  async function kill(sessionId: string, name: string) {
    if (!confirm(`Kill session "${name}" (${sessionId})? This terminates the tmux session.`)) return;
    busy = { ...busy, [sessionId]: true };
    errMsg = null;
    try {
      await sessionsApi.kill(sessionId);
      const { sessions: ss } = await tauri.sessionList();
      sessions.set(ss);
    } catch (e) {
      errMsg = String(e);
    } finally {
      const next = { ...busy };
      delete next[sessionId];
      busy = next;
    }
  }
</script>

<div>
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
  {#if $projects.length === 0}
    <EmptyState title="No sessions yet" />
  {:else}
  <table>
    <thead>
      <tr><th>tmux id</th><th>name</th><th>pid</th><th>cwd</th><th>state</th><th></th></tr>
    </thead>
    <tbody>
      {#each $sessions as s (s.tmux_session_id)}
        <tr>
          <td><code>{s.tmux_session_id}</code></td>
          <td>{s.name}</td>
          <td>{s.pid}</td>
          <td><code>{s.cwd}</code></td>
          <td class={s.state === 'running' ? 'running' : 'exited'}>{s.state}</td>
          <td class="actions">
            <button on:click={() => attach(s.tmux_session_id)}>Attach</button>
            <button class="danger" on:click={() => kill(s.tmux_session_id, s.name)} disabled={busy[s.tmux_session_id]}>
              {busy[s.tmux_session_id] ? '…' : 'Kill'}
            </button>
          </td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty">(no sessions — click "Launch session" up top)</td></tr>
      {/each}
    </tbody>
  </table>
  {/if}
</div>

<style>
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  th { color: var(--fg-dim); font-weight: 500; font-size: 12px; text-transform: uppercase; letter-spacing: 1px; }
  .running { color: var(--success); }
  .exited { color: var(--fg-dim); }
  .empty { text-align: center; color: var(--fg-dim); font-style: italic; padding: 24px; }
  .actions { display: flex; gap: 6px; }
  .danger {
    background: transparent;
    border: 1px solid var(--danger);
    color: var(--danger);
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 12px;
  }
  .danger:hover:not([disabled]) { background: var(--danger); color: var(--bg); }
  .err {
    padding: 8px 12px;
    background: rgba(255, 0, 0, 0.1);
    color: var(--danger);
    font-size: 12px;
  }
</style>
