<script lang="ts">
  import type { HealthInfo, PortInfo, ProjectPart } from "$lib/types";
  import { isExpectedSource, portLabel, portUrl } from "$lib/format";

  let {
    ports = [],
    health = null,
    expected = false,
    part = null,
    script = null,
  }: {
    ports?: PortInfo[] | null;
    health?: HealthInfo | null;
    expected?: boolean;
    part?: ProjectPart | null;
    script?: string | null;
  } = $props();

  function statusOf(item: PortInfo): string {
    if (expected || isExpectedSource(item.source)) return "expected";
    const entry = health?.ports?.find((value) => value.port === item.port);
    return entry?.status ?? (health && item.source === "process" ? health.status : "unknown");
  }

  function hrefOf(item: PortInfo): string {
    return item.url || portUrl(item.port);
  }

  function titleOf(item: PortInfo, status: string): string {
    const label = `${portLabel(item, part, script)} :${item.port}`;
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
      {@const label = portLabel(item, part, script)}
      {#if status === "expected"}
        <span class="port-chip expected" title={titleOf(item, status)}>
          <em>{label}</em>
          :{item.port}{#if item.source === "bridge"}<i class="bridge-mark" title="From config bridge">cfg</i>{/if}
        </span>
      {:else}
        <a
          class="port-chip health-{status}"
          href={hrefOf(item)}
          target="_blank"
          rel="noopener noreferrer"
          title={titleOf(item, status)}
          onclick={(event) => event.stopPropagation()}
        >
          <span class="status-dot" class:on={status === "ok"} class:fail={status === "fail"}></span>
          <em>{label}</em>
          :{item.port}{#if item.source === "bridge"}<i class="bridge-mark" title="From config bridge">cfg</i>{/if}
        </a>
      {/if}
    {/each}
  </div>
{/if}
