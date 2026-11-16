<script lang="ts">
  import { onMount } from "svelte";
  import { dash } from "$lib/dashboard.svelte";
  import { dedupeWarnings, warningSummaryLabel } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import Icon from "./Icon.svelte";
  import WarningList from "./WarningList.svelte";

  const STORAGE_KEY = "orbitry.alertsOpen";

  let open = $state(false);

  const warnings = $derived(dedupeWarnings([...(runState.warnings ?? []), ...(dash.warnings ?? [])]));
  const danger = $derived(warnings.some((item) => item.severity === "error"));

  function loadOpen() {
    try {
      open = localStorage.getItem(STORAGE_KEY) === "1";
    } catch {
      open = false;
    }
  }

  function toggle() {
    open = !open;
    try {
      localStorage.setItem(STORAGE_KEY, open ? "1" : "0");
    } catch {
      // Private mode can block storage.
    }
  }

  onMount(loadOpen);
</script>

{#if warnings.length}
  <div class="alert-strip" class:danger class:warn={!danger}>
    <button class="alert-toggle" type="button" onclick={toggle} aria-expanded={open}>
      <Icon name="alert" size={14} />
      <span>{warningSummaryLabel(warnings)}</span>
      <span class="chevron" class:open></span>
    </button>
    {#if open}
      <div class="alert-body">
        <WarningList {warnings} />
      </div>
    {/if}
  </div>
{/if}
