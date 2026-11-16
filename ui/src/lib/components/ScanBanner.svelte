<script lang="ts">
  import { changesVisible, dash, dismissChanges } from "$lib/dashboard.svelte";
  import { changeLabel, changeSummary, normalizeScanDiff } from "$lib/format";
  import Icon from "./Icon.svelte";

  const diff = $derived(normalizeScanDiff(dash.scan?.diff));
  const visible = $derived(changesVisible());
</script>

{#if visible && diff}
  <div class="change-banner" role="status">
    <div class="change-copy">
      <strong>Changes since last scan</strong>
      <span>{changeSummary(diff)}</span>
      {#if diff.removed.length}
        <span class="change-removed">
          Removed: {diff.removed.map(changeLabel).join(", ")}
        </span>
      {/if}
    </div>
    <button class="icon-btn ghost" type="button" title="Dismiss" onclick={dismissChanges}>
      <Icon name="close" size={14} />
    </button>
  </div>
{/if}
