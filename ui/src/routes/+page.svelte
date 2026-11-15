<script lang="ts">
  import { onMount } from "svelte";
  import { fetchConfig, fetchProjects, saveConfig } from "$lib/api";
  import ControlPanel from "$lib/components/ControlPanel.svelte";
  import ProjectRow from "$lib/components/ProjectRow.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import { loadExpanded, toggleExpanded } from "$lib/expand";
  import { groupSearchText } from "$lib/format";
  import { openRun, refreshRuns, restoreSelection, runState } from "$lib/runs.svelte";
  import type { AppConfig, RepoGroup as RepoGroupType, ScriptPattern } from "$lib/types";

  let groups = $state<RepoGroupType[]>([]);
  let config = $state<AppConfig | null>(null);
  let query = $state("");
  let rootDraft = $state("");
  let settingsOpen = $state(false);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let refreshing = $state(false);
  let expanded = $state<string[]>([]);

  const visible = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return groups;
    return groups
      .map((group) => {
        const groupHit = [group.name, group.folder, group.path, group.git?.branch ?? ""]
          .join(" ")
          .toLowerCase()
          .includes(needle);
        if (groupHit || groupSearchText(group).includes(needle)) {
          if (groupHit) return group;
          const parts = group.parts.filter((part) =>
            [
              part.name,
              part.path,
              part.relativePath,
              part.framework ?? "",
              part.role,
              ...part.scripts.map((script) => script.name),
            ]
              .join(" ")
              .toLowerCase()
              .includes(needle),
          );
          return parts.length ? { ...group, parts } : null;
        }
        return null;
      })
      .filter((group): group is RepoGroupType => group !== null);
  });

  const allParts = $derived(groups.flatMap((group) => group.parts));
  const partCount = $derived(groups.reduce((sum, group) => sum + group.parts.length, 0));
  const runningCount = $derived(runState.runs.filter((run) => run.status === "running").length);
  const errorCount = $derived(
    runState.runs.filter((run) => run.status === "failed").length +
      runState.runs.filter((run) => run.status === "running" && run.health?.status === "fail").length,
  );
  const selectedPart = $derived(
    allParts.find((part) => part.id === runState.selectedPartId) ?? null,
  );

  function isOpen(group: RepoGroupType): boolean {
    if (query.trim()) return true;
    return expanded.includes(group.id);
  }

  async function refresh() {
    const [nextConfig, nextProjects] = await Promise.all([fetchConfig(), fetchProjects()]);
    config = nextConfig;
    groups = nextProjects.groups;
    restoreSelection(nextProjects.groups.flatMap((group) => group.parts));
  }

  async function refreshAll() {
    refreshing = true;
    error = null;
    try {
      await Promise.all([refresh(), refreshRuns()]);
    } catch (err: unknown) {
      error = err instanceof Error ? err.message : "Could not reach Orbitry.";
    } finally {
      refreshing = false;
    }
  }

  onMount(() => {
    expanded = loadExpanded();
    const wanted = new URLSearchParams(location.search).get("run");
    refresh()
      .then(() => refreshRuns())
      .then(() => {
        if (wanted) openRun(wanted);
      })
      .catch((err: unknown) => {
        error = err instanceof Error ? err.message : "Could not reach Orbitry.";
      })
      .finally(() => {
        loading = false;
      });
    const timer = setInterval(() => {
      refreshRuns().catch(() => {
        // Keep the last known process list if a poll fails.
      });
    }, 2000);
    return () => clearInterval(timer);
  });

  async function persist(next: { roots: string[]; scriptPatterns: ScriptPattern[] }) {
    saving = true;
    error = null;
    try {
      config = await saveConfig(next);
      const { groups: nextGroups } = await fetchProjects();
      groups = nextGroups;
    } catch (err: unknown) {
      error = err instanceof Error ? err.message : "Could not save settings.";
    } finally {
      saving = false;
    }
  }

  async function addRoot(event: Event) {
    event.preventDefault();
    const root = rootDraft.trim();
    if (!root || !config) return;
    if (config.roots.some((existing) => existing.toLowerCase() === root.toLowerCase())) {
      rootDraft = "";
      return;
    }
    await persist({ roots: [...config.roots, root], scriptPatterns: config.scriptPatterns });
    rootDraft = "";
  }

  async function removeRoot(root: string) {
    if (!config) return;
    await persist({
      roots: config.roots.filter((item) => item !== root),
      scriptPatterns: config.scriptPatterns,
    });
  }

  async function addPattern(pattern: ScriptPattern) {
    if (!config) return;
    const exists = config.scriptPatterns.some(
      (item) =>
        item.kind === pattern.kind &&
        item.scope === pattern.scope &&
        item.pattern.toLowerCase() === pattern.pattern.toLowerCase(),
    );
    if (exists) return;
    await persist({
      roots: config.roots,
      scriptPatterns: [...config.scriptPatterns, pattern],
    });
  }

  async function removePattern(index: number) {
    if (!config) return;
    await persist({
      roots: config.roots,
      scriptPatterns: config.scriptPatterns.filter((_, i) => i !== index),
    });
  }
</script>

<div class="shell" class:panel-open={runState.panelOpen}>
  <div class="glow glow-a"></div>
  <div class="glow glow-b"></div>

  <header class="topbar">
    <div class="brand">
      <span class="mark" aria-hidden="true"></span>
      <div>
        <p class="logo">Orbitry</p>
        <p class="tagline">All your apps. One orbit.</p>
      </div>
    </div>

    <div class="stats" aria-label="Summary">
      <div class="stat">
        <b>{loading ? "—" : groups.length}</b>
        <span>Projects</span>
      </div>
      <div class="stat">
        <b>{loading ? "—" : partCount}</b>
        <span>Apps</span>
      </div>
      <div class="stat" class:live={runningCount > 0}>
        <b>{runningCount}</b>
        <span>Running</span>
      </div>
      <div class="stat" class:alert={errorCount > 0}>
        <b>{errorCount}</b>
        <span>Errors</span>
      </div>
    </div>

    <label class="search">
      <span class="sr-only">Search projects</span>
      <input bind:value={query} placeholder="Search projects, apps, branches..." />
    </label>

    <div class="top-actions">
      <button class="icon-btn" type="button" disabled={refreshing || loading} onclick={refreshAll}>
        {refreshing ? "Refreshing..." : "Refresh"}
      </button>
      <button class="icon-btn" type="button" onclick={() => (settingsOpen = true)}>Settings</button>
    </div>
  </header>

  {#if error || runState.error}
    <div class="banner">{error || runState.error}</div>
  {/if}

  <div class="workspace">
    <main class="list-pane">
      {#if loading}
        <div class="project-list">
          {#each Array(8) as _, index (index)}
            <div class="project skeleton"></div>
          {/each}
        </div>
      {:else if visible.length === 0}
        <div class="empty">
          <strong>{groups.length === 0 ? "Nothing in orbit yet." : "No matches."}</strong>
          <p>
            {groups.length === 0
              ? "Open Settings and add a folder of projects."
              : "Try a different name, branch, or script."}
          </p>
        </div>
      {:else}
        <div class="project-list">
          {#each visible as group (group.id)}
            <ProjectRow
              {group}
              open={isOpen(group)}
              ontoggle={() => (expanded = toggleExpanded(expanded, group.id))}
            />
          {/each}
        </div>
      {/if}
    </main>

    <ControlPanel part={selectedPart} />
  </div>

  {#if settingsOpen && config}
    <SettingsPanel
      {config}
      {saving}
      bind:rootDraft
      onclose={() => (settingsOpen = false)}
      onadd={addPattern}
      onremove={removePattern}
      onaddroot={addRoot}
      onremoveroot={removeRoot}
    />
  {/if}
</div>
