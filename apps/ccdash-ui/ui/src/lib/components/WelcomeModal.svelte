<script lang="ts">
  import { open as openFolderDialog } from '@tauri-apps/plugin-dialog';
  import { projects } from '$lib/stores';
  import { daemonApi, projectsApi, tauri, type DiscoveredRepo } from '$lib/tauri';

  export let open: boolean = false;

  let roots: string[] = [];
  let scanning = false;
  let scanned = false;
  let discovered: DiscoveredRepo[] = [];
  let selected: Set<string> = new Set();
  let busy = false;
  let errMsg: string | null = null;

  async function pickRoot() {
    try {
      const picked = await openFolderDialog({ directory: true, multiple: false, title: 'Pick directory to scan' });
      if (typeof picked === 'string') {
        if (!roots.includes(picked)) roots = [...roots, picked];
      }
    } catch (e) {
      errMsg = String(e);
    }
  }

  function removeRoot(r: string) {
    roots = roots.filter((x) => x !== r);
  }

  async function runScan() {
    if (roots.length === 0) {
      errMsg = 'Pick at least one directory to scan.';
      return;
    }
    scanning = true;
    errMsg = null;
    try {
      const { discovered: d } = await daemonApi.scanPaths(roots);
      discovered = d;
      selected = new Set(d.map((r) => r.path));
      scanned = true;
    } catch (e) {
      errMsg = String(e);
    } finally {
      scanning = false;
    }
  }

  function toggle(path: string) {
    const next = new Set(selected);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    selected = next;
  }

  async function addSelected() {
    busy = true;
    errMsg = null;
    const failures: string[] = [];
    try {
      for (const repo of discovered) {
        if (!selected.has(repo.path)) continue;
        try {
          await projectsApi.add(repo.path, repo.suggested_name);
        } catch (e) {
          failures.push(`${repo.suggested_name}: ${String(e)}`);
        }
      }
      const { projects: ps } = await tauri.projectList();
      projects.set(ps);
      if (failures.length > 0) {
        errMsg = `Some adds failed:\n${failures.join('\n')}`;
      }
      await daemonApi.firstRunComplete();
      open = false;
    } catch (e) {
      errMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function skip() {
    try {
      await daemonApi.firstRunComplete();
    } catch {}
    open = false;
  }

</script>

{#if open}
  <div class="backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true">
      <header>
        <h2>Welcome to ccdash</h2>
        <p>Let's find your git repositories so you can launch Claude sessions in them.</p>
      </header>

      <div class="body">
        {#if !scanned}
          <section>
            <h3>1. Pick directories to scan</h3>
            <p class="hint">Most people pick <code>~/Documents</code> or wherever your code lives. We'll look for git repos up to 4 directories deep.</p>
            {#if roots.length === 0}
              <div class="empty">No directories picked yet.</div>
            {:else}
              <ul class="roots">
                {#each roots as r (r)}
                  <li>
                    <code>{r}</code>
                    <button class="x" on:click={() => removeRoot(r)} aria-label="Remove">×</button>
                  </li>
                {/each}
              </ul>
            {/if}
            <button on:click={pickRoot} disabled={scanning}>+ Pick a directory</button>
          </section>
        {:else}
          <section>
            <h3>2. Pick the projects you want to register ({selected.size}/{discovered.length})</h3>
            {#if discovered.length === 0}
              <div class="empty">No git repositories found in the directories you picked.</div>
            {:else}
              <ul class="discovered">
                {#each discovered as r (r.path)}
                  <li>
                    <label>
                      <input
                        type="checkbox"
                        checked={selected.has(r.path)}
                        on:change={() => toggle(r.path)}
                      />
                      <span class="name">{r.suggested_name}</span>
                      <code class="path">{r.path}</code>
                    </label>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/if}

        {#if errMsg}
          <pre class="err">{errMsg}</pre>
        {/if}
      </div>

      <footer>
        <button on:click={skip} disabled={busy || scanning}>Skip for now</button>
        {#if !scanned}
          <button class="primary" on:click={runScan} disabled={scanning || roots.length === 0}>
            {scanning ? 'Scanning…' : 'Scan'}
          </button>
        {:else if discovered.length === 0}
          <button class="primary" on:click={() => { scanned = false; discovered = []; }}>Try again</button>
        {:else}
          <button class="primary" on:click={addSelected} disabled={busy || selected.size === 0}>
            {busy ? 'Adding…' : `Add ${selected.size} project${selected.size === 1 ? '' : 's'}`}
          </button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .modal {
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    min-width: 560px;
    max-width: 720px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-lg);
    animation: popIn 180ms ease-out;
  }
  @keyframes popIn { from { transform: scale(0.96); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  header { padding: 22px 26px 16px; border-bottom: 1px solid var(--border); }
  header h2 { margin: 0 0 4px; font-size: 17px; color: var(--accent); font-weight: 600; }
  header p { margin: 0; color: var(--fg-dim); font-size: 12.5px; line-height: 1.5; }
  .body {
    padding: 18px 24px;
    overflow-y: auto;
    flex: 1;
  }
  .body h3 { font-size: 14px; margin: 0 0 8px; color: var(--fg); }
  .body .hint { font-size: 12px; color: var(--fg-dim); margin: 0 0 12px; }
  .body .hint code {
    background: var(--bg);
    padding: 1px 5px;
    border-radius: 3px;
    font-family: var(--mono);
  }
  .empty { padding: 10px 12px; color: var(--fg-dim); font-style: italic; font-size: 12px; }
  ul { list-style: none; margin: 0 0 12px; padding: 0; }
  .roots li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: var(--bg);
    border-radius: 4px;
    margin-bottom: 4px;
    font-family: var(--mono);
    font-size: 12px;
  }
  .roots li code { flex: 1; }
  .roots li .x {
    background: transparent;
    border: none;
    color: var(--danger);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    padding: 0 4px;
  }
  .discovered li {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
  }
  .discovered label {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    align-items: center;
    cursor: pointer;
  }
  .discovered .name { font-weight: 600; }
  .discovered .path { font-family: var(--mono); font-size: 11px; color: var(--fg-dim); }
  .err {
    padding: 10px 12px;
    background: rgba(255, 0, 0, 0.1);
    color: var(--danger);
    border-radius: 4px;
    font-size: 12px;
    white-space: pre-wrap;
  }
  footer {
    padding: 14px 24px;
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  footer button {
    padding: 6px 16px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    border-radius: 4px;
    font-size: 13px;
  }
  footer .primary {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }
  footer button:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
