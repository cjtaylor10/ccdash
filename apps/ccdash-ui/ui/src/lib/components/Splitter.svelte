<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  /** Direction of the bar:
   *  - 'vertical' = a horizontal bar that resizes the height above/below
   *    (drag up/down to change the bottom panel's height).
   *  - 'horizontal' = a vertical bar that resizes the width to its left
   *    (drag left/right to change the sidebar's width).
   */
  export let orientation: 'vertical' | 'horizontal' = 'vertical';

  /** Current value (height in px for vertical, width in px for horizontal). */
  export let value: number;
  /** Min allowable value. */
  export let min: number = 100;
  /** Max allowable value — defaults to "lots". */
  export let max: number = 10000;

  /** When true, the splitter inverts the delta — used for bottom-anchored
   *  panes where dragging UP increases the value. */
  export let invert: boolean = false;

  const dispatch = createEventDispatcher<{ change: number }>();

  let dragging = false;
  let startPos = 0;
  let startValue = 0;

  function clamp(n: number) { return Math.max(min, Math.min(max, n)); }

  function onMouseDown(e: MouseEvent) {
    dragging = true;
    startPos = orientation === 'vertical' ? e.clientY : e.clientX;
    startValue = value;
    document.body.style.cursor = orientation === 'vertical' ? 'row-resize' : 'col-resize';
    document.body.style.userSelect = 'none';
    e.preventDefault();
  }

  function onMouseMove(e: MouseEvent) {
    if (!dragging) return;
    const pos = orientation === 'vertical' ? e.clientY : e.clientX;
    const delta = pos - startPos;
    const next = clamp(startValue + (invert ? -delta : delta));
    if (next !== value) {
      value = next;
      dispatch('change', next);
    }
  }

  function onMouseUp() {
    if (!dragging) return;
    dragging = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }

  function onDblClick() {
    // Reset to min on double-click for quick collapse-to-min.
    value = min;
    dispatch('change', min);
  }
</script>

<svelte:window on:mousemove={onMouseMove} on:mouseup={onMouseUp} />

<div
  class="splitter {orientation}"
  class:dragging
  on:mousedown={onMouseDown}
  on:dblclick={onDblClick}
  role="separator"
  aria-orientation={orientation}
  title="Drag to resize"
></div>

<style>
  .splitter {
    flex-shrink: 0;
    background: transparent;
    transition: background 100ms ease-out;
    z-index: 5;
  }
  .splitter.vertical {
    height: 4px;
    width: 100%;
    cursor: row-resize;
    margin: -2px 0;
  }
  .splitter.horizontal {
    width: 4px;
    height: 100%;
    cursor: col-resize;
    margin: 0 -2px;
  }
  .splitter:hover,
  .splitter.dragging {
    background: var(--accent);
  }
</style>
