<script lang="ts">
  import { createEventDispatcher, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { activeTab, projects, selectedProjectId } from '$lib/stores';
  import { windows as windowsApi } from '$lib/tauri';
  import { addProjectViaPicker } from '$lib/projectActions';

  export let open = false;

  const dispatch = createEventDispatcher<{ openLaunchDialog: void }>();

  type Action = {
    id: string;
    label: string;
    hint?: string;
    run: () => void | Promise<void>;
  };

  let query = '';
  let cursor = 0;
  let inputEl: HTMLInputElement;

  $: items = buildActions(query);

  function buildActions(q: string): Action[] {
    const list: Action[] = [];
    for (const p of $projects) {
      list.push({
        id: `proj:${p.id}`,
        label: `Switch to project: ${p.name}`,
        hint: p.path,
        run: () => selectedProjectId.set(p.id),
      });
    }
    list.push(
      {
        id: 'launch',
        label: 'Launch session',
        run: () => dispatch('openLaunchDialog'),
      },
      {
        id: 'add',
        label: 'Add project…',
        run: async () => { await addProjectViaPicker(); },
      },
      { id: 'new-win', label: 'New window', run: async () => { try { await windowsApi.openNew(); } catch {} } },
      { id: 'close-win', label: 'Close window', run: async () => { try { await getCurrentWindow().close(); } catch {} } },
      { id: 'tab-sessions', label: 'Go to: Sessions', run: () => activeTab.set('sessions') },
      { id: 'tab-ports', label: 'Go to: Ports', run: () => activeTab.set('ports') },
      { id: 'tab-plans', label: 'Go to: Plans', run: () => activeTab.set('plans') },
      { id: 'tab-browser', label: 'Go to: Browser', run: () => activeTab.set('browser') },
    );
    if (!q.trim()) return list;
    const needle = q.toLowerCase();
    return list.filter((a) =>
      a.label.toLowerCase().includes(needle)
      || (a.hint?.toLowerCase().includes(needle) ?? false)
    );
  }

  $: if (cursor >= items.length) cursor = Math.max(0, items.length - 1);

  async function focusInput() {
    await tick();
    inputEl?.focus();
    inputEl?.select();
  }

  $: if (open) {
    cursor = 0;
    query = '';
    focusInput();
  }

  function close() {
    open = false;
  }

  async function runItem(it: Action) {
    close();
    try { await it.run(); } catch {}
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      cursor = Math.min(items.length - 1, cursor + 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      cursor = Math.max(0, cursor - 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const it = items[cursor];
      if (it) runItem(it);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }
</script>

{#if open}
  <div class="backdrop" on:click={close} role="presentation">
    <div class="palette" on:click|stopPropagation role="dialog" aria-modal="true">
      <input
        bind:this={inputEl}
        bind:value={query}
        on:keydown={onKeydown}
        placeholder="Type a command…"
      />
      <ul>
        {#each items as it, i (it.id)}
          <li class:active={i === cursor} on:mousedown|preventDefault={() => runItem(it)}>
            <span class="label">{it.label}</span>
            {#if it.hint}<code class="hint">{it.hint}</code>{/if}
          </li>
        {:else}
          <li class="empty">No matches</li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 14vh;
    z-index: 300;
  }
  .palette {
    width: 520px;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
  }
  input {
    border: none;
    background: var(--bg);
    color: var(--fg);
    padding: 12px 16px;
    font-size: 14px;
    outline: none;
    border-bottom: 1px solid var(--border);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 7px 16px;
    cursor: pointer;
  }
  li.active { background: var(--accent-bg); }
  li:hover:not(.empty) { background: var(--accent-bg); }
  li .label { color: var(--fg); font-size: 13px; }
  li .hint { color: var(--fg-dim); font-size: 11px; font-family: var(--mono); margin-left: auto; }
  li.empty { color: var(--fg-dim); justify-content: center; cursor: default; font-style: italic; padding: 16px; }
</style>
