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
  /** Max allowable value — re-read on every mousemove so a reactive parent
   *  (e.g. one tracking window height) can widen the range during drag. */
  export let max: number = 10000;

  /** When true, the splitter inverts the delta — used for bottom-anchored
   *  panes where dragging UP increases the value. */
  export let invert: boolean = false;

  const dispatch = createEventDispatcher<{ change: number }>();

  let bar: HTMLDivElement;
  let dragging = false;
  let startPos = 0;
  let startValue = 0;
  let pendingValue: number | null = null;
  let rafHandle: number | null = null;

  function clamp(n: number) {
    return Math.max(min, Math.min(max, n));
  }

  /** Coalesce mousemove deltas through requestAnimationFrame — caps updates
   *  at ~60Hz and lets the browser batch layout work, eliminating jitter
   *  during fast drags. */
  function scheduleApply() {
    if (rafHandle !== null || pendingValue === null) return;
    rafHandle = requestAnimationFrame(() => {
      rafHandle = null;
      if (pendingValue !== null && pendingValue !== value) {
        value = pendingValue;
        dispatch('change', pendingValue);
      }
      pendingValue = null;
    });
  }

  function onPointerDown(e: PointerEvent) {
    dragging = true;
    startPos = orientation === 'vertical' ? e.clientY : e.clientX;
    startValue = value;
    document.body.style.cursor = orientation === 'vertical' ? 'row-resize' : 'col-resize';
    document.body.style.userSelect = 'none';
    // Capture the pointer so we keep getting events even if the cursor
    // leaves the (thin) bar element during a fast drag.
    try { bar.setPointerCapture(e.pointerId); } catch {}
    e.preventDefault();
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const pos = orientation === 'vertical' ? e.clientY : e.clientX;
    const delta = pos - startPos;
    pendingValue = clamp(startValue + (invert ? -delta : delta));
    scheduleApply();
  }

  function onPointerUp(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    try { bar.releasePointerCapture(e.pointerId); } catch {}
    // Flush any pending RAF before letting it drop.
    if (rafHandle !== null) {
      cancelAnimationFrame(rafHandle);
      rafHandle = null;
    }
    if (pendingValue !== null && pendingValue !== value) {
      value = pendingValue;
      dispatch('change', pendingValue);
    }
    pendingValue = null;
  }

  function onDblClick() {
    value = min;
    dispatch('change', min);
  }
</script>

<div
  bind:this={bar}
  class="splitter {orientation}"
  class:dragging
  on:pointerdown={onPointerDown}
  on:pointermove={onPointerMove}
  on:pointerup={onPointerUp}
  on:pointercancel={onPointerUp}
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
    position: relative;
    touch-action: none;
  }
  /* Visual: thin line. Hit area: wider via ::before pseudo-element so the
     user can grab a 10px-wide region around the 4px visual bar — much
     forgiving on quick clicks. */
  .splitter.vertical {
    height: 4px;
    width: 100%;
    cursor: row-resize;
    margin: -2px 0;
  }
  .splitter.vertical::before {
    content: '';
    position: absolute;
    inset: -4px 0;
  }
  .splitter.horizontal {
    width: 4px;
    height: 100%;
    cursor: col-resize;
    margin: 0 -2px;
  }
  .splitter.horizontal::before {
    content: '';
    position: absolute;
    inset: 0 -4px;
  }
  .splitter:hover,
  .splitter.dragging {
    background: var(--accent);
  }
</style>
