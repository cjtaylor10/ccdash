<script lang="ts">
  import {
    activeTerminalSessionId,
    attachedSessions,
    browserStateBySession,
    detectedUrlsBySession,
    sessions,
  } from '$lib/stores';
  import { openExternal, screenshot as screenshotApi } from '$lib/tauri';
  import { showToast } from '$lib/toast';
  import { invoke } from '@tauri-apps/api/core';

  let errMsg: string | null = null;

  /** Which session's browser context is showing. Defaults to the currently
   *  attached terminal — switching terminals switches the browser too,
   *  preserving each session's own URL + history. The user can also pick
   *  "All" (null) to see machine-wide URL detection. */
  let viewSession: string | null = $activeTerminalSessionId;

  // Keep the browser context synced to the active terminal — unless the user
  // explicitly overrode by picking another session from the dropdown.
  let userOverride = false;
  $: if (!userOverride) viewSession = $activeTerminalSessionId;

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

  function onContextChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value;
    viewSession = v === '__all__' ? null : v;
    userOverride = true;
  }

  function followActive() {
    userOverride = false;
    viewSession = $activeTerminalSessionId;
  }

  $: contextLabel = (() => {
    if (viewSession === null) return 'All sessions';
    const sess = $sessions.find((s) => s.tmux_session_id === viewSession);
    return sess?.name ?? viewSession;
  })();
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

  <div class="context-bar">
    <span class="ctx-label">Browser for:</span>
    <select class="ctx-select" value={viewSession ?? '__all__'} on:change={onContextChange}>
      <option value="__all__">All sessions</option>
      {#each $attachedSessions as t (t.sessionId)}
        {@const sess = $sessions.find((s) => s.tmux_session_id === t.sessionId)}
        <option value={t.sessionId}>
          {sess?.name ?? t.sessionId} ({t.sessionId})
        </option>
      {/each}
    </select>
    {#if userOverride && $activeTerminalSessionId && viewSession !== $activeTerminalSessionId}
      <button class="follow-btn" on:click={followActive} title="Follow the currently-attached session">
        ↺ follow active
      </button>
    {/if}
    <span class="ctx-name">{contextLabel}</span>
  </div>

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
          <p>No URL loaded for {contextLabel}.</p>
          <p>Pick one from the left rail or type an address up top.</p>
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
  .context-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-dim);
  }
  .ctx-label {
    text-transform: uppercase;
    letter-spacing: 0.6px;
    font-size: 10px;
    color: var(--fg-mute);
  }
  .ctx-select {
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 2px 6px;
    font-size: 11px;
    max-width: 280px;
  }
  .follow-btn {
    background: transparent;
    border: 1px solid var(--accent);
    color: var(--accent);
    border-radius: var(--r-sm);
    padding: 2px 8px;
    font-size: 10.5px;
  }
  .follow-btn:hover { background: var(--accent-bg); }
  .ctx-name {
    margin-left: auto;
    font-family: var(--mono);
    color: var(--fg-mute);
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
  .rail li button:hover { background: var(--bg-elev-2); color: var(--fg); }
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
