<script lang="ts">
  import { onMount } from 'svelte';
  import { writable } from 'svelte/store';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { daemonApi, tauri, windows as windowsApi } from '$lib/tauri';
  import {
    activeTab,
    connectError,
    connected,
    detectedUrls,
    mirrorTarget,
    nextRetryAt,
    plans,
    ports,
    projects,
    reconnecting,
    selectedProjectId,
    sessions,
    terminalPane,
  } from '$lib/stores';
  import {
    startPublishing,
    stopPublishing,
    startMirroring,
    stopMirroring,
  } from '$lib/windowSync';
  import {
    startReconnectLoop,
    retryNow,
    stopReconnectLoop,
  } from '$lib/reconnect';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionsView from '$lib/components/SessionsView.svelte';
  import PortsView from '$lib/components/PortsView.svelte';
  import PlansView from '$lib/components/PlansView.svelte';
  import Terminal from '$lib/components/Terminal.svelte';
  import LaunchDialog from '$lib/components/LaunchDialog.svelte';
  import WelcomeModal from '$lib/components/WelcomeModal.svelte';
  import BrowserView from '$lib/components/BrowserView.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { installKeybinds } from '$lib/keybinds';
  import { theme, watchSystem, type Theme } from '$lib/theme';

  const otherWindowList = writable<string[]>([]);

  let launchOpen = false;
  let welcomeOpen = false;
  let paletteOpen = false;

  async function log(msg: string) {
    try {
      await invoke('log_from_frontend', { level: 'info', message: msg });
    } catch {}
  }

  async function refreshTopLevel() {
    try {
      const { projects: ps } = await tauri.projectList();
      projects.set(ps);
      if ($selectedProjectId === null && ps.length > 0) {
        selectedProjectId.set(ps[0].id);
      }
      const { sessions: ss } = await tauri.sessionList();
      sessions.set(ss);
      const ports_ = await tauri.portsList();
      ports.set(ports_);
      // Feed detected URLs: every running TCP listener on a loopback-like
      // host becomes http://localhost:PORT. Terminal output adds more.
      if (ports_.running.length > 0) {
        detectedUrls.update((s) => {
          const next = new Set(s);
          for (const p of ports_.running) {
            next.add(`http://localhost:${p.port}`);
          }
          return next;
        });
      }
    } catch (e) {
      connectError.set(String(e));
    }
  }

  async function refreshPlansFor(pid: string | null) {
    if (!pid) {
      plans.set([]);
      return;
    }
    try {
      const { plans: ps } = await tauri.plansGet(pid);
      plans.set(ps);
    } catch (e) {
      console.warn('plans.get failed', e);
      plans.set([]);
    }
  }

  $: refreshPlansFor($selectedProjectId);

  async function refreshOtherWindows() {
    try {
      const all = await windowsApi.list();
      otherWindowList.set(all.filter((l) => l !== windowsApi.currentLabel()));
    } catch (e) {
      console.warn('list_windows failed', e);
    }
  }

  onMount(async () => {
    await log('App.onMount fired');
    try {
      await log('calling tauri.connect()');
      await tauri.connect();
      await log('tauri.connect() returned');
      connected.set(true);
      await refreshTopLevel();
      await log('refreshTopLevel done');
      // First-run check: only on the main window (label "main"); other
      // windows skip the welcome flow to avoid duplicate prompts.
      if (windowsApi.currentLabel() === 'main') {
        try {
          const { pending } = await daemonApi.firstRunStatus();
          if (pending) welcomeOpen = true;
        } catch (e) {
          await log(`first_run_status failed: ${String(e)}`);
        }
      }
    } catch (e) {
      await log(`connect/refresh failed: ${String(e)}`);
      connectError.set(String(e));
      // Kick off auto-reconnect with exponential backoff.
      startReconnectLoop(refreshTopLevel);
    }

    const unlistenDaemon = await listen<{ method: string; params: any }>(
      'daemon-event',
      (e) => {
        const m = e.payload.method;
        if (m.startsWith('project.') || m.startsWith('projects.')) {
          tauri.projectList().then(({ projects: ps }) => projects.set(ps)).catch(() => {});
        } else if (m.startsWith('session.') || m.startsWith('sessions.')) {
          tauri.sessionList().then(({ sessions: ss }) => sessions.set(ss)).catch(() => {});
        }
      },
    );

    startPublishing();
    await refreshOtherWindows();
    const windowsTimer = window.setInterval(refreshOtherWindows, 5000);

    const uninstallKeybinds = installKeybinds({
      openCommandPalette: () => (paletteOpen = true),
      openLaunchDialog: () => (launchOpen = true),
    });
    watchSystem();

    return () => {
      unlistenDaemon();
      stopPublishing();
      stopMirroring();
      stopReconnectLoop();
      clearInterval(windowsTimer);
      uninstallKeybinds();
    };
  });

  function setTab(t: 'sessions' | 'ports' | 'plans' | 'browser') {
    activeTab.set(t);
  }

  function closeTerminal() {
    terminalPane.set(null);
  }

  function onMirrorChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value;
    if (v) startMirroring(v);
    else stopMirroring();
  }

  function onThemeChange(e: Event) {
    theme.set((e.target as HTMLSelectElement).value as Theme);
  }

  $: healthColor =
    $reconnecting ? 'yellow' : $connected ? 'green' : 'red';
  $: healthTitle =
    $reconnecting ? 'Reconnecting…' : $connected ? 'Daemon connected' : 'Daemon disconnected';
</script>

<div class="root">
  <Sidebar />
  <main>
    {#if $reconnecting}
      <div class="reconnect-banner">
        <span class="dot" />
        <span class="msg">
          Disconnected from daemon — retrying
          {#if $nextRetryAt}
            in {Math.max(0, Math.ceil(($nextRetryAt - Date.now()) / 1000))}s
          {/if}
        </span>
        {#if $connectError}
          <span class="err">{$connectError}</span>
        {/if}
        <button class="retry-btn" on:click={retryNow}>Retry now</button>
      </div>
    {/if}
    <header>
      <div class="tabs">
        <button class:active={$activeTab === 'sessions'} on:click={() => setTab('sessions')}>Sessions</button>
        <button class:active={$activeTab === 'ports'} on:click={() => setTab('ports')}>Ports</button>
        <button class:active={$activeTab === 'plans'} on:click={() => setTab('plans')}>Plans</button>
        <button class:active={$activeTab === 'browser'} on:click={() => setTab('browser')}>
          Browser
          {#if $detectedUrls.size > 0 && $activeTab !== 'browser'}
            <span class="badge" aria-label="{$detectedUrls.size} URLs detected"></span>
          {/if}
        </button>
      </div>
      <div class="actions">
        <button class="primary" on:click={() => (launchOpen = true)}>Launch session</button>
        <select value={$mirrorTarget ?? ''} on:change={onMirrorChange}>
          <option value="">— independent —</option>
          {#each $otherWindowList as w (w)}
            <option value={w}>follow {w}</option>
          {/each}
        </select>
        <button on:click={() => windowsApi.openNew()}>+ New window</button>
        <select class="theme-select" value={$theme} on:change={onThemeChange} title="Theme">
          <option value="system">Auto</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
        <span class="health health-{healthColor}" title={healthTitle} aria-label={healthTitle}></span>
      </div>
    </header>
    <section class="content">
      {#if $activeTab === 'sessions'}
        <SessionsView />
      {:else if $activeTab === 'ports'}
        <PortsView />
      {:else if $activeTab === 'plans'}
        <PlansView />
      {:else}
        <BrowserView />
      {/if}
    </section>
    {#if $terminalPane}
      <section class="terminal-panel">
        <div class="terminal-header">
          <span>Terminal: {$terminalPane.command.join(' ')}</span>
          <button on:click={closeTerminal}>Close</button>
        </div>
        <div class="terminal-host">
          {#key $terminalPane.command.join(' ')}
            <Terminal command={$terminalPane.command} />
          {/key}
        </div>
      </section>
    {/if}
  </main>
</div>

<LaunchDialog bind:open={launchOpen} />
<WelcomeModal bind:open={welcomeOpen} />
<CommandPalette
  bind:open={paletteOpen}
  on:openLaunchDialog={() => (launchOpen = true)}
/>

<style>
  .root { display: flex; height: 100vh; }
  main { flex: 1; display: flex; flex-direction: column; }
  .reconnect-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 16px;
    background: rgba(255, 165, 0, 0.12);
    border-bottom: 1px solid rgba(255, 165, 0, 0.35);
    color: #f4a83c;
    font-size: 12px;
  }
  .reconnect-banner .dot {
    width: 8px; height: 8px;
    background: #f4a83c;
    border-radius: 50%;
    animation: pulse 1s ease-in-out infinite;
  }
  .reconnect-banner .err {
    color: var(--danger);
    font-family: var(--mono);
    margin-left: 6px;
  }
  .reconnect-banner .retry-btn {
    margin-left: auto;
    background: #f4a83c;
    color: var(--bg);
    border: none;
    border-radius: 4px;
    padding: 4px 12px;
    font-size: 12px;
    cursor: pointer;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .tabs { display: flex; gap: 4px; }
  .tabs button { border-radius: 4px; position: relative; }
  .tabs button.active { background: var(--accent-bg); color: var(--accent); border-color: var(--accent); }
  .tabs .badge {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    margin-left: 4px;
    vertical-align: middle;
  }
  .actions { display: flex; gap: 8px; margin-left: auto; align-items: center; }
  .actions .primary {
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 600;
  }
  .actions .primary:hover { opacity: 0.9; }
  .actions .theme-select {
    padding: 3px 6px;
    font-size: 11px;
  }
  .health {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    margin-left: 4px;
  }
  .health-green { background: var(--success); box-shadow: 0 0 4px var(--success); }
  .health-yellow { background: #f4a83c; box-shadow: 0 0 4px #f4a83c; animation: pulse 1s ease-in-out infinite; }
  .health-red { background: var(--danger); box-shadow: 0 0 4px var(--danger); }
  .actions select {
    background: var(--bg-elev);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 12px;
  }
  .status .error { color: var(--danger); font-size: 12px; }
  .content { flex: 1; overflow-y: auto; min-height: 200px; }
  .terminal-panel {
    height: 340px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: #1a1b1e;
  }
  .terminal-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 6px 12px; background: var(--bg-elev); border-bottom: 1px solid var(--border);
    font-family: var(--mono); font-size: 12px; color: var(--fg-dim);
  }
  .terminal-host { flex: 1; overflow: hidden; }
</style>
