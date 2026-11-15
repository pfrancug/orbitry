<script lang="ts">
  import type { DetectedScript } from "$lib/types";

  let { scripts }: { scripts: DetectedScript[] } = $props();
</script>

<div class="scripts">
  {#if scripts.length === 0}
    <span class="muted">No dev or preview scripts</span>
  {:else}
    {#each scripts as script (script.kind + script.name)}
      <span class="chip scope-{script.scope}">
        <em>{script.scope}</em>
        {script.name}
      </span>
    {/each}
  {/if}
</div>