<script lang="ts">
  import { marked } from 'marked';
  import { plans, projects } from '$lib/stores';
  import { openExternal } from '$lib/tauri';
  import EmptyState from './EmptyState.svelte';

  marked.setOptions({ async: false, gfm: true });

  function renderInline(s: string): string {
    return marked.parseInline(s) as string;
  }

  async function openInVSCode(path: string) {
    try { await openExternal(`vscode://file/${path}`); } catch {}
  }

  function shortPath(p: string): string {
    const home = '/Users/';
    if (p.startsWith(home)) {
      const rest = p.slice(home.length);
      const slash = rest.indexOf('/');
      if (slash > 0) return '~' + rest.slice(slash);
    }
    return p;
  }
</script>

{#if $projects.length === 0}
  <EmptyState title="No plans to show" />
{:else}
  <div class="root">
    {#each $plans as p (p.path)}
      {@const totalTasks = p.phases.reduce((sum, ph) => sum + ph.tasks.length, 0)}
      {@const doneTasks = p.phases.reduce((sum, ph) => sum + ph.tasks.filter((t) => t.done).length, 0)}
      {@const pct = totalTasks > 0 ? Math.round((doneTasks / totalTasks) * 100) : 0}
      <article>
        <header>
          <div class="title-row">
            <h3>{p.title}</h3>
            <button class="open-vscode" on:click={() => openInVSCode(p.path)} title="Open in VS Code">
              VS Code <span class="ext">↗</span>
            </button>
          </div>
          <div class="path"><code>{shortPath(p.path)}</code></div>
          {#if totalTasks > 0}
            <div class="bar" title="{doneTasks} of {totalTasks} tasks done">
              <div class="bar-fill" style="width: {pct}%"></div>
              <span class="bar-label">{doneTasks}/{totalTasks} · {pct}%</span>
            </div>
          {/if}
        </header>
        {#each p.phases as phase (phase.name)}
          {@const phaseDone = phase.tasks.filter((t) => t.done).length}
          {@const phaseTotal = phase.tasks.length}
          <section class="phase">
            <h4>
              <span>{@html renderInline(phase.name)}</span>
              {#if phaseTotal > 0}
                <span class="phase-progress">{phaseDone}/{phaseTotal}</span>
              {/if}
            </h4>
            <ul>
              {#each phase.tasks as t (t.title)}
                <li class:done={t.done}>
                  <span class="check">{t.done ? '✓' : ''}</span>
                  <span class="task-text">{@html renderInline(t.title)}</span>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </article>
    {:else}
      <div class="empty">
        No plans found under <code>docs/superpowers/&#123;specs,plans&#125;/</code>.
      </div>
    {/each}
  </div>
{/if}

<style>
  .root { padding: 8px 0; }
  article {
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }
  header { margin-bottom: 12px; }
  .title-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  h3 {
    margin: 0;
    font-size: 14.5px;
    font-weight: 600;
    color: var(--fg);
  }
  .open-vscode {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: var(--r-sm);
    padding: 3px 9px;
    font-size: 11px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .open-vscode:hover { color: var(--accent); border-color: var(--accent); }
  .ext { font-size: 10px; opacity: 0.7; }
  .path {
    margin-top: 2px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-mute);
  }

  .bar {
    position: relative;
    margin-top: 10px;
    height: 4px;
    background: var(--bg-elev-2);
    border-radius: 2px;
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width var(--t-base);
  }
  .bar-label {
    position: absolute;
    right: 0;
    top: 8px;
    font-size: 10.5px;
    color: var(--fg-mute);
    font-variant-numeric: tabular-nums;
  }

  .phase {
    margin-top: 18px;
    padding-left: 2px;
  }
  h4 {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 0 0 6px;
    font-size: 12px;
    color: var(--accent);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }
  .phase-progress {
    font-size: 10px;
    color: var(--fg-mute);
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
    font-variant-numeric: tabular-nums;
  }
  ul { list-style: none; padding: 0; margin: 0; }
  li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 2px 0;
    font-size: 12.5px;
    color: var(--fg);
  }
  li.done { color: var(--fg-mute); }
  li.done .task-text { text-decoration: line-through; text-decoration-color: var(--fg-mute); }
  .check {
    display: inline-block;
    width: 14px;
    height: 14px;
    border-radius: 3px;
    border: 1px solid var(--border-strong);
    text-align: center;
    line-height: 12px;
    font-size: 10px;
    color: transparent;
    flex-shrink: 0;
    margin-top: 2px;
  }
  li.done .check {
    background: var(--state-running);
    border-color: var(--state-running);
    color: var(--bg);
    font-weight: 700;
  }
  .task-text :global(code) {
    font-family: var(--mono);
    background: var(--bg-elev-2);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 11.5px;
  }
  .empty {
    padding: 40px;
    text-align: center;
    color: var(--fg-dim);
    font-style: italic;
    font-size: 12.5px;
  }
  .empty code { font-style: normal; font-family: var(--mono); background: var(--bg-elev-2); padding: 1px 6px; border-radius: 3px; }
</style>
