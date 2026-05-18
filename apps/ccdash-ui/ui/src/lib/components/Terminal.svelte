<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { Terminal as XTerm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { listen } from '@tauri-apps/api/event';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { terminal } from '$lib/tauri';
  import { detectedUrlsBySession } from '$lib/stores';
  import { extractLocalUrls } from '$lib/urlDetect';
  import '@xterm/xterm/css/xterm.css';

  /** Tmux session id this terminal is wrapping (e.g. "$0"). URLs detected
   *  from this pane's output are recorded against this id so the BrowserView
   *  can scope its suggestions per session. */
  export let sessionId: string;

  /** Visibility flag. When this terminal's slot is hidden (Browser tab
   *  open, or another terminal active), the container can have 0 layout
   *  size depending on positioning. Re-fit + resize when it flips back to
   *  visible so tmux gets the correct rows/cols. */
  export let visible: boolean = true;

  const decoder = new TextDecoder();

  export let command: string[];

  let containerEl: HTMLDivElement;
  let xterm: XTerm | undefined;
  let fit: FitAddon | undefined;
  let ptyId: string | null = null;
  let unlistenOutput: UnlistenFn | null = null;
  let unlistenEof: UnlistenFn | null = null;
  let ready = false;

  /** Re-fit the xterm to the current container size and push the new
   *  rows/cols to tmux. Safe to call repeatedly; bails if not mounted yet
   *  or the container has zero dimensions (avoids tmux thinking the pane
   *  is 0x0 during a layout transition). */
  async function refit() {
    if (!ready || !xterm || !fit || !containerEl) return;
    if (containerEl.offsetWidth === 0 || containerEl.offsetHeight === 0) return;
    try {
      fit.fit();
      if (ptyId) {
        await terminal.resize(ptyId, xterm.rows, xterm.cols);
      }
    } catch (e) {
      console.warn('Terminal refit failed:', e);
    }
  }

  // Whenever the parent toggles visible→true, re-fit on the next tick so
  // the container has its final layout dimensions before measurement.
  $: if (visible && ready) {
    tick().then(refit);
  }

  onMount(async () => {
    if (!containerEl) return;
    xterm = new XTerm({
      convertEol: true,
      fontFamily: 'ui-monospace, "SF Mono", Monaco, monospace',
      fontSize: 13,
      theme: { background: '#1a1b1e', foreground: '#e6e6e6' },
    });
    fit = new FitAddon();
    xterm.loadAddon(fit);
    xterm.open(containerEl);
    fit.fit();

    const { rows, cols } = xterm;
    ptyId = await terminal.open(command, rows, cols);
    ready = true;

    unlistenOutput = await listen<number[]>(`terminal-output::${ptyId}`, (e) => {
      const bytes = new Uint8Array(e.payload);
      xterm!.write(bytes);
      // Scan for loopback URLs and surface them to the Browser tab,
      // scoped to this session's id.
      const text = decoder.decode(bytes, { stream: true });
      const urls = extractLocalUrls(text);
      if (urls.length > 0) {
        detectedUrlsBySession.update((m) => {
          const next = new Map(m);
          const set = new Set(next.get(sessionId) ?? []);
          for (const u of urls) set.add(u);
          next.set(sessionId, set);
          return next;
        });
      }
    });
    unlistenEof = await listen(`terminal-eof::${ptyId}`, () => {
      xterm!.write('\r\n\x1b[31m[process exited]\x1b[0m\r\n');
    });

    xterm.onData((data) => {
      if (ptyId) terminal.write(ptyId, new TextEncoder().encode(data));
    });

    xterm.onResize(({ rows, cols }) => {
      if (ptyId) terminal.resize(ptyId, rows, cols);
    });

    // Track parent size changes (splitter drag, sidebar collapse, etc.)
    // and re-fit. ResizeObserver fires for both width and height changes,
    // including the visibility→visible transition once layout settles.
    const ro = new ResizeObserver(() => refit());
    ro.observe(containerEl);

    const onWinResize = () => refit();
    window.addEventListener('resize', onWinResize);

    return () => {
      window.removeEventListener('resize', onWinResize);
      ro.disconnect();
    };
  });

  onDestroy(async () => {
    unlistenOutput?.();
    unlistenEof?.();
    if (ptyId) await terminal.close(ptyId).catch(() => {});
    xterm?.dispose();
  });
</script>

<div bind:this={containerEl} class="terminal"></div>

<style>
  .terminal {
    width: 100%;
    height: 100%;
    background: #1a1b1e;
    padding: 4px;
  }
</style>
