<script lang="ts">
  import { onMount } from "svelte";
  import { afterNavigate } from "$app/navigation";
  import "../app.css";
  import AlertStrip from "$lib/components/AlertStrip.svelte";
  import ControlPanel from "$lib/components/ControlPanel.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ScanBanner from "$lib/components/ScanBanner.svelte";
  import AutostartToast from "$lib/components/AutostartToast.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import UpdateOverlay from "$lib/components/UpdateOverlay.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { bannerMessage, dash, refreshAll } from "$lib/dashboard.svelte";
  import { refreshModels } from "$lib/models.svelte";
  import { refreshTools } from "$lib/tools.svelte";
  import { initLocale } from "$lib/i18n.svelte";
  import { migrateClientStorage } from "$lib/storage";
  import { refreshVersion, startUpdatePolling, update, updateInProgress } from "$lib/update.svelte";
  import { formatAgeMs } from "$lib/format";
  import { findPart } from "$lib/format";
  import { closePanel, refreshRuns, runState, setPanelOpen } from "$lib/runs.svelte";
  import { refreshMetrics } from "$lib/metrics.svelte";

  let { children } = $props();
  let sidebarCollapsed = $state(false);
  let mobileNav = $state(false);

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
    if (event.key === "Escape") {
      if (mobileNav) {
        mobileNav = false;
        return;
      }
      if (runState.panelOpen) closePanel();
    }
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

  afterNavigate(() => {
    mobileNav = false;
  });
</script>

<div
  class="app-shell"
  class:sidebar-collapsed={sidebarCollapsed}
  class:panel-open={runState.panelOpen}
  class:mobile-nav={mobileNav}
>
  {#if mobileNav}
    <button class="nav-scrim" type="button" aria-label="Close navigation" onclick={() => (mobileNav = false)}></button>
  {/if}

  <Sidebar collapsed={sidebarCollapsed} ontoggle={toggleSidebar} />

  <div class="main-column">
    <header class="chrome">
      <button class="icon-btn ghost menu-btn" type="button" onclick={() => (mobileNav = !mobileNav)} aria-label="Open navigation">
        <Icon name="menu" />
      </button>
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
            Cached · scanned {formatAgeMs(dash.scan.scannedAt)} · verifying…
          {:else if dash.scan.state === "scanning"}
            Scanning…
          {:else if dash.scan.state === "cached" && dash.scan.scannedAt}
            Cached · scanned {formatAgeMs(dash.scan.scannedAt)}
          {:else if dash.scan.error}
            Scan failed
          {/if}
        </div>
      {/if}
      <AlertStrip />
      <button
        class="icon-btn ghost"
        type="button"
        disabled={dash.refreshing || dash.loading}
        onclick={() => refreshAll(true)}
        title="Refresh"
      >
        <Icon name="refresh" />
        <span class="btn-label">{dash.refreshing || dash.scan?.state === "scanning" ? "Verifying..." : "Refresh"}</span>
      </button>
      <button
        class="icon-btn ghost"
        type="button"
        class:active={runState.panelOpen}
        onclick={() => setPanelOpen(!runState.panelOpen)}
        title={runState.panelOpen ? "Hide control panel" : "Show control panel"}
        aria-pressed={runState.panelOpen}
      >
        <Icon name="panel" />
      </button>
    </header>

    {#if bannerMessage()}
      <div class="banner">{bannerMessage()}</div>
    {/if}

    <ScanBanner />
    <div class="toast-stack">
      <UpdateBanner />
      <AutostartToast />
    </div>
    {#if updateInProgress()}
      <UpdateOverlay />
    {/if}

    <div class="workspace">
      <main class="list-pane">
        {@render children()}
      </main>
      {#if runState.panelOpen}
        <ControlPanel part={selectedPart} />
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
