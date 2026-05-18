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
    mirrorTarget,
    nextRetryAt,
    plans,
    ports,
    projects,
    reconnecting,
    selectedProjectId,
    sessions,
    attachedSessions,
    activeTerminalSessionId,
    detectedUrlsBySession,
    terminalCollapsed,
    sidebarWidth,
    sidebarCollapsed,
    terminalPanelHeight,
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
  import Splitter from '$lib/components/Splitter.svelte';
  import { installKeybinds } from '$lib/keybinds';
  import { theme, watchSystem, type Theme } from '$lib/theme';

  const otherWindowList = writable<string[]>([]);

  let launchOpen = false;
  let welcomeOpen = false;
  let paletteOpen = false;

  /** If set, this window was opened via `open_terminal_window` to host a
   *  single popped-out tmux session. Renders the terminal full-screen
   *  with no sidebar/tabs. The underlying tmux session lives independently
   *  of this window (and the main one). */
  let poppedOutSession: string | null = null;
  try {
    const term = new URLSearchParams(window.location.search).get('term');
    if (term) poppedOutSession = term;
  } catch {}

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
      // Feed machine-wide detected URLs (null session key) — every running
      // TCP listener on a loopback host becomes http://localhost:PORT. The
      // Terminal component adds per-session URLs scoped to its session id.
      if (ports_.running.length > 0) {
        detectedUrlsBySession.update((m) => {
          const next = new Map(m);
          const global = new Set(next.get(null) ?? []);
          for (const p of ports_.running) {
            global.add(`http://localhost:${p.port}`);
          }
          next.set(null, global);
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
      // If launched as a popped-out terminal window, auto-attach to the
      // requested session so the user sees the terminal immediately.
      if (poppedOutSession) {
        attachedSessions.set([
          {
            sessionId: poppedOutSession,
            command: ['tmux', 'attach-session', '-t', poppedOutSession],
          },
        ]);
        activeTerminalSessionId.set(poppedOutSession);
        terminalCollapsed.set(false);
      }
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

  /** Close the currently-active terminal pane: removes it from the
   *  attachedSessions list (which unmounts its Terminal component +
   *  closes its pty) and switches to the next attached session if any,
   *  otherwise hides the panel entirely. */
  function closeTerminal() {
    const active = $activeTerminalSessionId;
    if (!active) return;
    const remaining = $attachedSessions.filter((s) => s.sessionId !== active);
    attachedSessions.set(remaining);
    activeTerminalSessionId.set(remaining.length > 0 ? remaining[remaining.length - 1].sessionId : null);
  }

  function switchTerminal(sessionId: string) {
    activeTerminalSessionId.set(sessionId);
  }

  /** Pop the active terminal out into its own ccdash window. The session
   *  in the main window stays attached as well — both windows share the
   *  underlying tmux session, so what one sees the other sees. Closing
   *  either window doesn't kill the tmux session (that's what Kill is for). */
  async function popOutActive() {
    const sid = $activeTerminalSessionId;
    if (!sid) return;
    const sess = $sessions.find((s) => s.tmux_session_id === sid);
    try {
      await windowsApi.openTerminal(sid, sess?.name ?? sid);
    } catch (e) {
      console.warn('pop-out failed:', e);
    }
  }

  function toggleCollapse() {
    terminalCollapsed.update((v) => !v);
  }

  function toggleSidebar() {
    sidebarCollapsed.update((v) => !v);
  }

  $: activeTerminalState = $attachedSessions.find((s) => s.sessionId === $activeTerminalSessionId) ?? null;

  /** Total URLs detected across all sessions — drives the Browser tab
   *  badge regardless of which session is currently active. */
  $: totalDetectedUrls = (() => {
    let n = 0;
    for (const urls of $detectedUrlsBySession.values()) n += urls.size;
    return n;
  })();

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

  $: sessionsCount = $sessions.filter((s) => s.state === 'running').length;
  $: portsCount = $ports.running.length;
  $: plansCount = $plans.length;
</script>

{#if poppedOutSession}
  <!-- Popped-out single-session window: render only the terminal full-screen.
       The session keeps running in tmux regardless of whether this window
       (or the main one) is closed. -->
  <div class="popout-root">
    {#if $attachedSessions.length > 0}
      {@const t = $attachedSessions[0]}
      {@const sess = $sessions.find((s) => s.tmux_session_id === t.sessionId)}
      <header class="popout-header">
        <span class="popout-title">
          <span class="dot {sess?.state ?? 'running'}"></span>
          <span>{sess?.name ?? t.sessionId}</span>
        </span>
        <span class="popout-hint">tmux session keeps running if you close this window</span>
      </header>
      <div class="popout-term">
        <Terminal sessionId={t.sessionId} command={t.command} visible={true} />
      </div>
    {:else}
      <div class="popout-loading">
        {#if !$connected}
          Connecting to daemon…
        {:else}
          Loading session {poppedOutSession}…
        {/if}
      </div>
    {/if}
  </div>
{:else}
<div class="root">
  {#if !$sidebarCollapsed}
    <div class="sidebar-wrap" style="width: {$sidebarWidth}px;">
      <Sidebar onCollapse={toggleSidebar} />
    </div>
    <Splitter
      orientation="horizontal"
      bind:value={$sidebarWidth}
      min={180}
      max={500}
    />
  {:else}
    <button
      class="sidebar-show"
      on:click={toggleSidebar}
      title="Show projects sidebar"
      aria-label="Show sidebar"
    >☰</button>
  {/if}
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
      <div class="tabs" role="tablist">
        <button class="pill" class:active={$activeTab === 'sessions'} on:click={() => setTab('sessions')} role="tab" aria-selected={$activeTab === 'sessions'}>
          Sessions
          {#if sessionsCount > 0}<span class="count">{sessionsCount}</span>{/if}
        </button>
        <button class="pill" class:active={$activeTab === 'ports'} on:click={() => setTab('ports')} role="tab" aria-selected={$activeTab === 'ports'}>
          Ports
          {#if portsCount > 0}<span class="count">{portsCount}</span>{/if}
        </button>
        <button class="pill" class:active={$activeTab === 'plans'} on:click={() => setTab('plans')} role="tab" aria-selected={$activeTab === 'plans'}>
          Plans
          {#if plansCount > 0}<span class="count">{plansCount}</span>{/if}
        </button>
        <button class="pill" class:active={$activeTab === 'browser'} on:click={() => setTab('browser')} role="tab" aria-selected={$activeTab === 'browser'}>
          Browser
          {#if totalDetectedUrls > 0}
            <span class="count" class:pulse={$activeTab !== 'browser'}>{totalDetectedUrls}</span>
          {/if}
        </button>
      </div>
      <div class="actions">
        <button class="primary" on:click={() => (launchOpen = true)} title="Launch session (⌘L)">
          <span class="plus">+</span> Launch
        </button>
        <button class="icon-btn" on:click={() => windowsApi.openNew()} title="New window (⌘N)">⊞</button>
        {#if $otherWindowList.length > 0}
          <select value={$mirrorTarget ?? ''} on:change={onMirrorChange} title="Mirror another window">
            <option value="">independent</option>
            {#each $otherWindowList as w (w)}
              <option value={w}>follow {w}</option>
            {/each}
          </select>
        {/if}
        <select class="theme-select" value={$theme} on:change={onThemeChange} title="Theme">
          <option value="system">auto</option>
          <option value="dark">dark</option>
          <option value="light">light</option>
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
    {#if $attachedSessions.length > 0}
      {#if !$terminalCollapsed}
        <Splitter
          orientation="vertical"
          bind:value={$terminalPanelHeight}
          min={120}
          max={Math.max(200, window.innerHeight - 200)}
          invert
        />
      {/if}
      <section
        class="terminal-panel"
        class:collapsed={$terminalCollapsed}
        style={$terminalCollapsed ? '' : `height: ${$terminalPanelHeight}px;`}
      >
        <div class="terminal-header">
          {#if $attachedSessions.length > 1}
            <div class="term-tabs">
              {#each $attachedSessions as t (t.sessionId)}
                {@const sess = $sessions.find((s) => s.tmux_session_id === t.sessionId)}
                <button
                  class="term-tab"
                  class:active={t.sessionId === $activeTerminalSessionId}
                  on:click={() => switchTerminal(t.sessionId)}
                >
                  <code>{t.sessionId}</code>
                  {#if sess}<span class="term-tab-name">{sess.name}</span>{/if}
                </button>
              {/each}
            </div>
          {:else}
            <span>Terminal: <code>{activeTerminalState?.command.join(' ')}</code></span>
          {/if}
          <div class="term-actions">
            <button
              class="icon-btn"
              on:click={popOutActive}
              disabled={!$activeTerminalSessionId}
              title="Open in a new window (tmux session keeps running everywhere)"
              aria-label="Pop out"
            >⤢</button>
            <button
              class="icon-btn"
              on:click={toggleCollapse}
              title={$terminalCollapsed ? 'Expand terminal pane' : 'Collapse terminal pane (keeps sessions running)'}
              aria-label={$terminalCollapsed ? 'Expand' : 'Collapse'}
            >{$terminalCollapsed ? '▴' : '▾'}</button>
            <button class="close-btn" on:click={closeTerminal} title="Detach (closes view; tmux session keeps running)">Close</button>
          </div>
        </div>
        <div class="terminal-host">
          <!-- All attached terminals stay mounted; only the active one is
               visible. This makes switching instant (no pty respawn or xterm
               re-init) and preserves scrollback. -->
          {#each $attachedSessions as t (t.sessionId)}
            {@const isVisible = t.sessionId === $activeTerminalSessionId && !$terminalCollapsed}
            <div class="terminal-slot" class:visible={isVisible}>
              <Terminal sessionId={t.sessionId} command={t.command} visible={isVisible} />
            </div>
          {/each}
        </div>
      </section>
    {/if}
  </main>
</div>
{/if}

<LaunchDialog bind:open={launchOpen} />
<WelcomeModal bind:open={welcomeOpen} />
<CommandPalette
  bind:open={paletteOpen}
  on:openLaunchDialog={() => (launchOpen = true)}
/>

<style>
  /* Popped-out window mode */
  .popout-root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #0a0c10;
  }
  .popout-header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 14px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .popout-title {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg);
  }
  .popout-hint {
    margin-left: auto;
    font-size: 10.5px;
    color: var(--fg-mute);
    font-style: italic;
  }
  .popout-term { flex: 1; overflow: hidden; }
  .popout-loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 13px;
  }

  .root { display: flex; height: 100vh; background: var(--bg); }
  main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .sidebar-wrap {
    flex-shrink: 0;
    display: flex;
    min-width: 0;
    height: 100vh;
  }
  .sidebar-show {
    position: absolute;
    top: 10px;
    left: 10px;
    z-index: 50;
    width: 30px;
    height: 30px;
    padding: 0;
    background: var(--bg-elev);
    color: var(--fg-dim);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    font-size: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: var(--shadow-sm);
  }
  .sidebar-show:hover { color: var(--fg); border-color: var(--border-strong); background: var(--bg-elev-2); }

  /* Reconnect banner */
  .reconnect-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 14px;
    background: var(--state-warn-bg);
    border-bottom: 1px solid color-mix(in srgb, var(--state-warn) 30%, transparent);
    color: var(--state-warn);
    font-size: 12px;
  }
  .reconnect-banner .dot {
    background: var(--state-warn);
    animation: blink 1.2s ease-in-out infinite;
  }
  .reconnect-banner .err {
    color: var(--state-error);
    font-family: var(--mono);
    margin-left: 6px;
    font-size: 11px;
  }
  .reconnect-banner .retry-btn {
    margin-left: auto;
    background: var(--state-warn);
    color: var(--bg);
    border: none;
    border-radius: var(--r-sm);
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 600;
  }
  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  /* Top header */
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
    flex-shrink: 0;
  }

  /* Pill tabs */
  .tabs {
    display: flex;
    gap: 2px;
    background: var(--bg);
    padding: 2px;
    border-radius: var(--r-md);
    border: 1px solid var(--border);
  }
  .pill {
    background: transparent;
    border: none;
    color: var(--fg-dim);
    padding: 4px 10px;
    font-size: 12px;
    font-weight: 500;
    border-radius: var(--r-sm);
    display: flex;
    align-items: center;
    gap: 6px;
    transition: color var(--t-fast), background var(--t-fast);
  }
  .pill:hover:not(.active) { color: var(--fg); background: var(--bg-elev-2); }
  .pill.active {
    background: var(--accent-bg-strong);
    color: var(--accent);
  }
  .count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 16px;
    height: 14px;
    padding: 0 4px;
    font-size: 9.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    background: var(--bg-elev-2);
    color: var(--fg-dim);
    border-radius: 7px;
  }
  .pill.active .count { background: var(--accent); color: var(--bg); }
  .count.pulse { animation: pulse-pop 1.8s ease-in-out infinite; background: var(--accent); color: var(--bg); }
  @keyframes pulse-pop {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.12); }
  }

  /* Action group */
  .actions { display: flex; gap: 6px; margin-left: auto; align-items: center; }
  .actions .primary {
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .actions .primary:hover:not(:disabled) { filter: brightness(1.08); background: var(--accent); }
  .actions .primary .plus { font-weight: 400; font-size: 14px; line-height: 1; opacity: 0.9; }

  .icon-btn {
    width: 26px;
    height: 26px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
    color: var(--fg-dim);
  }
  .icon-btn:hover { color: var(--fg); }

  .actions select {
    background: var(--bg);
    color: var(--fg-dim);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 3px 6px;
    font-size: 11px;
  }
  .actions select:hover { color: var(--fg); border-color: var(--border-strong); }
  .actions .theme-select { font-variant-caps: all-small-caps; letter-spacing: 0.5px; }

  /* Health indicator */
  .health {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    margin-left: 4px;
    flex-shrink: 0;
  }
  .health-green { background: var(--state-running); box-shadow: 0 0 0 2px var(--state-running-bg); }
  .health-yellow { background: var(--state-warn); animation: blink 1.2s ease-in-out infinite; }
  .health-red { background: var(--state-error); box-shadow: 0 0 0 2px var(--state-error-bg); }

  /* Content + terminal panel */
  .content { flex: 1; overflow-y: auto; min-height: 0; }
  .terminal-panel {
    /* height set inline from $terminalPanelHeight when expanded */
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: #0a0c10;
    flex-shrink: 0;
    min-height: 60px;
  }
  .terminal-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 4px 10px 4px 12px; background: var(--bg-elev); border-bottom: 1px solid var(--border);
    font-family: var(--mono); font-size: 11px; color: var(--fg-dim);
    gap: 8px;
  }
  .terminal-header code { color: var(--fg); }
  .terminal-header .close-btn {
    padding: 3px 10px;
    font-size: 11px;
  }
  .term-tabs {
    display: flex;
    gap: 2px;
    overflow-x: auto;
    flex: 1;
  }
  .term-tab {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: var(--r-sm);
    padding: 3px 9px;
    font-size: 11px;
    font-family: var(--sans);
    cursor: pointer;
  }
  .term-tab code {
    font-family: var(--mono);
    color: var(--fg-mute);
    font-size: 10.5px;
  }
  .term-tab:hover { color: var(--fg); border-color: var(--border-strong); background: var(--bg-elev-2); }
  .term-tab.active {
    background: var(--accent-bg-strong);
    color: var(--accent);
    border-color: var(--accent);
  }
  .term-tab.active code { color: var(--accent); }
  .term-tab-name {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .terminal-panel.collapsed { height: auto; }
  .terminal-panel.collapsed .terminal-host { display: none; }

  .term-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .term-actions .icon-btn {
    width: 24px;
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
  }
  .term-actions .icon-btn:hover:not(:disabled) { color: var(--fg); border-color: var(--border); background: var(--bg-elev-2); }
  .term-actions .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .terminal-host {
    flex: 1;
    overflow: hidden;
    position: relative;
  }
  .terminal-slot {
    position: absolute;
    inset: 0;
    visibility: hidden;
    /* Stay rendered but moved off-screen so xterm's resize observer fires
       when we re-show it. visibility: hidden is enough to hide it visually
       while keeping the element laid out. */
  }
  .terminal-slot.visible {
    visibility: visible;
    z-index: 1;
  }
</style>
