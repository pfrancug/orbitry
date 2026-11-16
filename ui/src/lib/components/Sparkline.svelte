<script lang="ts">
  let { values = [], color = "#818cf8", height = 14 }: { values?: number[]; color?: string; height?: number } = $props();

  const W = 300;
  const pts = $derived.by(() => {
    const n = values.length;
    const top = Math.max(100, ...values, 1e-9);
    return values.map((value, i) => {
      const x = n <= 1 ? W : (i / (n - 1)) * W;
      const y = height - (Math.max(0, Math.min(value, top)) / top) * (height - 2) - 1;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
  });
</script>

<svg class="sparkline" viewBox="0 0 {W} {height}" preserveAspectRatio="none" aria-hidden="true" style="height: {height}px">
  {#if values.length > 1}
    <polygon fill={color} fill-opacity="0.14" points="0,{height} {pts.join(' ')} {W},{height}"></polygon>
    <polyline fill="none" points={pts.join(" ")} stroke={color} stroke-linejoin="round" stroke-width="1.6" vector-effect="non-scaling-stroke"></polyline>
  {:else}
    <line stroke="rgba(255,255,255,0.12)" stroke-dasharray="4 4" x1="0" x2={W} y1={height - 1} y2={height - 1}></line>
  {/if}
</svg>

<style>
  .sparkline {
    display: block;
    width: 100%;
  }
</style>
