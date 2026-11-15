<script lang="ts">
  import { slide } from "svelte/transition";
  import { isJsLike, shortPath } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import type { RepoGroup as RepoGroupType } from "$lib/types";
  import AppRow from "./AppRow.svelte";

  let {
    group,
    open,
    ontoggle,
  }: {
    group: RepoGroupType;
    open: boolean;
    ontoggle: () => void;
  } = $props();

  const running = $derived(
    runState.runs.filter(
      (run) => run.status === "running" && (group.parts ?? []).some((part) => part.id === run.partId),
    ),
  );
  const failed = $derived(
    (group.parts ?? []).some((part) =>
      runState.runs.some((run) => run.status === "failed" && run.partId === part.id),
    ),
  );
  const needsInstall = $derived(
    (group.parts ?? []).some((item) => isJsLike(item.kind) && !item.nodeModules),
  );
  const versions = $derived(
    [...new Set((group.parts ?? []).map((part) => part.version).filter((value): value is string => Boolean(value)))],
  );
</script>

<section class="project" class:open class:live={running.length > 0}>
  <button class="project-row" type="button" onclick={ontoggle} aria-expanded={open}>
    <span class="chevron" class:open aria-hidden="true"></span>
    <div class="project-id">
      <div class="title-row">
        <h2 title={group.name}>{group.name}</h2>
        {#if running.length > 0}
          <span class="pulse" title="Running" aria-label="Running"></span>
          <span class="run-count">{running.length}</span>
        {:else if failed}
          <span class="dot fail" title="A process failed"></span>
        {/if}
      </div>
      <p class="path" title={group.path}>{shortPath(group.path)}</p>
    </div>
    <div class="project-meta">
      {#if group.git}
        <span class="git" title={group.git.dirty ? "Uncommitted changes" : "Clean working tree"}>
          <span class="dot {group.git.dirty ? 'dirty' : 'clean'}"></span>
          {group.git.branch}
        </span>
      {:else}
        <span class="muted">no git</span>
      {/if}
      {#if versions[0]}
        <span class="badge quiet">v{versions[0]}</span>
      {/if}
      <span class="count">{(group.parts ?? []).length === 1 ? "1 app" : `${(group.parts ?? []).length} apps`}</span>
      {#if needsInstall && !open}
        <span class="warn-inline">deps</span>
      {/if}
    </div>
  </button>

  {#if open}
    <div class="app-list" transition:slide={{ duration: 160 }}>
      {#each (group.parts ?? []) as part (part.id)}
        <AppRow {part} />
      {/each}
    </div>
  {/if}
</section>
