<script lang="ts">
  import { fmtBytes, fmtPct } from "$lib/format";
  import type { RunMetrics } from "$lib/types";

  let { usage, compact = false }: { usage: RunMetrics | null | undefined; compact?: boolean } = $props();

  const cpu = $derived(usage?.cpuPct ?? null);
  const mem = $derived(usage?.memBytes ?? null);
  const title = $derived(
    usage
      ? `CPU ${fmtPct(usage.cpuPct, 1)} · RAM ${fmtBytes(usage.memBytes)}${usage.pidCount ? ` · ${usage.pidCount} proc.` : ""}`
      : "Brak danych o zasobach",
  );
</script>

{#if usage}
  <span class="res-chip" class:compact {title}>
    <span class="res-item cpu">
      <i style="width: {Math.min(100, Math.max(0, cpu ?? 0))}%"></i>
      <em>CPU</em> {fmtPct(cpu)}
    </span>
    <span class="res-item ram">
      <em>RAM</em> {fmtBytes(mem)}
    </span>
  </span>
{/if}

<style>
  .res-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    white-space: nowrap;
  }
  .res-item {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    position: relative;
  }
  .res-item em {
    font-style: normal;
    color: var(--muted);
    font-weight: 600;
    font-size: 0.64rem;
  }
  .res-item.cpu i {
    display: inline-block;
    width: 0;
    max-width: 2.2rem;
    height: 3px;
    border-radius: 99px;
    background: #818cf8;
    margin-right: 0.15rem;
  }
  .compact { font-size: 0.68rem; }
</style>
