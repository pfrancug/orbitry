<script lang="ts">
  import { onMount } from "svelte";
  import { fetchConfig, fetchProjects, saveConfig } from "$lib/api";
  import RepoGroup from "$lib/components/RepoGroup.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import { groupSearchText } from "$lib/format";
  import type { AppConfig, RepoGroup as RepoGroupType, ScriptPattern } from "$lib/types";

  let groups = $state<RepoGroupType[]>([]);
  let config = $state<AppConfig | null>(null);
  let query = $state("");
  let rootDraft = $state("");
  let settingsOpen = $state(false);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let saving = $state(false);

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
            [part.name, part.path, part.relativePath, part.framework ?? "", part.role, ...part.scripts.map((s) => s.name)]
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

  const partCount = $derived(visible.reduce((sum, group) => sum + group.parts.length, 0));

  async function refresh() {
    const [nextConfig, nextProjects] = await Promise.all([fetchConfig(), fetchProjects()]);
    config = nextConfig;
    groups = nextProjects.groups;
  }

  onMount(() => {
    refresh()
      .catch((err: unknown) => {
        error = err instanceof Error ? err.message : "Could not reach Orbitry.";
      })
      .finally(() => {
        loading = false;
      });
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

<div class="shell">
  <div class="glow glow-a"></div>
  <div class="glow glow-b"></div>

  <header class="topbar">
    <div class="brand">
      <span class="mark" aria-hidden="true">✦</span>
      <div>
        <p class="logo">Orbitry</p>
        <p class="tagline">All your apps. One orbit.</p>
      </div>
    </div>

    <label class="search">
      <span class="sr-only">Search projects</span>
      <input bind:value={query} placeholder="Search projects, branches, scripts..." />
    </label>

    <button class="icon-btn" type="button" onclick={() => (settingsOpen = true)}>Settings</button>
  </header>

  <section class="roots">
    <form class="root-form" onsubmit={addRoot}>
      <label class="sr-only" for="root-path">Project root folder</label>
      <input
        id="root-path"
        bind:value={rootDraft}
        placeholder="Add a project folder...  C:\Projects\private"
        spellcheck="false"
      />
      <button type="submit" disabled={saving || !rootDraft.trim()}>Add folder</button>
    </form>
    <div class="root-list">
      {#each config?.roots ?? [] as root (root)}
        <button class="root-chip" type="button" onclick={() => removeRoot(root)} title="Remove folder">
          <span>{root}</span>
          <i aria-hidden="true">×</i>
        </button>
      {/each}
      {#if config && config.roots.length === 0}
        <p class="hint">Point Orbitry at a folder of projects to begin.</p>
      {/if}
    </div>
  </section>

  {#if error}
    <div class="banner">{error}</div>
  {/if}

  <div class="meta">
    <p>
      {#if loading}
        Scanning...
      {:else}
        {visible.length}{visible.length === groups.length ? "" : ` of ${groups.length}`} in orbit
        · {partCount} parts
      {/if}
    </p>
  </div>

  {#if loading}
    <div class="grid">
      {#each Array(6) as _, index (index)}
        <div class="stack skeleton"></div>
      {/each}
    </div>
  {:else if visible.length === 0}
    <div class="empty">
      <strong>{groups.length === 0 ? "Nothing in orbit yet." : "No matches."}</strong>
      <p>
        {groups.length === 0
          ? "Add a folder above and Orbitry will map every app it finds."
          : "Try a different name, branch, or script."}
      </p>
    </div>
  {:else}
    <div class="grid">
      {#each visible as group, index (group.id)}
        <RepoGroup {group} {index} />
      {/each}
    </div>
  {/if}

  {#if settingsOpen && config}
    <SettingsPanel
      {config}
      {saving}
      onclose={() => (settingsOpen = false)}
      onadd={addPattern}
      onremove={removePattern}
    />
  {/if}
</div>