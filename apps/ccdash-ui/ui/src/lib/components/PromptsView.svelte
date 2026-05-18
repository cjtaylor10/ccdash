<script lang="ts">
  import { prompts, addPrompt, updatePrompt, deletePrompt } from '$lib/stores';
  import type { Prompt } from '$lib/stores';
  import { tick } from 'svelte';
  import { showToast } from '$lib/toast';

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

  let titleBuf = '';
  let bodyBuf = '';
  let lastSyncedId: string | null = null;

  $: if (selected && selected.id !== lastSyncedId) {
    titleBuf = selected.title;
    bodyBuf = selected.body;
    lastSyncedId = selected.id;
  } else if (!selected && lastSyncedId !== null) {
    titleBuf = '';
    bodyBuf = '';
    lastSyncedId = null;
  }

  $: isDirty =
    !!selected && (titleBuf !== selected.title || bodyBuf !== selected.body);

  let titleInput: HTMLInputElement | null = null;

  async function focusTitle() {
    await tick();
    titleInput?.focus();
  }

  function saveSelected() {
    if (!selected || !isDirty) return;
    updatePrompt(selected.id, { title: titleBuf, body: bodyBuf });
  }

  function revertSelected() {
    if (!selected) return;
    titleBuf = selected.title;
    bodyBuf = selected.body;
  }

  function deleteSelected() {
    if (!selected) return;
    if (!confirm('Delete prompt?')) return;
    const id = selected.id;
    deletePrompt(id);
    if (selectedId === id) selectedId = null;
  }

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      saveSelected();
    } else if (e.key === 'Escape') {
      revertSelected();
    }
  }

  async function copyToClipboard(body: string) {
    try {
      await navigator.clipboard.writeText(body);
      showToast('Prompt copied to clipboard');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      showToast(`Copy failed: ${msg}`, 'err');
    }
  }

  function copySelected() {
    if (!selected) return;
    void copyToClipboard(selected.body);
  }

  function copyRow(ev: MouseEvent, body: string) {
    ev.stopPropagation();
    void copyToClipboard(body);
  }

  function selectPrompt(p: Prompt) {
    selectedId = p.id;
  }

  function newPrompt() {
    const id = addPrompt();
    selectedId = id;
    focusTitle();
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
            <button
              class="row-copy"
              on:click={(ev) => copyRow(ev, p.body)}
              title="Copy body to clipboard"
              aria-label="Copy {p.title || 'Untitled'} to clipboard"
            >⎘</button>
          </li>
        {/each}
      </ul>
    {/if}
  </aside>
  <section class="editor-pane">
    {#if selected}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="editor" on:keydown={onKey} role="form">
        <label class="field">
          <span class="field-label">Title</span>
          <input
            type="text"
            bind:value={titleBuf}
            bind:this={titleInput}
            placeholder="Prompt title"
          />
        </label>
        <label class="field body-field">
          <span class="field-label">Body</span>
          <textarea
            bind:value={bodyBuf}
            placeholder="Prompt body — copied to your clipboard when you hit Copy."
            spellcheck="false"
          ></textarea>
        </label>
        <div class="actions">
          <button class="copy" on:click={copySelected} title="Copy body to clipboard">Copy</button>
          <button class="save" on:click={saveSelected} disabled={!isDirty}>Save{isDirty ? ' *' : ''}</button>
          <button class="delete" on:click={deleteSelected}>Delete</button>
        </div>
      </div>
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
  .editor {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 18px;
    min-height: 0;
  }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .field-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1.2px;
    color: var(--fg-dim);
    font-weight: 600;
  }
  .field input,
  .field textarea {
    width: 100%;
    background: var(--bg-elev);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 7px 10px;
    font-size: 13px;
    font-family: var(--sans);
  }
  .field input:focus,
  .field textarea:focus { outline: none; border-color: var(--accent); }
  .body-field { flex: 1; min-height: 0; }
  .body-field textarea {
    flex: 1;
    min-height: 0;
    height: 100%;
    resize: none;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.55;
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  .actions button {
    padding: 5px 14px;
    font-size: 12px;
    font-weight: 600;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--fg);
    cursor: pointer;
  }
  .actions button:disabled { opacity: 0.5; cursor: not-allowed; }
  .actions button:hover:not(:disabled) { background: var(--bg-elev-2); border-color: var(--border-strong); }
  .actions .copy:not(:disabled) {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }
  .actions .delete {
    color: var(--state-error);
    border-color: color-mix(in srgb, var(--state-error) 40%, var(--border));
  }
  .actions .delete:hover { background: var(--state-error-bg); }
  .row-copy {
    width: 22px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    color: var(--fg-mute);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .list li:hover .row-copy { border-color: var(--border); color: var(--fg-dim); }
  .row-copy:hover { background: var(--bg-elev-2); color: var(--fg) !important; border-color: var(--border-strong) !important; }
  .list li.active .row-copy { color: var(--accent); border-color: transparent; }
</style>
