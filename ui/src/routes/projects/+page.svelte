<script lang="ts">
  import ProjectRow from "$lib/components/ProjectRow.svelte";
  import { dash } from "$lib/dashboard.svelte";
  import { loadExpanded, toggleExpanded } from "$lib/expand";
  import { groupSearchText } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import type { RepoGroup } from "$lib/types";
  import Icon from "$lib/components/Icon.svelte";

  let expanded = $state<string[]>(loadExpanded());

  const visible = $derived.by(() => {
    const needle = dash.query.trim().toLowerCase();
    if (!needle) return dash.groups;
    return dash.groups
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
      .filter((group): group is RepoGroup => group !== null);
  });

  const running = $derived(runState.runs.filter((run) => run.status === "running").length);

  function isOpen(group: RepoGroup): boolean {
    if (dash.query.trim()) return true;
    return expanded.includes(group.id);
  }
</script>

<div class="page-head">
  <div>
    <h1>Projects</h1>
    <p>
      {#if dash.loading}
        Scanning...
      {:else}
        {visible.length}{visible.length === dash.groups.length ? "" : ` of ${dash.groups.length}`} projects
        · {dash.groups.reduce((sum, group) => sum + group.parts.length, 0)} apps
        {#if running > 0}· {running} running{/if}
      {/if}
    </p>
  </div>
  <label class="search">
    <Icon name="search" size={15} />
    <span class="sr-only">Search projects</span>
    <input bind:value={dash.query} placeholder="Search projects, apps, branches..." />
  </label>
</div>

{#if dash.loading}
  <div class="project-list">
    {#each Array(8) as _, index (index)}
      <div class="project skeleton"></div>
    {/each}
  </div>
{:else if visible.length === 0}
  <div class="empty-card">
    <strong>{dash.groups.length === 0 ? "Nothing in orbit yet." : "No matches."}</strong>
    <p>
      {dash.groups.length === 0
        ? "Open Settings and add a folder of projects."
        : "Try a different name, branch, or script."}
    </p>
    {#if dash.groups.length === 0}
      <a class="btn primary" href="/settings">Open settings</a>
    {/if}
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
