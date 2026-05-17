<script lang="ts">
  import { ports, projects, selectedProjectId } from '$lib/stores';
  import EmptyState from './EmptyState.svelte';

  $: filteredRunning = $selectedProjectId
    ? $ports.running.filter((p) => p.project_id === $selectedProjectId)
    : $ports.running;
  $: filteredDeclared = $selectedProjectId
    ? $ports.declared.filter((p) => p.project_id === $selectedProjectId)
    : $ports.declared;
</script>

{#if $projects.length === 0}
  <EmptyState title="No ports to show" />
{:else}
<div>
  <h3>Running listeners</h3>
  <table>
    <thead><tr><th>port</th><th>pid</th><th>command</th><th>project</th></tr></thead>
    <tbody>
      {#each filteredRunning as p (`${p.port}-${p.pid}`)}
        <tr>
          <td><code>{p.port}</code></td>
          <td>{p.pid ?? '?'}</td>
          <td>{p.command ?? '?'}</td>
          <td><code>{p.project_id ?? '-'}</code></td>
        </tr>
      {:else}
        <tr><td colspan="4" class="empty">(none)</td></tr>
      {/each}
    </tbody>
  </table>

  <h3>Declared</h3>
  <table>
    <thead><tr><th>port</th><th>project</th><th>source</th></tr></thead>
    <tbody>
      {#each filteredDeclared as p (`${p.project_id}-${p.port}-${p.source}`)}
        <tr>
          <td><code>{p.port}</code></td>
          <td><code>{p.project_id}</code></td>
          <td>{p.source}</td>
        </tr>
      {:else}
        <tr><td colspan="3" class="empty">(none)</td></tr>
      {/each}
    </tbody>
  </table>
</div>
{/if}

<style>
  h3 { margin: 16px 12px 8px; font-size: 13px; text-transform: uppercase; letter-spacing: 1px; color: var(--fg-dim); }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); }
  th { color: var(--fg-dim); font-weight: 500; font-size: 12px; }
  .empty { text-align: center; color: var(--fg-dim); font-style: italic; padding: 16px; }
</style>
