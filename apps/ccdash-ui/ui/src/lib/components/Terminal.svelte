<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Terminal as XTerm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { listen } from '@tauri-apps/api/event';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { terminal } from '$lib/tauri';
  import '@xterm/xterm/css/xterm.css';

  export let command: string[];

  let containerEl: HTMLDivElement;
  let xterm: XTerm | undefined;
  let fit: FitAddon | undefined;
  let ptyId: string | null = null;
  let unlistenOutput: UnlistenFn | null = null;
  let unlistenEof: UnlistenFn | null = null;

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

    unlistenOutput = await listen<number[]>(`terminal-output::${ptyId}`, (e) => {
      const bytes = new Uint8Array(e.payload);
      xterm!.write(bytes);
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

    const onResize = () => fit?.fit();
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
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
