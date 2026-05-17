<script lang="ts">
  import '$lib/theme.css';
  import { onMount } from 'svelte';
  import { tauri } from '$lib/tauri';
  import {
    activeTab,
    connectError,
    connected,
    plans,
    ports,
    projects,
    selectedProjectId,
    sessions,
  } from '$lib/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionsView from '$lib/components/SessionsView.svelte';
  import PortsView from '$lib/components/PortsView.svelte';
  import PlansView from '$lib/components/PlansView.svelte';

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

  onMount(async () => {
    try {
      await tauri.connect();
      connected.set(true);
      await refreshTopLevel();
    } catch (e) {
      connectError.set(String(e));
    }
    const handle = setInterval(refreshTopLevel, 5000);
    return () => clearInterval(handle);
  });

  function setTab(t: 'sessions' | 'ports' | 'plans') {
    activeTab.set(t);
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
  </main>
</div>

<style>
  .root { display: flex; height: 100vh; }
  main { flex: 1; display: flex; flex-direction: column; }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .tabs { display: flex; gap: 4px; }
  .tabs button { border-radius: 4px; }
  .tabs button.active { background: var(--accent-bg); color: var(--accent); border-color: var(--accent); }
  .status .error { color: var(--danger); font-size: 12px; }
  .content { flex: 1; overflow-y: auto; }
</style>
