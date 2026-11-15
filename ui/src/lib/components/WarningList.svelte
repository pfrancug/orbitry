<script lang="ts">
  import type { AppWarning } from "$lib/types";

  let { warnings = [], compact = false }: { warnings?: AppWarning[]; compact?: boolean } = $props();
</script>

{#if warnings.length}
  <ul class="warning-list" class:compact>
    {#each warnings as warning (`${warning.code}:${warning.port ?? ""}:${warning.partId ?? ""}:${warning.message}`)}
      <li class="warning-item sev-{warning.severity}">
        <strong>{warning.message}</strong>
        {#if warning.hint && !compact}
          <span class="hint">{warning.hint}</span>
        {/if}
      </li>
    {/each}
  </ul>
{/if}
