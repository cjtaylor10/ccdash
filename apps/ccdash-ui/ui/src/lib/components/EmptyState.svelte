<script lang="ts">
  import { projects } from '$lib/stores';
  import { addProjectViaPicker } from '$lib/projectActions';

  export let title: string;
  export let body: string = '';

  $: hasProjects = $projects.length > 0;

  let busy = false;
  let errMsg: string | null = null;

  async function add() {
    busy = true;
    errMsg = await addProjectViaPicker();
    busy = false;
  }
</script>

<div class="empty">
  <div class="icon" aria-hidden="true">
    <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
      <rect x="3" y="4" width="18" height="16" rx="2"/>
      <path d="M3 9h18"/>
      <path d="M8 14h3"/>
      <path d="M13 14h3"/>
    </svg>
  </div>
  <h2>{title}</h2>
  {#if !hasProjects}
    <p>Add your first project — point ccdash at a git repository on disk to register its worktrees and start tracking sessions.</p>
    {#if errMsg}<p class="err">{errMsg}</p>{/if}
    <button class="primary" on:click={add} disabled={busy}>
      {busy ? 'Adding…' : '+ Add a project'}
    </button>
  {:else}
    <p>{body || 'Nothing to show for the current selection.'}</p>
  {/if}
</div>

<style>
  .empty {
    padding: 56px 32px;
    text-align: center;
    color: var(--fg-dim);
    max-width: 460px;
    margin: 32px auto;
  }
  .icon {
    color: var(--fg-mute);
    margin: 0 auto 18px;
    width: 60px;
    height: 60px;
    border-radius: 14px;
    background: var(--bg-elev-2);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  h2 { margin: 0 0 8px; color: var(--fg); font-size: 16px; font-weight: 600; }
  p { margin: 6px 0; font-size: 12.5px; line-height: 1.55; }
  .err { color: var(--state-error); font-family: var(--mono); font-size: 11px; }
  .primary {
    margin-top: 20px;
    padding: 7px 18px;
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    border-radius: var(--r-md);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .primary:hover:not(:disabled) { filter: brightness(1.08); }
  .primary:disabled { opacity: 0.5; }
</style>
