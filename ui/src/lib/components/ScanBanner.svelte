<script lang="ts">
  import { changesVisible, dash, dismissChanges, visibleScanDiff } from "$lib/dashboard.svelte";
  import { changeLabel, changeSummary } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  const diff = $derived(visibleScanDiff(dash.scan?.diff));
  const visible = $derived(changesVisible());
</script>

{#if visible && diff}
  <div class="change-banner" role="status">
    <div class="change-copy">
      <strong>{t("scan.changes")}</strong>
      <span>{changeSummary(diff)}</span>
      {#if diff.removed.length}
        <span class="change-removed">
          {t("scan.removed")}: {diff.removed.map(changeLabel).join(", ")}
        </span>
      {/if}
    </div>
    <button class="icon-btn ghost" type="button" title={t("common.close")} onclick={dismissChanges}>
      <Icon name="close" size={14} />
    </button>
  </div>
{/if}
