<script lang="ts">
  import { ports, projects, selectedProjectId } from '$lib/stores';
  import EmptyState from './EmptyState.svelte';

  $: filteredRunning = $selectedProjectId
    ? $ports.running.filter((p) => p.project_id === $selectedProjectId)
    : $ports.running;
  $: filteredDeclared = $selectedProjectId
    ? $ports.declared.filter((p) => p.project_id === $selectedProjectId)
    : $ports.declared;

  function projectName(id: string | null | undefined): string {
    if (!id) return '—';
    return $projects.find((p) => p.id === id)?.name ?? id.slice(0, 8);
  }
</script>

{#if $projects.length === 0}
  <EmptyState title="No ports to show" />
{:else}
  <section>
    <h3><span class="dot running"></span>Running listeners <span class="count">{filteredRunning.length}</span></h3>
    {#if filteredRunning.length === 0}
      <div class="empty">No TCP listeners on this machine right now.</div>
    {:else}
      <table>
        <thead>
          <tr><th class="num">Port</th><th>Command</th><th class="num">PID</th><th>Project</th></tr>
        </thead>
        <tbody>
          {#each filteredRunning as p (`${p.port}-${p.pid}`)}
            <tr>
              <td class="num port"><code>:{p.port}</code></td>
              <td class="cmd">{p.command ?? '—'}</td>
              <td class="num"><code>{p.pid ?? '?'}</code></td>
              <td class="proj">{projectName(p.project_id)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <section>
    <h3><span class="dot warn"></span>Declared <span class="count">{filteredDeclared.length}</span></h3>
    {#if filteredDeclared.length === 0}
      <div class="empty">No declared ports (looked in <code>package.json</code>, <code>.env</code>, <code>docker-compose.yml</code>, <code>Procfile</code>).</div>
    {:else}
      <table>
        <thead>
          <tr><th class="num">Port</th><th>Project</th><th>Source</th></tr>
        </thead>
        <tbody>
          {#each filteredDeclared as p (`${p.project_id}-${p.port}-${p.source}`)}
            <tr>
              <td class="num port"><code>:{p.port}</code></td>
              <td class="proj">{projectName(p.project_id)}</td>
              <td class="source"><code>{p.source}</code></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/if}

<style>
  section {
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 14px 8px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--fg-dim);
    font-weight: 600;
  }
  h3 .count {
    margin-left: auto;
    font-size: 10px;
    color: var(--fg-mute);
    background: var(--bg-elev-2);
    padding: 1px 6px;
    border-radius: 8px;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  th, td {
    text-align: left;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
  }
  th {
    color: var(--fg-mute);
    font-weight: 500;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    background: var(--bg-elev);
  }
  th.num, td.num { text-align: right; font-variant-numeric: tabular-nums; }
  tbody tr:hover { background: var(--bg-elev); }
  .port code { color: var(--accent); font-family: var(--mono); font-weight: 600; }
  .cmd { font-family: var(--mono); color: var(--fg-dim); }
  .source code { font-family: var(--mono); color: var(--fg-dim); }
  .proj { color: var(--fg); }
  .empty {
    padding: 18px 14px;
    color: var(--fg-mute);
    font-style: italic;
    font-size: 12px;
  }
  .empty code { font-family: var(--mono); background: var(--bg-elev-2); padding: 1px 5px; border-radius: 3px; font-style: normal; }
</style>
