<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import "../app.css";
  import AlertStrip from "$lib/components/AlertStrip.svelte";
  import ControlPanel from "$lib/components/ControlPanel.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ScanBanner from "$lib/components/ScanBanner.svelte";
  import AutostartToast from "$lib/components/AutostartToast.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import UpdateOverlay from "$lib/components/UpdateOverlay.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { bannerMessage, dash, dismissBanner, refreshAll } from "$lib/dashboard.svelte";
  import { refreshModels } from "$lib/models.svelte";
  import { refreshTools } from "$lib/tools.svelte";
  import { initLocale, t } from "$lib/i18n.svelte";
  import { migrateClientStorage } from "$lib/storage";
  import { refreshVersion, startUpdatePolling, update, updateInProgress } from "$lib/update.svelte";
  import { findPart } from "$lib/format";
  import { closePanel, refreshRuns, runState } from "$lib/runs.svelte";
  import { refreshMetrics } from "$lib/metrics.svelte";

  let { children } = $props();
  let sidebarCollapsed = $state(false);

  const selectedPart = $derived(
    runState.selectedPartId ? findPart(runState.selectedPartId, dash.groups) ?? null : null,
  );

  function loadSidebar() {
    try {
      sidebarCollapsed = localStorage.getItem("orbitry.sidebarCollapsed") === "1";
    } catch {
      sidebarCollapsed = false;
    }
  }

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    try {
      localStorage.setItem("orbitry.sidebarCollapsed", sidebarCollapsed ? "1" : "0");
    } catch {
      // Private mode can block storage.
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape" && runState.panelOpen) closePanel();
  }

  function scanAge(ms: number) {
    const seconds = Math.max(0, Math.floor((Date.now() - ms) / 1000));
    if (seconds < 60) return t("chrome.justNow");
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return t("chrome.ageMinutes", { count: minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t("chrome.ageHours", { count: hours });
    const days = Math.floor(hours / 24);
    if (days < 14) return t("chrome.ageDays", { count: days });
    return new Date(ms).toLocaleDateString();
  }

  function panelFly(node: Element) {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    return fly(node, reduce ? { duration: 0, x: 0, y: 0 } : { duration: 200, x: 32, y: 0 });
  }

  onMount(() => {
    loadSidebar();
    refreshAll()
      .catch(() => {
        // refreshAll already stores the error.
      })
      .finally(() => {
        dash.loading = false;
      });
    migrateClientStorage();
    initLocale();
    void refreshVersion();
    startUpdatePolling();
    void refreshTools();
    void refreshModels();
    void refreshMetrics();
    const timer = setInterval(() => {
      refreshRuns().catch(() => {
        // Keep the last known process list if a poll fails.
      });
      void refreshMetrics();
    }, 2000);
    const versionTimer = setInterval(() => {
      void refreshVersion();
    }, 60000);
    window.addEventListener("keydown", onKey);
    return () => {
      clearInterval(timer);
      clearInterval(versionTimer);
      window.removeEventListener("keydown", onKey);
    };
  });

</script>

<div
  class="app-shell"
  class:sidebar-collapsed={sidebarCollapsed}
  class:panel-open={runState.panelOpen}
>
  <Sidebar collapsed={sidebarCollapsed} ontoggle={toggleSidebar} />

  <div class="main-column">
    <header class="chrome">
      <div class="chrome-spacer"></div>
      {#if dash.scan}
        <div
          class="scan-pill"
          class:scanning={dash.scan.state === "scanning"}
          class:stale={dash.scan.stale}
          title={dash.scan.error ?? ""}
        >
          <span class="scan-dot" aria-hidden="true"></span>
          {#if dash.scan.state === "scanning" && dash.scan.stale && dash.scan.scannedAt}
            {t("chrome.cachedVerifying", { time: scanAge(dash.scan.scannedAt) })}
          {:else if dash.scan.state === "scanning"}
            {t("chrome.scanning")}
          {:else if dash.scan.state === "cached" && dash.scan.scannedAt}
            {t("chrome.cachedScanned", { time: scanAge(dash.scan.scannedAt) })}
          {:else if dash.scan.error}
            {t("chrome.scanFailed")}
          {/if}
        </div>
      {/if}
      <AlertStrip />
      <button
        class="icon-btn ghost"
        type="button"
        disabled={dash.refreshing || dash.loading}
        onclick={() => refreshAll(true)}
        title={t("chrome.refresh")}
      >
        <Icon name="refresh" />
        <span class="btn-label">{dash.refreshing || dash.scan?.state === "scanning" ? t("chrome.verifying") : t("chrome.refresh")}</span>
      </button>
    </header>

    <div class="notice-layer">
      {#if bannerMessage()}
        <div class="banner" role="status">
          <span>{bannerMessage()}</span>
          <button class="icon-btn ghost" type="button" title={t("common.close")} onclick={dismissBanner}>
            <Icon name="close" size={14} />
          </button>
        </div>
      {/if}
      <ScanBanner />
    </div>
    <div class="toast-stack">
      <UpdateBanner />
      <AutostartToast />
    </div>
    {#if updateInProgress()}
      <UpdateOverlay />
    {/if}

    <div class="workspace">
      <main class="list-pane" data-main-scroll>
        {@render children()}
      </main>
      {#if runState.panelOpen}
        <div class="control-panel-wrap" transition:panelFly>
          <ControlPanel part={selectedPart} />
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .toast-stack {
    position: fixed;
    right: 1.1rem;
    bottom: 1.1rem;
    z-index: 60;
    display: flex;
    flex-direction: column-reverse;
    gap: 0.65rem;
    pointer-events: none;
    width: min(26rem, calc(100vw - 2rem));
  }
  @media (max-width: 720px) {
    .toast-stack {
      left: 1rem;
      right: 1rem;
      width: auto;
    }
  }
</style>
