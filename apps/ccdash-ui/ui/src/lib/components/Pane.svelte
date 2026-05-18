<script lang="ts">
  import { panes, removePane, setPaneType, type PaneType } from '$lib/stores';
  import BrowserView from './BrowserView.svelte';
  import PlansView from './PlansView.svelte';
  import SessionsView from './SessionsView.svelte';
  import PortsView from './PortsView.svelte';

  /** Pane id this component is rendering. Used to look up per-pane state. */
  export let paneId: string;
  /** Current pane type (or null = empty). */
  export let type: PaneType | null;

  $: canClose = $panes.length > 1;

  function onTypeChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value as PaneType;
    setPaneType(paneId, v);
  }

  function onClose() {
    if (!canClose) return;
    removePane(paneId);
  }
</script>

<div class="pane">
  <header class="pane-header">
    <select
      class="type-picker"
      value={type ?? ''}
      on:change={onTypeChange}
      aria-label="Pane content type"
    >
      {#if type === null}
        <option value="" disabled>Pick content…</option>
      {/if}
      <option value="browser">Browser</option>
      <option value="plans">Plans</option>
      <option value="sessions">Sessions</option>
      <option value="ports">Ports</option>
    </select>
    <button
      class="close"
      on:click={onClose}
      disabled={!canClose}
      title={canClose ? 'Close pane' : 'Cannot close the last pane'}
      aria-label="Close pane"
    >✕</button>
  </header>
  <div class="pane-body">
    {#if type === 'browser'}
      <BrowserView {paneId} />
    {:else if type === 'plans'}
      <PlansView />
    {:else if type === 'sessions'}
      <SessionsView />
    {:else if type === 'ports'}
      <PortsView />
    {:else}
      <div class="empty">
        <p>Pick a content type from the dropdown above.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .pane {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg);
  }
  .pane-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .type-picker {
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 2px 6px;
    font-size: 11.5px;
  }
  .type-picker:hover { border-color: var(--border-strong); }
  .close {
    margin-left: auto;
    width: 22px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    color: var(--fg-mute);
    border-radius: var(--r-sm);
    font-size: 12px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .close:hover:not(:disabled) {
    color: var(--state-error);
    border-color: var(--border);
    background: var(--bg-elev-2);
  }
  .close:disabled { opacity: 0.3; cursor: not-allowed; }
  .pane-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-mute);
    font-size: 12px;
    padding: 24px;
  }
</style>
