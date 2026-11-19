<script lang="ts">
  import { dash, partMarker, startBlockReason, startWarnReason, visibleWarnings } from "$lib/dashboard.svelte";
  import { allScripts, displayPorts, isJsLike, partDatabases, partTitle, primaryScript, roleLabel, scriptEachKey, scriptLabel, scriptSourceDiffers, warningsForPart } from "$lib/format";
  import { i18n, t } from "$lib/i18n.svelte";
  import { isPending, runningFor, runState, togglePart, toggleScript } from "$lib/runs.svelte";
  import type { ProjectPart } from "$lib/types";
  import PortChips from "./PortChips.svelte";
  import AiBadge from "./AiBadge.svelte";
  import WarningBadge from "./WarningBadge.svelte";
  import DepsBadge from "./DepsBadge.svelte";
  import DbBadge from "./DbBadge.svelte";
  import ResourceChip from "./ResourceChip.svelte";
  import { runMetrics } from "$lib/metrics.svelte";
  import { aiSignalsForProject } from "$lib/models.svelte";

  let { part, marker = null }: { part: ProjectPart; marker?: "new" | "changed" | "removed" | null } = $props();

  const locale = $derived(i18n.locale);
  const flag = $derived(marker ?? partMarker(part.id));

  const selected = $derived(runState.selectedPartId === part.id && runState.panelOpen);
  const live = $derived(runningFor(part.id));
  const ports = $derived(displayPorts(part, runState.runs));
  const failed = $derived(
    !live && runState.runs.some((run) => run.partId === part.id && run.status === "failed"),
  );
  const warnings = $derived(
    visibleWarnings(warningsForPart(part.id, runState.runs, [...(runState.warnings ?? []), ...(dash.warnings ?? [])])),
  );
  const expected = $derived(!live);
  const scripts = $derived(allScripts(part));
  const primary = $derived(primaryScript(part));
  const activeName = $derived(live?.script ?? primary?.name ?? null);
  const activeScript = $derived(scripts.find((script) => script.name === activeName) ?? primary);
  const missingDeps = $derived(isJsLike(part.kind) && !part.nodeModules);
  const databases = $derived(partDatabases(part));
  const aiSignals = $derived(aiSignalsForProject({ part }));
  const run = $derived(activeName ? runningFor(part.id, activeName) : undefined);
  const pending = $derived(activeName ? isPending(part.id, activeName) : false);
  const blocked = $derived(activeName && !run ? startBlockReason(part.id, activeName) : null);
  const warned = $derived(activeName && !run ? startWarnReason(part.id, activeName) : null);
  let menuOpen = $state(false);
  let menuRoot = $state<HTMLDivElement | undefined>(undefined);

  $effect(() => {
    if (!menuOpen) return;
    const onPointer = (event: PointerEvent) => {
      if (menuRoot && !menuRoot.contains(event.target as Node)) menuOpen = false;
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") menuOpen = false;
    };
    document.addEventListener("pointerdown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="app-row" class:selected class:live={Boolean(live)} class:removed={flag === "removed"} data-locale={locale}>
  <span class="badge role role-{part.role}">{roleLabel(part.role)}</span>

  <button
    class="app-main"
    type="button"
    onclick={() => togglePart(part.id, live?.script ?? primary?.name)}
  >
    <div class="app-id">
      <div class="app-name">
        <span class="status-dot" class:on={Boolean(live)} class:fail={failed}></span>
        <strong title={part.name}>{partTitle(part)}</strong>
        {#if part.framework}
          <span class="muted framework-tag">{part.framework}</span>
        {/if}
        <!-- Tag order: AI → DB → deps → warning (last). -->
        <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
          <AiBadge signals={aiSignals} />
        </span>
        {#if databases.length}
          <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
            <DbBadge {part} {databases} />
          </span>
        {/if}
        {#if missingDeps}
          <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
            <DepsBadge {part} />
          </span>
        {/if}
        <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
          <WarningBadge {warnings} />
        </span>
        {#if flag === "new"}<span class="marker new">{t("common.new")}</span>{/if}
        {#if flag === "removed"}<span class="marker removed">{t("common.removed")}</span>{/if}
      </div>
    </div>
  </button>

  <span class="port-cell">
    <PortChips ports={ports} health={live?.health ?? null} {expected} {part} script={live?.script} />
  </span>

  <div class="app-status">
    {#if live}
      <span class="res-cell">
        <ResourceChip usage={runMetrics(live.id)} compact />
      </span>
    {/if}
  </div>

  <div class="app-actions">
    {#if !scripts.length}
      <span class="muted">{t("script.noScripts")}</span>
    {:else if activeScript}
      <div class="script-split" bind:this={menuRoot}>
        <button
          class="chip mini script-main scope-{activeScript.scope}"
          class:running={Boolean(run)}
          class:pending
          type="button"
          aria-pressed={Boolean(run)}
          disabled={pending || Boolean(blocked)}
          title={blocked ?? warned ?? (run ? `${t("script.stop")} ${scriptLabel(activeScript)}` : `${t("script.start")} ${scriptLabel(activeScript)}`)}
          onclick={(event) => {
            event.stopPropagation();
            toggleScript(part.id, activeScript.name, dash.groups);
          }}
        >
          {#if run}
            <span class="pulse" aria-hidden="true"></span>
          {:else}
            <span class="play-icon" aria-hidden="true"></span>
          {/if}
          {scriptLabel(activeScript)}
        </button>
        {#if scripts.length > 1}
          <button
            class="chip mini script-caret"
            class:open={menuOpen}
            type="button"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            title={t("script.more")}
            aria-label={t("script.more")}
            onclick={(event) => {
              event.stopPropagation();
              menuOpen = !menuOpen;
            }}
          >
            <span class="caret" aria-hidden="true"></span>
          </button>
          {#if menuOpen}
            <div class="script-menu" role="menu">
              {#each scripts as script (scriptEachKey(script))}
                {@const itemRun = runningFor(part.id, script.name)}
                {@const itemPending = isPending(part.id, script.name)}
                {@const itemBlocked = !itemRun ? startBlockReason(part.id, script.name) : null}
                <button
                  type="button"
                  role="menuitem"
                  class:running={Boolean(itemRun)}
                  disabled={itemPending || Boolean(itemBlocked)}
                  title={itemBlocked ?? script.command}
                  onclick={(event) => {
                    event.stopPropagation();
                    menuOpen = false;
                    toggleScript(part.id, script.name, dash.groups);
                  }}
                >
                  <span>{scriptLabel(script)}</span>
                  {#if scriptSourceDiffers(script)}
                    <em>{t("script.viaRoot", { name: script.sourceName || t("script.workspace") })}</em>
                  {/if}
                  {#if itemRun}
                    <span class="muted">{t("script.stop")}</span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .script-split {
    position: relative;
    display: inline-flex;
    align-items: stretch;
  }
  .script-main {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .script-caret {
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    border-left-width: 0;
    padding-left: 7px;
    padding-right: 7px;
  }
  .script-split:has(.script-caret) .script-main {
    border-right-color: var(--line-strong);
  }
  .caret {
    width: 0;
    height: 0;
    border-style: solid;
    border-width: 4px 3.5px 0 3.5px;
    border-color: currentColor transparent transparent transparent;
  }
  .script-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 40;
    min-width: 12rem;
    max-width: 18rem;
    padding: 4px;
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    background: rgba(16, 16, 22, 0.98);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
  }
  .script-menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    margin: 0;
    padding: 7px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .script-menu button:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .script-menu button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .script-menu em {
    font-style: normal;
    color: var(--muted);
    font-size: 11px;
  }
</style>
