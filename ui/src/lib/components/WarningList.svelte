<script lang="ts">
  import type { AppWarning } from "$lib/types";
  import { dedupeWarnings, groupWarnings } from "$lib/format";

  let {
    warnings = [],
    compact = false,
    flat = false,
  }: {
    warnings?: AppWarning[];
    compact?: boolean;
    flat?: boolean;
  } = $props();

  const items = $derived(flat ? dedupeWarnings(warnings ?? []) : []);
  const groups = $derived(flat ? [] : groupWarnings(warnings ?? []));
  let openCodes = $state<Record<string, boolean>>({});

  function toggle(code: string) {
    openCodes = { ...openCodes, [code]: !openCodes[code] };
  }
</script>

{#if flat}
  {#if items.length}
    <ul class="warning-list" class:compact>
      {#each items as warning (`${warning.code}:${warning.port ?? ""}:${warning.partId ?? ""}:${warning.message}`)}
        <li class="warning-item sev-{warning.severity}">
          <strong>{warning.message}</strong>
          {#if warning.hint && !compact}
            <span class="hint">{warning.hint}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{:else if groups.length}
  <ul class="warning-list" class:compact>
    {#each groups as group (group.code)}
      <li class="warning-item sev-{group.severity}" class:grouped={group.count > 1}>
        {#if group.count === 1}
          <strong>{group.items[0].message}</strong>
          {#if group.items[0].hint && !compact}
            <span class="hint">{group.items[0].hint}</span>
          {/if}
        {:else}
          <button
            class="warning-group-toggle"
            type="button"
            onclick={() => toggle(group.code)}
            aria-expanded={Boolean(openCodes[group.code])}
          >
            <strong>{group.code} ×{group.count}</strong>
            <span class="chevron" class:open={Boolean(openCodes[group.code])}></span>
          </button>
          {#if openCodes[group.code]}
            <ul class="warning-sublist">
              {#each group.items as warning (`${warning.code}:${warning.port ?? ""}:${warning.partId ?? ""}:${warning.message}`)}
                <li>
                  <strong>{warning.message}</strong>
                  {#if warning.hint && !compact}
                    <span class="hint">{warning.hint}</span>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
{/if}
