<script lang="ts">
  import { projects, selectedProjectId } from '$lib/stores';

  function select(id: string) {
    selectedProjectId.set(id);
  }
</script>

<aside>
  <header>
    <h2>Projects</h2>
  </header>
  <ul>
    {#each $projects as p (p.id)}
      <li class:active={$selectedProjectId === p.id}>
        <button on:click={() => select(p.id)}>
          <span class="name">{p.name}</span>
          <span class="path">{p.path}</span>
          {#if p.worktrees.length > 1}
            <ul class="worktrees">
              {#each p.worktrees as wt (wt.path)}
                <li><code>{wt.branch}</code>{wt.is_primary ? ' (main)' : ''}</li>
              {/each}
            </ul>
          {/if}
        </button>
      </li>
    {:else}
      <li class="empty">(no projects — add one via the CLI)</li>
    {/each}
  </ul>
</aside>

<style>
  aside {
    width: 260px;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    overflow-y: auto;
    height: 100vh;
  }
  header { padding: 12px 16px; border-bottom: 1px solid var(--border); }
  header h2 { margin: 0; font-size: 14px; text-transform: uppercase; color: var(--fg-dim); letter-spacing: 1px; }
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
  .name { font-weight: 600; }
  .path { font-family: var(--mono); font-size: 11px; color: var(--fg-dim); }
  .worktrees { margin: 6px 0 0; padding-left: 12px; }
  .worktrees li { font-size: 12px; color: var(--fg-dim); }
  .empty { padding: 16px; color: var(--fg-dim); font-style: italic; }
</style>
