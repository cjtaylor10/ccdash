<script lang="ts">
  import { plans } from '$lib/stores';
</script>

<div>
  {#each $plans as p (p.path)}
    <section>
      <h3>{p.title}</h3>
      <div class="path"><code>{p.path}</code></div>
      {#each p.phases as phase (phase.name)}
        <div class="phase">
          <h4>{phase.name}</h4>
          <ul>
            {#each phase.tasks as t (t.title)}
              <li class:done={t.done}>
                <span class="check">{t.done ? '✓' : '○'}</span>
                {t.title}
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

<style>
  section { padding: 16px 20px; border-bottom: 1px solid var(--border); }
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
