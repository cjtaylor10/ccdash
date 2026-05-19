<script lang="ts">
  import {
    attachedSessions,
    browserPaneSubtabByPaneId,
    browserStateBySession,
    detectedUrlsBySession,
    resolvedProjectByTmuxId,
    selectedProjectId,
    sessions,
  } from '$lib/stores';
  import { openExternal, screenshot as screenshotApi } from '$lib/tauri';
  import { showToast } from '$lib/toast';
  import { invoke } from '@tauri-apps/api/core';

  export let paneId: string;

  let errMsg: string | null = null;

  $: hasActiveSubtab = viewSession !== null;

  /** Attached sessions that belong to the currently-selected project.
   *  Uses the shared `resolvedProjectByTmuxId` lookup so sessions started
   *  outside ccdash (no daemon-stamped `project_id`) are still attributed
   *  via cwd inference — same logic the sidebar tree uses. When no project
   *  is selected, every attached session is in scope. */
  $: inScopeSessions = (() => {
    const pid = $selectedProjectId;
    if (pid === null) return $attachedSessions;
    const resolved = $resolvedProjectByTmuxId;
    return $attachedSessions.filter((t) => resolved.get(t.sessionId) === pid);
  })();

  /** This pane's selected sub-tab. Reads from the per-pane map; if stale
   *  (no longer in scope) or unset, falls back to the first in-scope
   *  session. `null` if no sessions are in scope at all. */
  $: viewSession = (() => {
    const stored = $browserPaneSubtabByPaneId.get(paneId);
    if (stored && inScopeSessions.some((t) => t.sessionId === stored)) return stored;
    if (inScopeSessions.length > 0) return inScopeSessions[0].sessionId;
    return null;
  })();

  function selectSubtab(sessionId: string) {
    browserPaneSubtabByPaneId.update((m) => {
      const next = new Map(m);
      next.set(paneId, sessionId);
      return next;
    });
  }

  /** Default per-session browser state shape — created lazily on first use. */
  function defaultState() {
    return { history: [], index: -1, address: '', reloadCounter: 0 };
  }

  /** Live view of the active session's browser state. */
  $: state =
    $browserStateBySession.get(viewSession) ?? defaultState();
  $: current = state.index >= 0 ? state.history[state.index] : null;
  $: canBack = state.index > 0;
  $: canForward = state.index < state.history.length - 1;

  /** Detected URLs the active session can navigate to. The global (null)
   *  port-derived set is always merged in so machine-wide listeners show
   *  up regardless of which session detected them. */
  $: sortedUrls = (() => {
    const m = $detectedUrlsBySession;
    const merged = new Set<string>();
    for (const u of m.get(null) ?? []) merged.add(u);
    if (viewSession !== null) {
      for (const u of m.get(viewSession) ?? []) merged.add(u);
    } else {
      // "All" view: union of every per-session set.
      for (const [k, set] of m) {
        if (k === null) continue;
        for (const u of set) merged.add(u);
      }
    }
    return [...merged].sort();
  })();

  function updateState(mut: (s: { history: string[]; index: number; address: string; reloadCounter: number }) => void) {
    browserStateBySession.update((m) => {
      const next = new Map(m);
      const s = { ...(next.get(viewSession) ?? defaultState()) };
      mut(s);
      next.set(viewSession, s);
      return next;
    });
  }

  function navigate(url: string) {
    if (!url) return;
    errMsg = null;
    updateState((s) => {
      s.history = [...s.history.slice(0, s.index + 1), url];
      s.index = s.history.length - 1;
      s.address = url;
    });
  }

  function go() {
    let url = state.address.trim();
    if (!url) return;
    if (!/^https?:\/\//i.test(url)) url = `http://${url}`;
    navigate(url);
  }

  function back() {
    if (!canBack) return;
    updateState((s) => {
      s.index--;
      s.address = s.history[s.index];
    });
  }

  function forward() {
    if (!canForward) return;
    updateState((s) => {
      s.index++;
      s.address = s.history[s.index];
    });
  }

  function reload() {
    updateState((s) => { s.reloadCounter++; });
  }

  async function external() {
    if (!current) return;
    try {
      await openExternal(current);
    } catch (e) {
      errMsg = String(e);
    }
  }

  /** Reference to the iframe-host element — we capture its bounding rect
   *  so the screenshot covers exactly the rendered preview area (no chrome
   *  bar, no context bar, no left rail). */
  let iframeHost: HTMLDivElement;

  async function snapshot() {
    if (!iframeHost) return;
    const r = iframeHost.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) {
      showToast('Browser preview has no visible area to capture', 'err');
      return;
    }
    try {
      await screenshotApi.region(r.left, r.top, r.width, r.height);
      showToast('Browser screenshot copied to clipboard');
    } catch (e) {
      const msg = e && typeof e === 'object' && 'message' in e ? (e as { message: string }).message : String(e);
      await invoke('log_from_frontend', { level: 'error', message: `screenshot_region failed: ${msg}` }).catch(() => {});
      showToast(`Screenshot failed: ${msg}`, 'err');
    }
  }

  function onAddressInput(e: Event) {
    const v = (e.target as HTMLInputElement).value;
    updateState((s) => { s.address = v; });
  }

  function onAddressKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') go();
  }
</script>

<div class="root">
  <div class="chrome">
    <button class="nav" on:click={back} disabled={!canBack} title="Back">‹</button>
    <button class="nav" on:click={forward} disabled={!canForward} title="Forward">›</button>
    <button class="nav" on:click={reload} disabled={!current} title="Reload">↻</button>
    <input
      type="text"
      class="address"
      value={state.address}
      on:input={onAddressInput}
      on:keydown={onAddressKeydown}
      placeholder="http://localhost:3000"
    />
    <button class="go" on:click={go}>Go</button>
    <button class="ext" on:click={external} disabled={!current} title="Open in external browser">↗</button>
    <button class="ext" on:click={snapshot} disabled={!current} title="Screenshot preview to clipboard" aria-label="Screenshot preview">⎙</button>
  </div>

  {#if inScopeSessions.length > 0}
    <div class="subtabs">
      {#each inScopeSessions as t (t.sessionId)}
        {@const sess = $sessions.find((s) => s.tmux_session_id === t.sessionId)}
        <button
          class="subtab"
          class:active={t.sessionId === viewSession}
          on:click={() => selectSubtab(t.sessionId)}
          title={t.sessionId}
        >
          <span class="subtab-name">{sess?.name ?? t.sessionId}</span>
          <code>{t.sessionId}</code>
        </button>
      {/each}
    </div>
  {:else}
    <div class="subtabs subtabs-empty">
      <span>No attached sessions in this project. Launch one to start browsing.</span>
    </div>
  {/if}

  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}

  <div class="main">
    <aside class="rail">
      <h3>Detected</h3>
      {#if sortedUrls.length === 0}
        <div class="empty">
          Launch a dev server and we'll surface its URL here. Running ports + terminal output (for this session) are both watched.
        </div>
      {:else}
        <ul>
          {#each sortedUrls as u (u)}
            <li class:active={current === u}>
              <button on:click={() => navigate(u)}><code>{u}</code></button>
            </li>
          {/each}
        </ul>
      {/if}
    </aside>

    <div class="iframe-host" bind:this={iframeHost}>
      {#if current}
        {#key `${viewSession ?? '__all__'}::${current}::${state.reloadCounter}`}
          <iframe title="Preview" src={current}></iframe>
        {/key}
      {:else}
        <div class="placeholder">
          {#if hasActiveSubtab}
            <p>No URL loaded for this session.</p>
            <p>Pick one from the left rail or type an address up top.</p>
          {:else}
            <p>No session selected in this project.</p>
            <p>Launch or attach a session in the sidebar to start browsing.</p>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .root {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .chrome {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .nav, .go, .ext {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: var(--r-sm);
    padding: 3px 9px;
    font-size: 13px;
    cursor: pointer;
    min-width: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .nav:hover:not(:disabled), .ext:hover:not(:disabled) { color: var(--fg); border-color: var(--border-strong); background: var(--bg-elev-2); }
  .nav:disabled, .ext:disabled { opacity: 0.3; cursor: not-allowed; }
  .address {
    flex: 1;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 4px 10px;
    font-family: var(--mono);
    font-size: 11.5px;
  }
  .go {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
    font-weight: 600;
    font-size: 11.5px;
    min-width: auto;
    padding: 4px 12px;
  }
  .go:hover:not(:disabled) { filter: brightness(1.08); color: var(--bg); }
  .err {
    padding: 7px 12px;
    background: var(--state-error-bg);
    color: var(--state-error);
    font-size: 11.5px;
  }
  .subtabs {
    display: flex;
    gap: 2px;
    padding: 4px 8px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
    flex-shrink: 0;
  }
  .subtab {
    display: inline-flex;
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
    flex-shrink: 0;
  }
  .subtab:hover {
    color: var(--fg);
    border-color: var(--border-strong);
    background: var(--bg-elev-2);
  }
  .subtab.active {
    background: var(--accent-bg-strong);
    color: var(--accent);
    border-color: var(--accent);
  }
  .subtab.active code { color: var(--accent); }
  .subtab code {
    font-family: var(--mono);
    color: var(--fg-mute);
    font-size: 10.5px;
  }
  .subtab-name {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subtabs-empty {
    padding: 8px 12px;
    color: var(--fg-mute);
    font-size: 11.5px;
    font-style: italic;
  }
  .main {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .rail {
    width: 220px;
    min-width: 220px;
    background: var(--bg-elev);
    border-right: 1px solid var(--border);
    overflow-y: auto;
    padding: 8px 0;
  }
  .rail h3 {
    margin: 0 12px 8px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--fg-mute);
    font-weight: 600;
  }
  .empty {
    padding: 14px 16px;
    color: var(--fg-mute);
    font-size: 11.5px;
    line-height: 1.55;
  }
  .rail ul { list-style: none; margin: 0; padding: 0; }
  .rail li button {
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    padding: 5px 12px;
    font-size: 11px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  .rail li button:hover:not(:disabled) { background: var(--bg-elev-2); color: var(--fg); }
  .rail li button:disabled { cursor: not-allowed; opacity: 0.45; }
  .rail li.active button {
    background: var(--accent-bg);
    color: var(--accent);
    border-left-color: var(--accent);
    padding-left: 10px;
  }
  .rail li button code { font-family: var(--mono); word-break: break-all; }
  .iframe-host {
    flex: 1;
    background: #fff;
    position: relative;
  }
  iframe {
    width: 100%;
    height: 100%;
    border: 0;
  }
  .placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: var(--bg);
    color: var(--fg-dim);
    text-align: center;
    padding: 32px;
  }
  .placeholder p { margin: 4px 0; font-size: 13px; }
</style>
