<script lang="ts">
  import { armAutoDismiss } from "$lib/autoDismiss.svelte";
  import { changesVisible, dismissChanges } from "$lib/dashboard.svelte";
  import { t } from "$lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  const visible = $derived(changesVisible());
  let el = $state<HTMLElement | undefined>(undefined);
  $effect(() => {
    if (!visible) return;
    return armAutoDismiss(el, {
      ms: 6_000,
      dismiss: () => dismissChanges(),
    });
  });
</script>

{#if visible}
  <div class="change-banner" role="status" bind:this={el}>
    <div class="change-copy">
      <strong>{t("scan.done")}</strong>
    </div>
    <button class="icon-btn ghost" type="button" title={t("common.close")} onclick={dismissChanges}>
      <Icon name="close" size={14} />
    </button>
  </div>
{/if}
