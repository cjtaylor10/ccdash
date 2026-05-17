<script lang="ts">
  import { detectedUrls } from '$lib/stores';
  import { openExternal } from '$lib/tauri';

  /** Simple back/forward history of URLs the user has navigated to. */
  let history: string[] = [];
  let historyIndex = -1;
  /** Address bar text — may diverge from `current` until the user hits Go. */
  let address = '';
  let reloadCounter = 0;
  let errMsg: string | null = null;

  $: current = historyIndex >= 0 ? history[historyIndex] : null;
  $: canBack = historyIndex > 0;
  $: canForward = historyIndex < history.length - 1;
  $: sortedUrls = [...$detectedUrls].sort();

  function navigate(url: string) {
    if (!url) return;
    errMsg = null;
    // Truncate the forward tail when navigating from a back-traversed state.
    history = [...history.slice(0, historyIndex + 1), url];
    historyIndex = history.length - 1;
    address = url;
  }

  function go() {
    let url = address.trim();
    if (!url) return;
    if (!/^https?:\/\//i.test(url)) {
      url = `http://${url}`;
    }
    navigate(url);
  }

  function back() {
    if (canBack) {
      historyIndex--;
      address = history[historyIndex];
    }
  }

  function forward() {
    if (canForward) {
      historyIndex++;
      address = history[historyIndex];
    }
  }

  function reload() {
    reloadCounter++;
  }

  async function external() {
    if (!current) return;
    try {
      await openExternal(current);
    } catch (e) {
      errMsg = String(e);
    }
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
      bind:value={address}
      on:keydown={onAddressKeydown}
      placeholder="http://localhost:3000"
    />
    <button class="go" on:click={go}>Go</button>
    <button class="ext" on:click={external} disabled={!current} title="Open in external browser">↗</button>
  </div>

  {#if errMsg}
    <div class="err">{errMsg}</div>
  {/if}

  <div class="main">
    <aside class="rail">
      <h3>Detected</h3>
      {#if sortedUrls.length === 0}
        <div class="empty">
          Launch a dev server and we'll surface its URL here. Running ports + terminal output are both watched.
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

    <div class="iframe-host">
      {#if current}
        {#key `${current}::${reloadCounter}`}
          <iframe title="Preview" src={current}></iframe>
        {/key}
      {:else}
        <div class="placeholder">
          <p>No URL loaded.</p>
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
