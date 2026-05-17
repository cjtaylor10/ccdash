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
  <h2>{title}</h2>
  {#if !hasProjects}
    <p>Get started by adding your first project — point ccdash at a git repository on disk.</p>
    {#if errMsg}<p class="err">{errMsg}</p>{/if}
    <button class="primary" on:click={add} disabled={busy}>
      {busy ? 'Adding…' : 'Add a project'}
    </button>
  {:else}
    <p>{body}</p>
  {/if}
</div>

<style>
  .empty {
    padding: 48px 24px;
    text-align: center;
    color: var(--fg-dim);
    max-width: 480px;
    margin: 24px auto;
  }
  h2 { margin: 0 0 12px; color: var(--fg); font-size: 18px; }
  p { margin: 8px 0; font-size: 13px; }
  .err { color: var(--danger); font-family: var(--mono); font-size: 11px; }
  .primary {
    margin-top: 16px;
    padding: 8px 18px;
    background: var(--accent);
    color: var(--bg);
    border: 1px solid var(--accent);
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .primary:disabled { opacity: 0.5; }
</style>
