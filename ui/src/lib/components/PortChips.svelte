<script lang="ts">
  import type { HealthInfo, PortInfo } from "$lib/types";
  import { portUrl } from "$lib/format";

  let { ports, health = null }: { ports: PortInfo[]; health?: HealthInfo | null } = $props();

  function statusOf(port: number): string {
    const item = health?.ports?.find((entry) => entry.port === port);
    return item?.status ?? health?.status ?? "unknown";
  }
</script>

{#if ports.length === 0}
  <span class="muted">—</span>
{:else}
  <div class="port-chips">
    {#each ports as item (item.port)}
      {@const status = statusOf(item.port)}
      <a
        class="port-chip health-{status}"
        href={portUrl(item.port)}
        target="_blank"
        rel="noopener noreferrer"
        title={item.name ? `${item.name} :${item.port}` : `:${item.port}`}
      >
        <span class="status-dot" class:on={status === "ok"} class:fail={status === "fail"}></span>
        {#if item.name}<em>{item.name}</em>{/if}
        :{item.port}
      </a>
    {/each}
  </div>
{/if}
