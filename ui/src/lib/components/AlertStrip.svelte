<script lang="ts">
  import { onMount } from "svelte";
  import { dash, visibleWarnings } from "$lib/dashboard.svelte";
  import { t } from "$lib/i18n.svelte";
  import { dedupeWarnings, warningSummaryLabel } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import Icon from "./Icon.svelte";
  import WarningList from "./WarningList.svelte";

  let open = $state(false);
  let root = $state<HTMLDivElement | undefined>(undefined);

  const warnings = $derived(visibleWarnings(dedupeWarnings([...(runState.warnings ?? []), ...(dash.warnings ?? [])])));
  const danger = $derived(warnings.some((item) => item.severity === "error"));

  function toggle() {
    open = !open;
  }

  function onPointerDown(event: PointerEvent) {
    if (!open) return;
    const target = event.target;
    if (target instanceof Node && root?.contains(target)) return;
    open = false;
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape" && open) open = false;
  }

  onMount(() => {
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

{#if warnings.length}
  <div class="alert-overlay" class:danger class:warn={!danger} bind:this={root}>
    <button class="alert-pill" type="button" onclick={toggle} aria-expanded={open} aria-haspopup="dialog">
      <Icon name="alert" size={14} />
      <span class="alert-label">{warningSummaryLabel(warnings)}</span>
      <span class="chevron" class:open aria-hidden="true"></span>
    </button>
    {#if open}
      <div class="alert-popover" role="dialog" aria-label={t("alerts.title")}>
        <div class="alert-popover-head">
          <strong>{t("alerts.title")}</strong>
          <span class="muted">{warnings.length}</span>
          <button class="icon-btn ghost" type="button" title={t("common.close")} aria-label={t("common.close")} onclick={() => (open = false)}>
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="alert-popover-list">
          <WarningList {warnings} flat actionable onAction={() => (open = false)} />
        </div>
      </div>
    {/if}
  </div>
{/if}
