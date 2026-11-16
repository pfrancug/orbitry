<script lang="ts">
  import type { HealthInfo, PortInfo } from "$lib/types";
  import { isExpectedSource, portUrl } from "$lib/format";

  let {
    ports = [],
    health = null,
    expected = false,
  }: { ports?: PortInfo[] | null; health?: HealthInfo | null; expected?: boolean } = $props();

  function statusOf(item: PortInfo): string {
    if (expected || isExpectedSource(item.source)) return "expected";
    const entry = health?.ports?.find((value) => value.port === item.port);
    return entry?.status ?? (health && item.source === "process" ? health.status : "unknown");
  }

  function hrefOf(item: PortInfo): string {
    return item.url || portUrl(item.port);
  }

  function titleOf(item: PortInfo, status: string): string {
    const label = item.name ? `${item.name} :${item.port}` : `:${item.port}`;
    const binds = item.binds?.length ? ` · ${item.binds.join(", ")}` : "";
    const source = item.source === "bridge" ? " · config bridge" : item.source && item.source !== "process" ? ` · ${item.source}` : "";
    if (status === "expected") return `${label} (expected)${source}${binds}`;
    return `${label}${source}${binds}`;
  }
</script>

{#if !ports?.length}
  <span class="muted">-</span>
{:else}
  <div class="port-chips">
    {#each ports ?? [] as item (item.port)}
      {@const status = statusOf(item)}
      {#if status === "expected"}
        <span class="port-chip expected" title={titleOf(item, status)}>
          {#if item.name}<em>{item.name}</em>{/if}
          :{item.port}{#if item.source === "bridge"}<i class="bridge-mark" title="From config bridge">cfg</i>{/if}
        </span>
      {:else}
        <a
          class="port-chip health-{status}"
          href={hrefOf(item)}
          target="_blank"
          rel="noopener noreferrer"
          title={titleOf(item, status)}
        >
          <span class="status-dot" class:on={status === "ok"} class:fail={status === "fail"}></span>
          {#if item.name}<em>{item.name}</em>{/if}
          :{item.port}{#if item.source === "bridge"}<i class="bridge-mark" title="From config bridge">cfg</i>{/if}
        </a>
      {/if}
    {/each}
  </div>
{/if}
