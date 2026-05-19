<script lang="ts">
  import {
    projects,
    sessions,
    selectedProjectId,
    attachedSessions,
    activeTerminalSessionId,
  } from '$lib/stores';
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

  /** When no project is selected, group the filtered list by project so
   *  the user sees a categorized view of every session at once. Each group
   *  is sorted by project order; the "no project" bucket goes last. */
  $: groups = (() => {
    if ($selectedProjectId) return null;
    const byProject = new Map<string | null, typeof $sessions>();
    for (const s of filtered) {
      const key = s.project_id ?? null;
      if (!byProject.has(key)) byProject.set(key, [] as typeof $sessions);
      byProject.get(key)!.push(s);
    }
    const ordered: Array<{ projectId: string | null; name: string; sessions: typeof $sessions }> = [];
    for (const p of $projects) {
      const list = byProject.get(p.id);
      if (list && list.length > 0) ordered.push({ projectId: p.id, name: p.name, sessions: list });
    }
    const orphans = byProject.get(null);
    if (orphans && orphans.length > 0) ordered.push({ projectId: null, name: '(no project)', sessions: orphans });
    return ordered;
  })();

  /** Add the session to the attached set if absent, and make it the
   *  active terminal. If it's already attached, just switch to it —
   *  instant, no pty respawn, no scrollback loss. */
  function attach(sessionId: string) {
    const already = $attachedSessions.some((s) => s.sessionId === sessionId);
    if (!already) {
      attachedSessions.update((arr) => [
        ...arr,
        {
          sessionId,
          command: ['tmux', 'attach-session', '-t', sessionId],
        },
      ]);
    }
    activeTerminalSessionId.set(sessionId);
  }

  function isAttached(sessionId: string): boolean {
    return $activeTerminalSessionId === sessionId;
  }

  async function kill(sessionId: string, name: string) {
    if (!confirm(`Kill session "${name}" (${sessionId})? This terminates the tmux session.`)) return;
    busy = { ...busy, [sessionId]: true };
    errMsg = null;
    try {
      await sessionsApi.kill(sessionId);
      // If this session was attached, close its terminal pane too — there's
      // nothing left for the pty to read from.
      attachedSessions.update((arr) => arr.filter((s) => s.sessionId !== sessionId));
      if ($activeTerminalSessionId === sessionId) {
        const remaining = $attachedSessions.filter((s) => s.sessionId !== sessionId);
        activeTerminalSessionId.set(remaining.length > 0 ? remaining[remaining.length - 1].sessionId : null);
      }
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

  {#snippet headerRow()}
    <tr>
      <th class="state-col"></th>
      <th>Name</th>
      <th>Project · worktree</th>
      <th>cwd</th>
      <th class="num">pid</th>
      <th class="num" title="Tmux session ID — stable across renames">Tmux ID</th>
      <th></th>
    </tr>
  {/snippet}

  {#snippet sessionRow(s)}
    {@const attached = isAttached(s.tmux_session_id)}
    <tr
      class:attached
      on:click={() => attach(s.tmux_session_id)}
      on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); attach(s.tmux_session_id); } }}
      tabindex="0"
      role="button"
      aria-label="Attach to session {s.name}"
      title="Click to attach"
    >
      <td class="state-col">
        <span class="dot {s.state}" title={s.state}></span>
      </td>
      <td class="name">
        {s.name}
        {#if attached}<span class="attached-tag">attached</span>{/if}
      </td>
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
      <td class="actions" on:click|stopPropagation>
        <button
          class="btn-action danger"
          on:click={() => kill(s.tmux_session_id, s.name)}
          disabled={busy[s.tmux_session_id]}
        >{busy[s.tmux_session_id] ? '…' : 'Kill'}</button>
      </td>
    </tr>
  {/snippet}

  {#if filtered.length === 0}
    <div class="empty-pad">
      {#if query.trim()}
        No sessions match <code>{query}</code>.
      {:else}
        No sessions{$selectedProjectId ? ' for the selected project' : ''}. Press <kbd>⌘L</kbd> to launch one.
      {/if}
    </div>
  {:else if groups}
    <!-- No project selected → grouped-by-project view. -->
    {#each groups as g (g.projectId ?? '__none__')}
      <div class="group">
        <div class="group-header">
          <span class="group-name">{g.name}</span>
          <span class="group-count">{g.sessions.length} session{g.sessions.length === 1 ? '' : 's'}</span>
        </div>
        <table>
          <thead>{@render headerRow()}</thead>
          <tbody>
            {#each g.sessions as s (s.tmux_session_id)}
              {@render sessionRow(s)}
            {/each}
          </tbody>
        </table>
      </div>
    {/each}
  {:else}
    <!-- Project selected → flat filtered list (original behavior). -->
    <table>
      <thead>{@render headerRow()}</thead>
      <tbody>
        {#each filtered as s (s.tmux_session_id)}
          {@render sessionRow(s)}
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
  .group { margin-bottom: 10px; }
  .group-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 12px 14px 6px;
    background: var(--bg);
  }
  .group-name {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg);
  }
  .group-count {
    font-size: 10.5px;
    color: var(--fg-mute);
    font-variant-numeric: tabular-nums;
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
  tbody tr {
    transition: background var(--t-fast);
    cursor: pointer;
  }
  tbody tr:hover { background: var(--bg-elev); }
  tbody tr:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  tbody tr.attached {
    background: var(--accent-bg);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  tbody tr.attached:hover { background: var(--accent-bg-strong); }
  .attached-tag {
    display: inline-block;
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--accent);
    color: var(--bg);
    font-size: 9.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    vertical-align: middle;
  }
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
