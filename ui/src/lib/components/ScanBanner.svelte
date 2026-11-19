<script lang="ts">
  import { changesVisible, dismissChanges } from "$lib/dashboard.svelte";
  import { t } from "$lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  const visible = $derived(changesVisible());
</script>

{#if visible}
  <div class="change-banner" role="status">
    <div class="change-copy">
      <strong>{t("scan.done")}</strong>
    </div>
    <button class="icon-btn ghost" type="button" title={t("common.close")} onclick={dismissChanges}>
      <Icon name="close" size={14} />
    </button>
  </div>
{/if}
