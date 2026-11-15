<script lang="ts">
  import { dash } from "$lib/dashboard.svelte";
  import { displayPorts, isJsLike, partTitle, roleLabel } from "$lib/format";
  import { isPending, runningFor, runState, togglePart, toggleScript } from "$lib/runs.svelte";
  import type { ProjectPart } from "$lib/types";
  import PortChips from "./PortChips.svelte";

  let { part }: { part: ProjectPart } = $props();

  const selected = $derived(runState.selectedPartId === part.id && runState.panelOpen);
  const live = $derived(runningFor(part.id));
  const ports = $derived(displayPorts(part, runState.runs));
  const failed = $derived(
    !live && runState.runs.some((run) => run.partId === part.id && run.status === "failed"),
  );
</script>

<div class="app-row" class:selected class:live={Boolean(live)}>
  <button
    class="app-main"
    type="button"
    onclick={() => togglePart(part.id, live?.script ?? (part.scripts ?? [])[0]?.name)}
  >
    <span class="badge role role-{part.role}">{roleLabel(part.role)}</span>
    <div class="app-id">
      <div class="app-name">
        <span class="status-dot" class:on={Boolean(live)} class:fail={failed}></span>
        <strong title={part.name}>{partTitle(part)}</strong>
        {#if part.framework}
          <span class="muted framework-tag">{part.framework}</span>
        {/if}
      </div>
    </div>
    <span class="port-cell">
      <PortChips ports={ports} health={live?.health ?? null} />
    </span>
  </button>

  <div class="app-actions">
    {#if !(part.scripts ?? []).length}
      <span class="muted">no scripts</span>
    {:else}
      {#each part.scripts ?? [] as script (script.kind + script.name)}
        {@const run = runningFor(part.id, script.name)}
        {@const pending = isPending(part.id, script.name)}
        <button
          class="chip mini scope-{script.scope}"
          class:running={Boolean(run)}
          class:pending
          type="button"
          aria-pressed={Boolean(run)}
          aria-label={run ? `Stop ${script.name}` : `Start ${script.name}`}
          disabled={pending}
          onclick={(event) => {
            event.stopPropagation();
            toggleScript(part.id, script.name, dash.groups);
          }}
        >
          {#if run}
            <span class="pulse" aria-hidden="true"></span>
          {:else}
            <span class="play-icon" aria-hidden="true"></span>
          {/if}
          {script.name}
        </button>
      {/each}
    {/if}
    {#if isJsLike(part.kind) && !part.nodeModules}
      <span class="warn-inline">deps</span>
    {/if}
  </div>
</div>
