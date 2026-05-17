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
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .nav, .go, .ext {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--fg);
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 14px;
    cursor: pointer;
    min-width: 30px;
  }
  .nav:disabled, .ext:disabled { opacity: 0.4; cursor: not-allowed; }
  .address {
    flex: 1;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 5px 10px;
    font-family: var(--mono);
    font-size: 12px;
  }
  .go {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
    font-weight: 600;
  }
  .err {
    padding: 6px 12px;
    background: rgba(255, 0, 0, 0.1);
    color: var(--danger);
    font-size: 12px;
  }
  .main {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .rail {
    width: 220px;
    background: var(--bg-elev);
    border-right: 1px solid var(--border);
    overflow-y: auto;
    padding: 8px 0;
  }
  .rail h3 {
    margin: 0 12px 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--fg-dim);
  }
  .empty {
    padding: 12px;
    color: var(--fg-dim);
    font-size: 12px;
    font-style: italic;
  }
  .rail ul { list-style: none; margin: 0; padding: 0; }
  .rail li button {
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--fg);
    padding: 6px 12px;
    font-size: 12px;
    cursor: pointer;
  }
  .rail li button:hover { background: var(--accent-bg); }
  .rail li.active button { background: var(--accent-bg); border-left: 2px solid var(--accent); padding-left: 10px; }
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
