<script lang="ts">
  import { onMount } from "svelte";
  import { afterNavigate } from "$app/navigation";
  import "../app.css";
  import ControlPanel from "$lib/components/ControlPanel.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { bannerMessage, dash, refreshAll } from "$lib/dashboard.svelte";
  import { findPart } from "$lib/format";
  import { closePanel, refreshRuns, runState, setPanelOpen } from "$lib/runs.svelte";

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
    const timer = setInterval(() => {
      refreshRuns().catch(() => {
        // Keep the last known process list if a poll fails.
      });
    }, 2000);
    window.addEventListener("keydown", onKey);
    return () => {
      clearInterval(timer);
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
      <button
        class="icon-btn ghost"
        type="button"
        disabled={dash.refreshing || dash.loading}
        onclick={() => refreshAll()}
        title="Refresh"
      >
        <Icon name="refresh" />
        <span class="btn-label">{dash.refreshing ? "Refreshing..." : "Refresh"}</span>
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
