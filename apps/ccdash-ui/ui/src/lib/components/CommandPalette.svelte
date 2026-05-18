<script lang="ts">
  import { createEventDispatcher, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { projects, selectedProjectId } from '$lib/stores';
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
    width: 560px;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    overflow: hidden;
    box-shadow: var(--shadow-lg);
    animation: popIn 140ms ease-out;
  }
  @keyframes popIn { from { transform: scale(0.97); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  input {
    border: none;
    background: transparent;
    color: var(--fg);
    padding: 13px 18px;
    font-size: 13.5px;
    outline: none;
    border-bottom: 1px solid var(--border);
    font-family: inherit;
  }
  input::placeholder { color: var(--fg-mute); }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 12px;
    cursor: pointer;
    border-radius: var(--r-sm);
  }
  li.active { background: var(--accent-bg-strong); }
  li:hover:not(.empty):not(.active) { background: var(--bg-elev-2); }
  li .label { color: var(--fg); font-size: 12.5px; }
  li.active .label { color: var(--accent); }
  li .hint {
    color: var(--fg-mute);
    font-size: 10.5px;
    font-family: var(--mono);
    margin-left: auto;
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  li.empty { color: var(--fg-mute); justify-content: center; cursor: default; font-style: italic; padding: 22px; font-size: 12px; }
</style>
