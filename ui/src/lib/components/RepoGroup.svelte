<script lang="ts">
  import { slide } from "svelte/transition";
  import { isJsLike, partTitle, shortPath } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import type { RepoGroup as RepoGroupType } from "$lib/types";
  import GitBuild from "./GitBuild.svelte";
  import ScriptChips from "./ScriptChips.svelte";

  let { group, index = 0 }: { group: RepoGroupType; index?: number } = $props();

  let open = $state(true);
  const needsInstall = $derived(
    group.parts.some((item) => isJsLike(item.kind) && !item.nodeModules),
  );
  const partLabel = $derived(group.parts.length === 1 ? "1 part" : `${group.parts.length} parts`);
  const groupLive = $derived(
    group.parts.some((part) =>
      runState.runs.some((run) => run.status === "running" && run.partId === part.id),
    ),
  );

  function toggle() {
    open = !open;
  }
</script>

<section class="stack" style="animation-delay: {Math.min(index, 12) * 30}ms">
  <button class="stack-head" type="button" onclick={toggle} aria-expanded={open}>
    <div class="stack-title">
      <span class="chevron" class:open aria-hidden="true"></span>
      <div>
        <div class="title-row">
          <h2 title={group.name}>{group.name}</h2>
          {#if groupLive}<span class="pulse" title="Running" aria-label="Running"></span>{/if}
        </div>
        <p class="path" title={group.path}>
          {shortPath(group.path)}
          <span class="count">· {partLabel}</span>
        </p>
      </div>
    </div>
    <div class="stack-meta">
      <div class="badges">
        {#if group.version}<span class="badge quiet">v{group.version}</span>{/if}
        {#if group.framework}<span class="badge framework">{group.framework}</span>{/if}
      </div>
      <GitBuild git={group.git} build={group.build} />
    </div>
  </button>

  {#if open}
    <div class="parts" transition:slide={{ duration: 220 }}>
      {#each group.parts as item (item.id)}
        <article class="part">
          <div class="part-head">
            <div>
              <div class="part-name">
                <span class="badge role role-{item.role}">{item.role}</span>
                <h3 title={item.name}>{partTitle(item)}</h3>
                {#if runState.runs.some((run) => run.status === "running" && run.partId === item.id)}
                  <span class="pulse" title="Running" aria-label="Running"></span>
                {/if}
              </div>
              <p class="path" title={item.path}>{item.name}</p>
            </div>
            <div class="badges">
              {#if item.version && item.version !== group.version}
                <span class="badge quiet">v{item.version}</span>
              {/if}
              {#if item.framework}
                <span class="badge framework">{item.framework}</span>
              {/if}
            </div>
          </div>
          <ScriptChips partId={item.id} scripts={item.scripts} />
          <div class="part-foot">
            <span class="build" class:ok={item.build.built}>
              {item.build.built ? "Built" : "Not built"}
            </span>
            {#if isJsLike(item.kind) && !item.nodeModules}
              <span class="warn-inline">Dependencies missing</span>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {/if}
  {#if needsInstall && !open}
    <p class="warn stack-warn">Some parts are missing dependencies</p>
  {/if}
</section>
