<script lang="ts">
  import { sessions, terminalPane } from '$lib/stores';

  function attach(sessionId: string) {
    terminalPane.set({
      command: ['tmux', 'attach-session', '-t', sessionId],
      mode: 'live',
    });
  }
</script>

<div>
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
          <td><button on:click={() => attach(s.tmux_session_id)}>Attach</button></td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty">(no sessions)</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  th { color: var(--fg-dim); font-weight: 500; font-size: 12px; text-transform: uppercase; letter-spacing: 1px; }
  .running { color: var(--success); }
  .exited { color: var(--fg-dim); }
  .empty { text-align: center; color: var(--fg-dim); font-style: italic; padding: 24px; }
</style>
