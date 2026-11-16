<script lang="ts">
  import { appAutostartEnabled, appAutostartTitle, dash, partMarker, setAppAutostart, startBlockReason, startWarnReason, systemAutostartEnabled } from "$lib/dashboard.svelte";
  import { buildScripts, displayPorts, installCommand, isJsLike, partTitle, roleLabel, runScripts, scriptLabel, warningsForPart } from "$lib/format";
  import { i18n, t } from "$lib/i18n";
  import { isPending, runningFor, runState, startInstall, startScript, togglePart, toggleScript } from "$lib/runs.svelte";
  import type { ProjectPart } from "$lib/types";
  import PortChips from "./PortChips.svelte";
  import WarningBadge from "./WarningBadge.svelte";
  import ResourceChip from "./ResourceChip.svelte";
  import { runMetrics } from "$lib/metrics.svelte";

  let { part, marker = null }: { part: ProjectPart; marker?: "new" | "changed" | "removed" | null } = $props();

  const locale = $derived(i18n.locale);
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
  const startScripts = $derived(runScripts(part));
  const builds = $derived(buildScripts(part));
  const missingDeps = $derived(isJsLike(part.kind) && !part.nodeModules);
  const depsCommand = $derived(installCommand(part));
  const installLive = $derived(runningFor(part.id, "install"));
  const installPending = $derived(isPending(part.id, "install"));
</script>

<div class="app-row" class:selected class:live={Boolean(live)} class:removed={flag === "removed"} data-locale={locale}>
  <button
    class="app-main"
    type="button"
    onclick={() => togglePart(part.id, live?.script ?? startScripts[0]?.name)}
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
        {#if flag === "new"}<span class="marker new">{t("common.new")}</span>{/if}
        {#if flag === "changed"}<span class="marker changed">{t("common.changed")}</span>{/if}
        {#if flag === "removed"}<span class="marker removed">{t("common.removed")}</span>{/if}
      </div>
    </div>
    {#if live}
      <span class="res-cell">
        <ResourceChip usage={runMetrics(live.id)} compact />
      </span>
    {/if}
  </button>

  <span class="port-cell">
    <PortChips ports={ports} health={live?.health ?? null} {expected} />
  </span>

  <div class="app-actions">
    <label class="app-autostart" class:off={!systemOn} title={autoTitle}>
      <input
        type="checkbox"
        checked={autoOn}
        disabled={!systemOn || dash.saving}
        aria-label={t("autostart.app")}
        onclick={(event) => event.stopPropagation()}
        onchange={(event) => {
          event.stopPropagation();
          setAppAutostart(part.id, event.currentTarget.checked);
        }}
      />
      {t("autostart.short")}
    </label>
    {#if !startScripts.length && !builds.length}
      <span class="muted">{t("script.noScripts")}</span>
    {:else}
      {#each startScripts as script (`${script.kind}:${script.name}`)}
        {@const run = runningFor(part.id, script.name)}
        {@const pending = isPending(part.id, script.name)}
        {@const blocked = !run ? startBlockReason(part.id, script.name) : null}
        {@const warned = !run ? startWarnReason(part.id, script.name) : null}
        <button
          class="chip mini scope-{script.scope}"
          class:running={Boolean(run)}
          class:pending
          type="button"
          aria-pressed={Boolean(run)}
          aria-label={run ? `${t("script.stop")} ${scriptLabel(script)}` : `${t("script.start")} ${scriptLabel(script)}`}
          disabled={pending || Boolean(blocked)}
          title={blocked ?? warned ?? (run ? `${t("script.stop")} ${scriptLabel(script)}` : `${t("script.start")} ${scriptLabel(script)}`)}
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
      {#each builds as script (`build:${script.name}`)}
        {@const run = runningFor(part.id, script.name)}
        {@const pending = isPending(part.id, script.name)}
        <button
          class="chip mini"
          class:running={Boolean(run)}
          class:pending
          type="button"
          disabled={pending}
          title={script.command}
          onclick={(event) => {
            event.stopPropagation();
            if (run) toggleScript(part.id, script.name, dash.groups);
            else startScript(part.id, script.name, dash.groups);
          }}
        >
          {run ? t("build.running") : t("build.run")}
        </button>
      {/each}
    {/if}
    {#if missingDeps}
      <span
        class="warn-inline"
        title={t("deps.tooltip", { command: depsCommand })}
      >{t("deps.badge")}</span>
      <button
        class="chip mini"
        class:running={Boolean(installLive)}
        class:pending={installPending}
        type="button"
        disabled={installPending || Boolean(installLive)}
        title={t("deps.explain", { command: depsCommand })}
        onclick={(event) => {
          event.stopPropagation();
          startInstall(part.id, dash.groups);
        }}
      >
        {installLive || installPending ? t("deps.installing") : t("deps.install")}
      </button>
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
