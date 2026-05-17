<script lang="ts">
  import { onMount } from 'svelte';
  import { writable } from 'svelte/store';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { tauri, windows as windowsApi } from '$lib/tauri';
  import {
    activeTab,
    connectError,
    connected,
    mirrorTarget,
    plans,
    ports,
    projects,
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
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionsView from '$lib/components/SessionsView.svelte';
  import PortsView from '$lib/components/PortsView.svelte';
  import PlansView from '$lib/components/PlansView.svelte';
  import Terminal from '$lib/components/Terminal.svelte';

  const otherWindowList = writable<string[]>([]);

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
    } catch (e) {
      await log(`connect/refresh failed: ${String(e)}`);
      connectError.set(String(e));
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

    return () => {
      unlistenDaemon();
      stopPublishing();
      stopMirroring();
      clearInterval(windowsTimer);
    };
  });

  function setTab(t: 'sessions' | 'ports' | 'plans') {
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
</script>

<div class="root">
  <Sidebar />
  <main>
    <header>
      <div class="tabs">
        <button class:active={$activeTab === 'sessions'} on:click={() => setTab('sessions')}>Sessions</button>
        <button class:active={$activeTab === 'ports'} on:click={() => setTab('ports')}>Ports</button>
        <button class:active={$activeTab === 'plans'} on:click={() => setTab('plans')}>Plans</button>
      </div>
      <div class="actions">
        <select value={$mirrorTarget ?? ''} on:change={onMirrorChange}>
          <option value="">— independent —</option>
          {#each $otherWindowList as w (w)}
            <option value={w}>follow {w}</option>
          {/each}
        </select>
        <button on:click={() => windowsApi.openNew()}>+ New window</button>
      </div>
      <div class="status">
        {#if !$connected}
          <span class="error">{$connectError ?? 'connecting...'}</span>
        {/if}
      </div>
    </header>
    <section class="content">
      {#if $activeTab === 'sessions'}
        <SessionsView />
      {:else if $activeTab === 'ports'}
        <PortsView />
      {:else}
        <PlansView />
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

<style>
  .root { display: flex; height: 100vh; }
  main { flex: 1; display: flex; flex-direction: column; }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .tabs { display: flex; gap: 4px; }
  .tabs button { border-radius: 4px; }
  .tabs button.active { background: var(--accent-bg); color: var(--accent); border-color: var(--accent); }
  .actions { display: flex; gap: 8px; margin-left: auto; align-items: center; }
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
