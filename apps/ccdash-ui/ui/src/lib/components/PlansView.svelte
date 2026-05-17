<script lang="ts">
  import { marked } from 'marked';
  import { plans, projects } from '$lib/stores';
  import { openExternal } from '$lib/tauri';
  import EmptyState from './EmptyState.svelte';

  // Use a small inline-markdown render for task titles so backticks and
  // emphasis come through. We render synchronously to avoid the promise
  // pattern muddying templates.
  marked.setOptions({ async: false, gfm: true });

  function renderInline(s: string): string {
    return marked.parseInline(s) as string;
  }

  async function openInVSCode(path: string) {
    try { await openExternal(`vscode://file/${path}`); } catch {}
  }
</script>

{#if $projects.length === 0}
  <EmptyState title="No plans to show" />
{:else}
<div>
  {#each $plans as p (p.path)}
    <section>
      <div class="head">
        <h3>{p.title}</h3>
        <button class="open-vscode" on:click={() => openInVSCode(p.path)} title="Open in VS Code">Open in VS Code ↗</button>
      </div>
      <div class="path"><code>{p.path}</code></div>
      {#each p.phases as phase (phase.name)}
        <div class="phase">
          <h4>{@html renderInline(phase.name)}</h4>
          <ul>
            {#each phase.tasks as t (t.title)}
              <li class:done={t.done}>
                <span class="check">{t.done ? '✓' : '○'}</span>
                <span>{@html renderInline(t.title)}</span>
              </li>
            {/each}
          </ul>
          {#if phase.tasks.length > 0}
            <div class="progress">
              {phase.tasks.filter((t) => t.done).length}/{phase.tasks.length} done
            </div>
          {/if}
        </div>
      {/each}
    </section>
  {:else}
    <div class="empty">(no plans found under docs/superpowers/&#123;specs,plans&#125;/)</div>
  {/each}
</div>
{/if}

<style>
  section { padding: 16px 20px; border-bottom: 1px solid var(--border); }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .open-vscode {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    border-radius: 4px;
    padding: 3px 9px;
    font-size: 11px;
  }
  .open-vscode:hover { color: var(--accent); border-color: var(--accent); }
  h3 { margin: 0 0 4px; font-size: 16px; }
  .path { font-family: var(--mono); font-size: 11px; color: var(--fg-dim); margin-bottom: 12px; }
  .phase { margin: 12px 0; }
  h4 { margin: 8px 0 4px; font-size: 13px; color: var(--accent); }
  ul { list-style: none; padding: 0; margin: 0; }
  li { padding: 2px 0; font-size: 13px; }
  li.done { color: var(--fg-dim); text-decoration: line-through; }
  .check { display: inline-block; width: 16px; text-align: center; color: var(--success); }
  li:not(.done) .check { color: var(--fg-dim); }
  .progress { font-size: 11px; color: var(--fg-dim); margin-top: 4px; padding-left: 16px; }
  .empty { padding: 32px; text-align: center; color: var(--fg-dim); font-style: italic; }
</style>
