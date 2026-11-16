<script lang="ts">
  import { onMount } from "svelte";
  import { dash, visibleWarnings } from "$lib/dashboard.svelte";
  import { t } from "$lib/i18n.svelte";
  import { dedupeWarnings, warningSummaryLabel } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import Icon from "./Icon.svelte";
  import WarningList from "./WarningList.svelte";

  const STORAGE_KEY = "orbitry.alertsOpen";

  let open = $state(false);
  let root: HTMLDivElement | undefined;

  const warnings = $derived(visibleWarnings(dedupeWarnings([...(runState.warnings ?? []), ...(dash.warnings ?? [])])));
  const danger = $derived(warnings.some((item) => item.severity === "error"));

  function loadOpen() {
    try {
      open = localStorage.getItem(STORAGE_KEY) === "1";
    } catch {
      open = false;
    }
  }

  function setOpen(next: boolean) {
    open = next;
    try {
      localStorage.setItem(STORAGE_KEY, open ? "1" : "0");
    } catch {
      // Private mode can block storage.
    }
  }

  function toggle() {
    setOpen(!open);
  }

  function onPointerDown(event: PointerEvent) {
    if (!open) return;
    const target = event.target;
    if (target instanceof Node && root?.contains(target)) return;
    setOpen(false);
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      setOpen(false);
    }
  }

  onMount(() => {
    loadOpen();
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
      <span class="chevron" class:open></span>
    </button>
    {#if open}
      <div class="alert-popover" role="dialog" aria-label={t("alerts.title")}>
        <WarningList {warnings} flat />
      </div>
    {/if}
  </div>
{/if}
