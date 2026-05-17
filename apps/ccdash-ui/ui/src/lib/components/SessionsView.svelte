<script lang="ts">
  import { projects, sessions, selectedProjectId, terminalPane } from '$lib/stores';
  import { sessionsApi, tauri } from '$lib/tauri';
  import EmptyState from './EmptyState.svelte';

  let busy: Record<string, boolean> = {};
  let errMsg: string | null = null;
  let query = '';

  /** Sessions filtered to the currently-selected project, OR all if no
   *  selection. Applied BEFORE the search filter. */
  $: scoped = $selectedProjectId
    ? $sessions.filter((s) => s.project_id === $selectedProjectId)
    : $sessions;

  $: showSearch = scoped.length > 10;
  $: filtered = query.trim()
    ? scoped.filter((s) => {
        const q = query.toLowerCase();
        return (
          s.name.toLowerCase().includes(q)
          || s.cwd.toLowerCase().includes(q)
          || s.tmux_session_id.toLowerCase().includes(q)
        );
      })
    : scoped;

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

  function shortPath(p: string): string {
    const home = '/Users/';
    if (p.startsWith(home)) {
      const rest = p.slice(home.length);
      const slash = rest.indexOf('/');
      if (slash > 0) return '~' + rest.slice(slash);
    }
    return p;
  }
</script>

{#if $projects.length === 0}
  <EmptyState title="No sessions yet" />
{:else}
  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}
  {#if showSearch}
    <div class="search">
      <input type="text" placeholder="Filter…" bind:value={query} />
      <span class="count-info">{filtered.length} / {scoped.length}</span>
    </div>
  {/if}

  {#if filtered.length === 0}
    <div class="empty-pad">
      {#if query.trim()}
        No sessions match <code>{query}</code>.
      {:else}
        No sessions{$selectedProjectId ? ' for the selected project' : ''}. Press <kbd>⌘L</kbd> to launch one.
      {/if}
    </div>
  {:else}
    <table>
      <thead>
        <tr>
          <th class="state-col"></th>
          <th>Name</th>
          <th>Project · worktree</th>
          <th>cwd</th>
          <th class="num">pid</th>
          <th class="num">tmux</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each filtered as s (s.tmux_session_id)}
          <tr>
            <td class="state-col">
              <span class="dot {s.state}" title={s.state}></span>
            </td>
            <td class="name">{s.name}</td>
            <td class="meta">
              {#if s.project_id}
                {@const proj = $projects.find((p) => p.id === s.project_id)}
                {#if proj}
                  <span class="proj">{proj.name}</span>
                {/if}
              {/if}
              {#if s.worktree}
                <span class="sep">·</span>
                <code>{s.worktree}</code>
              {/if}
            </td>
            <td class="cwd"><code title={s.cwd}>{shortPath(s.cwd)}</code></td>
            <td class="num"><code>{s.pid}</code></td>
            <td class="num tmux-id"><code>{s.tmux_session_id}</code></td>
            <td class="actions">
              <button class="btn-action" on:click={() => attach(s.tmux_session_id)}>Attach</button>
              <button
                class="btn-action danger"
                on:click={() => kill(s.tmux_session_id, s.name)}
                disabled={busy[s.tmux_session_id]}
              >{busy[s.tmux_session_id] ? '…' : 'Kill'}</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/if}

<style>
  .err {
    margin: 8px 12px;
    padding: 7px 11px;
    background: var(--state-error-bg);
    color: var(--state-error);
    border-radius: var(--r-sm);
    font-size: 12px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .search input {
    flex: 1;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 4px 8px;
    font-size: 12px;
  }
  .count-info { color: var(--fg-mute); font-size: 11px; font-variant-numeric: tabular-nums; }
  .empty-pad {
    padding: 32px;
    text-align: center;
    color: var(--fg-dim);
    font-size: 12px;
  }
  .empty-pad kbd {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    font-family: var(--mono);
  }
  .empty-pad code { font-family: var(--mono); background: var(--bg-elev-2); padding: 1px 5px; border-radius: 3px; }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  thead { background: var(--bg-elev); position: sticky; top: 0; z-index: 1; }
  th, td {
    text-align: left;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }
  th {
    color: var(--fg-mute);
    font-weight: 500;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    padding-top: 8px;
    padding-bottom: 8px;
  }
  th.num, td.num { text-align: right; font-variant-numeric: tabular-nums; }
  tbody tr { transition: background var(--t-fast); }
  tbody tr:hover { background: var(--bg-elev); }
  .state-col { width: 22px; padding-right: 0; }
  .name { font-weight: 500; }
  .meta { color: var(--fg-dim); }
  .meta .proj { color: var(--fg); }
  .meta .sep { color: var(--fg-mute); margin: 0 4px; }
  .meta code { font-family: var(--mono); }
  .cwd { color: var(--fg-dim); max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cwd code { font-family: var(--mono); }
  .tmux-id { color: var(--fg-mute); }
  .tmux-id code { font-family: var(--mono); }
  .actions { display: flex; gap: 4px; justify-content: flex-end; }
  .btn-action {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: var(--r-sm);
    padding: 3px 9px;
    font-size: 11px;
  }
  .btn-action:hover:not(:disabled) { color: var(--fg); border-color: var(--border-strong); background: var(--bg-elev-2); }
  .btn-action.danger:hover:not(:disabled) {
    color: var(--state-error);
    border-color: var(--state-error);
    background: var(--state-error-bg);
  }
</style>
