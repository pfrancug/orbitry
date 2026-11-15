<script lang="ts">
  import { isPending, runState, toggleScript } from "$lib/runs.svelte";
  import type { DetectedScript } from "$lib/types";

  let { partId, scripts }: { partId: string; scripts: DetectedScript[] } = $props();
</script>

<div class="scripts">
  {#if scripts.length === 0}
    <span class="muted">No dev or preview scripts</span>
  {:else}
    {#each scripts as script (script.kind + script.name)}
      {@const run = runState.runs.find(
        (item) => item.status === "running" && item.partId === partId && item.script === script.name,
      )}
      {@const pending = isPending(partId, script.name)}
      <button
        class="chip scope-{script.scope}"
        class:running={Boolean(run)}
        class:pending
        type="button"
        aria-pressed={Boolean(run)}
        aria-label={run ? `Stop ${script.name}` : `Start ${script.name}`}
        disabled={pending}
        onclick={(event) => {
          event.stopPropagation();
          toggleScript(partId, script.name);
        }}
      >
        {#if run}
          <span class="pulse" aria-hidden="true"></span>
        {:else}
          <span class="play-icon" aria-hidden="true"></span>
        {/if}
        <em>{script.scope}</em>
        {script.name}
      </button>
    {/each}
  {/if}
</div>
