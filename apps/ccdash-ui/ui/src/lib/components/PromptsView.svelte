<script lang="ts">
  import { prompts, addPrompt } from '$lib/stores';
  import type { Prompt } from '$lib/stores';

  let selectedId: string | null = null;
  let query = '';

  $: sorted = [...$prompts].sort((a, b) => b.updatedAt - a.updatedAt);
  $: filtered = (() => {
    const q = query.trim().toLowerCase();
    if (!q) return sorted;
    return sorted.filter(
      (p) =>
        p.title.toLowerCase().includes(q) || p.body.toLowerCase().includes(q),
    );
  })();
  $: selected = $prompts.find((p) => p.id === selectedId) ?? null;

  function selectPrompt(p: Prompt) {
    selectedId = p.id;
  }

  function newPrompt() {
    const id = addPrompt();
    selectedId = id;
  }
</script>

<div class="prompts-root">
  <aside class="list-pane">
    <div class="list-toolbar">
      <input
        type="search"
        class="search"
        placeholder="Search prompts…"
        bind:value={query}
      />
      <button class="new-btn" on:click={newPrompt} title="New prompt">+ New</button>
    </div>
    {#if filtered.length === 0}
      <div class="empty">
        {#if $prompts.length === 0}
          No prompts yet — click <kbd>+ New</kbd> to add one.
        {:else}
          No prompts match "{query}".
        {/if}
      </div>
    {:else}
      <ul class="list">
        {#each filtered as p (p.id)}
          <li
            class:active={p.id === selectedId}
            role="button"
            tabindex="0"
            on:click={() => selectPrompt(p)}
            on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectPrompt(p); } }}
          >
            <span class="row-title">{p.title || 'Untitled'}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </aside>
  <section class="editor-pane">
    {#if selected}
      <div class="placeholder">Editor coming in next task — selected: {selected.title || 'Untitled'}</div>
    {:else}
      <div class="placeholder">Select a prompt or click <kbd>+ New</kbd>.</div>
    {/if}
  </section>
</div>

<style>
  .prompts-root {
    display: flex;
    flex: 1;
    min-height: 0;
    background: var(--bg);
  }
  .list-pane {
    width: 280px;
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    background: var(--bg-elev);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .list-toolbar {
    display: flex;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .search {
    flex: 1;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 4px 8px;
    font-size: 12px;
  }
  .search:focus { outline: none; border-color: var(--accent); }
  .new-btn {
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    border-radius: var(--r-sm);
    padding: 4px 10px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
  }
  .new-btn:hover { filter: brightness(1.08); }
  .list { list-style: none; margin: 0; padding: 4px 0; overflow-y: auto; }
  .list li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    font-size: 12.5px;
    color: var(--fg);
    cursor: pointer;
    border-left: 2px solid transparent;
    transition: background var(--t-fast);
  }
  .list li:hover { background: var(--bg-elev-2); }
  .list li.active {
    background: var(--accent-bg);
    color: var(--accent);
    border-left-color: var(--accent);
  }
  .row-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 24px 16px;
    color: var(--fg-dim);
    font-size: 12px;
    text-align: center;
    line-height: 1.6;
  }
  .empty kbd {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    font-family: var(--mono);
    font-size: 11px;
  }
  .editor-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 13px;
  }
  .placeholder kbd {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    font-family: var(--mono);
    font-size: 11px;
    margin: 0 2px;
  }
</style>
