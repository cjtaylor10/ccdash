<script lang="ts">
  import { panes, paneLayoutDirection, paneSizeById } from '$lib/stores';
  import Pane from './Pane.svelte';
  import Splitter from './Splitter.svelte';

  /** Update the stored size for a pane after the user drags its splitter.
   *  We store explicit pixel sizes so panes don't reflow when a sibling
   *  resizes — only the dragged pane and its left/upper neighbour change. */
  function setSize(id: string, v: number) {
    paneSizeById.update((m) => {
      const next = new Map(m);
      next.set(id, v);
      return next;
    });
  }

  function sizeFor(id: string): number | null {
    return $paneSizeById.get(id) ?? null;
  }
</script>

<div class="container" class:row={$paneLayoutDirection === 'row'} class:column={$paneLayoutDirection === 'column'}>
  {#each $panes as pane, i (pane.id)}
    {#if i > 0}
      {@const prev = $panes[i - 1]}
      {@const initial = sizeFor(prev.id) ?? 320}
      <Splitter
        orientation={$paneLayoutDirection === 'row' ? 'horizontal' : 'vertical'}
        value={initial}
        min={160}
        max={2000}
        on:change={(e) => setSize(prev.id, (e as CustomEvent<number>).detail)}
      />
    {/if}
    {@const explicit = sizeFor(pane.id)}
    <div
      class="pane-slot"
      style={explicit !== null && i < $panes.length - 1
        ? `flex: 0 0 ${explicit}px;`
        : 'flex: 1 1 0;'}
    >
      <Pane paneId={pane.id} type={pane.type} />
    </div>
  {/each}
</div>

<style>
  .container {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .container.row { flex-direction: row; }
  .container.column { flex-direction: column; }
  .pane-slot {
    display: flex;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
</style>
