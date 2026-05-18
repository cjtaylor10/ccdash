<script lang="ts">
  import { activeView } from '$lib/stores';
</script>

<nav class="sidebar-nav" aria-label="Primary">
  <button
    class="nav-item"
    class:active={$activeView === 'workspace'}
    on:click={() => activeView.set('workspace')}
    title="Workspace (Sessions, Ports, Plans, Browser)"
    aria-current={$activeView === 'workspace' ? 'page' : undefined}
  >
    <span class="icon" aria-hidden="true">📁</span>
    <span class="label">Workspace</span>
  </button>
  <button
    class="nav-item"
    class:active={$activeView === 'prompts'}
    on:click={() => activeView.set('prompts')}
    title="Prompts library"
    aria-current={$activeView === 'prompts' ? 'page' : undefined}
  >
    <span class="icon" aria-hidden="true">📝</span>
    <span class="label">Prompts</span>
  </button>
</nav>

<style>
  .sidebar-nav {
    display: flex;
    flex-direction: column;
    padding: 6px 6px 8px;
    border-bottom: 1px solid var(--border);
    gap: 2px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    background: transparent;
    color: var(--fg-dim);
    border: none;
    border-left: 2px solid transparent;
    border-radius: var(--r-sm);
    font-size: 12.5px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background var(--t-fast), color var(--t-fast);
  }
  .nav-item:hover { background: var(--bg-elev-2); color: var(--fg); }
  .nav-item.active {
    background: var(--accent-bg);
    color: var(--accent);
    border-left-color: var(--accent);
  }
  .nav-item .icon { font-size: 13px; line-height: 1; }
  .nav-item .label { flex: 1; }
</style>
