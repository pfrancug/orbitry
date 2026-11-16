<script lang="ts">
  import { fmtPct } from "$lib/format";
  import Sparkline from "./Sparkline.svelte";

  let {
    label,
    value,
    sub,
    pct,
    color,
    values = [],
  }: {
    label: string;
    value?: string;
    sub?: string;
    pct: number | null;
    color: string;
    values?: number[];
  } = $props();

  const bar = $derived(Math.min(100, Math.max(0, pct ?? 0)));
  const hot = $derived(pct != null && pct > 90);
</script>

<div class="meter">
  <div class="meter-head">
    <span class="meter-label">{label}</span>
    <strong class="meter-value">{value ?? fmtPct(pct)}</strong>
  </div>
  <div class="meter-track" aria-hidden="true">
    <div class="meter-bar" class:hot style="width: {bar}%; background: {hot ? 'var(--danger)' : color}"></div>
  </div>
  <div class="meter-spark">
    <Sparkline {color} {values} height={64} min={0} max={100} />
  </div>
  {#if sub}
    <span class="meter-sub">{sub}</span>
  {/if}
</div>

<style>
  .meter { min-width: 0; padding: 0.15rem 0 0.25rem; }
  .meter-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .meter-label {
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }
  .meter-value {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .meter-track {
    height: 8px;
    margin-top: 0.5rem;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    overflow: hidden;
  }
  .meter-bar {
    height: 100%;
    border-radius: 999px;
  }
  .meter-spark { margin-top: 0.45rem; }
  .meter-sub {
    display: block;
    margin-top: 0.15rem;
    color: var(--muted);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
</style>
