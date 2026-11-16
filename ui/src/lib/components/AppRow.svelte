<script lang="ts">
  import { appAutostartEnabled, appAutostartTitle, dash, partMarker, setAppAutostart, startBlockReason, systemAutostartEnabled } from "$lib/dashboard.svelte";
  import { displayPorts, isJsLike, partTitle, roleLabel, warningsForPart } from "$lib/format";
  import { isPending, runningFor, runState, togglePart, toggleScript } from "$lib/runs.svelte";
  import type { ProjectPart } from "$lib/types";
  import PortChips from "./PortChips.svelte";
  import WarningBadge from "./WarningBadge.svelte";
  import ResourceChip from "./ResourceChip.svelte";
  import { runMetrics } from "$lib/metrics.svelte";

  let { part, marker = null }: { part: ProjectPart; marker?: "new" | "changed" | "removed" | null } = $props();

  const flag = $derived(marker ?? partMarker(part.id));

  const selected = $derived(runState.selectedPartId === part.id && runState.panelOpen);
  const live = $derived(runningFor(part.id));
  const ports = $derived(displayPorts(part, runState.runs));
  const failed = $derived(
    !live && runState.runs.some((run) => run.partId === part.id && run.status === "failed"),
  );
  const warnings = $derived(warningsForPart(part.id, runState.runs, [...(runState.warnings ?? []), ...(dash.warnings ?? [])]));
  const expected = $derived(!live);
  const systemOn = $derived(systemAutostartEnabled());
  const autoOn = $derived(appAutostartEnabled(part.id));
  const autoTitle = $derived(appAutostartTitle());
</script>

<div class="app-row" class:selected class:live={Boolean(live)} class:removed={flag === "removed"}>
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
        <WarningBadge {warnings} />
        {#if flag === "new"}<span class="marker new">New</span>{/if}
        {#if flag === "changed"}<span class="marker changed">Changed</span>{/if}
        {#if flag === "removed"}<span class="marker removed">Removed</span>{/if}
      </div>
    </div>
    <span class="port-cell">
      <PortChips ports={ports} health={live?.health ?? null} {expected} />
    </span>
    {#if live}
      <span class="res-cell">
        <ResourceChip usage={runMetrics(live.id)} compact />
      </span>
    {/if}
  </button>

  <div class="app-actions">
    <label class="app-autostart" class:off={!systemOn} title={autoTitle}>
      <input
        type="checkbox"
        checked={autoOn}
        disabled={!systemOn || dash.saving}
        aria-label="Uruchamiaj przy starcie Orbitry"
        onclick={(event) => event.stopPropagation()}
        onchange={(event) => {
          event.stopPropagation();
          setAppAutostart(part.id, event.currentTarget.checked);
        }}
      />
      Autostart
    </label>
    {#if !(part.scripts ?? []).length}
      <span class="muted">no scripts</span>
    {:else}
      {#each part.scripts ?? [] as script (script.kind + script.name)}
        {@const run = runningFor(part.id, script.name)}
        {@const pending = isPending(part.id, script.name)}
        {@const blocked = !run ? startBlockReason(part.id, script.name) : null}
        <button
          class="chip mini scope-{script.scope}"
          class:running={Boolean(run)}
          class:pending
          type="button"
          aria-pressed={Boolean(run)}
          aria-label={run ? `Stop ${script.name}` : `Start ${script.name}`}
          disabled={pending || Boolean(blocked)}
          title={blocked ?? (run ? `Stop ${script.name}` : `Start ${script.name}`)}
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

<style>
  .app-autostart {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
    margin-right: 0.15rem;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
    cursor: pointer;
    user-select: none;
    white-space: nowrap;
  }
  .app-autostart input {
    margin: 0;
    accent-color: var(--accent);
  }
  .app-autostart.off {
    opacity: 0.55;
    cursor: not-allowed;
  }
</style>
