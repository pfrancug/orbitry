<script lang="ts">
  import { appAutostartEnabled, appAutostartTitle, dash, setAppAutostart, startBlockReason, startWarnReason, systemAutostartEnabled } from "$lib/dashboard.svelte";
  import { openPath } from "$lib/api";
  import {
    buildScripts,
    displayPorts,
    formatUptime,
    healthLabel,
    installCommand,
    isJsLike,
    partTitle,
    roleLabel,
    runScripts,
    scriptLabel,
    shortPath,
    timeAgo,
    warningsForPart,
    warningsForRun,
  } from "$lib/format";
  import { i18n, t } from "$lib/i18n";
  import {
    closePanel,
    isPending,
    latestRun,
    restartScript,
    runningFor,
    runState,
    selectPart,
    startInstall,
    startScript,
    stopScript,
  } from "$lib/runs.svelte";
  import type { ProjectPart } from "$lib/types";
  import Icon from "./Icon.svelte";
  import PortChips from "./PortChips.svelte";
  import LogViewer from "./LogViewer.svelte";
  import WarningList from "./WarningList.svelte";
  import PortOverrides from "./PortOverrides.svelte";
  import { tools } from "$lib/tools.svelte";
  import ResourceChip from "./ResourceChip.svelte";
  import { runMetrics } from "$lib/metrics.svelte";

  let { part }: { part: ProjectPart | null } = $props();

  const locale = $derived(i18n.locale);
  const systemOn = $derived(systemAutostartEnabled());
  const autoOn = $derived(part ? appAutostartEnabled(part.id) : false);
  const autoTitle = $derived(appAutostartTitle());
  const startables = $derived(runScripts(part));
  const builds = $derived(buildScripts(part));

  const scriptName = $derived.by(() => {
    if (!part) return null;
    if (runState.selectedScript && startables.some((item) => item.name === runState.selectedScript)) {
      return runState.selectedScript;
    }
    return runningFor(part.id)?.script ?? startables[0]?.name ?? null;
  });

  const selectedScript = $derived(startables.find((item) => item.name === scriptName) ?? null);

  const current = $derived.by(() => {
    if (!part) return null;
    return (
      (scriptName ? runningFor(part.id, scriptName) : null) ??
      (runState.selectedId ? (runState.runs.find((run) => run.id === runState.selectedId) ?? null) : null) ??
      latestRun(part.id, scriptName) ??
      null
    );
  });

  const live = $derived(part && scriptName ? runningFor(part.id, scriptName) : undefined);
  const pending = $derived(part && scriptName ? isPending(part.id, scriptName) : false);
  const startBlocked = $derived(part && scriptName && !live ? startBlockReason(part.id, scriptName) : null);
  const startWarned = $derived(part && scriptName && !live ? startWarnReason(part.id, scriptName) : null);
  const ports = $derived(part ? displayPorts(part, runState.runs) : []);
  const health = $derived(live?.health ?? current?.health);
  const expected = $derived(!live);
  const warnings = $derived(
    part
      ? (live || current
          ? warningsForRun(live ?? current, [...(runState.warnings ?? []), ...(dash.warnings ?? [])])
          : warningsForPart(part.id, runState.runs, [...(runState.warnings ?? []), ...(dash.warnings ?? [])]))
      : [],
  );
  const missingDeps = $derived(part ? isJsLike(part.kind) && !part.nodeModules : false);
  const pm = $derived(part?.packageManager || "npm");
  const commandPreview = $derived(
    selectedScript
      ? selectedScript.command || t("script.command", { pm, name: selectedScript.name })
      : "",
  );
  const cwdPreview = $derived(selectedScript?.cwd || part?.path || "");
  const sourceHint = $derived(
    selectedScript?.source === "workspace"
      ? t("script.viaRoot", { name: selectedScript.sourceName || t("script.workspace") })
      : t("script.package"),
  );

  let opening = $state<string | null>(null);

  async function openWith(kind: "terminal" | "vscode") {
    if (!part) return;
    opening = kind;
    dash.error = null;
    try {
      await openPath(kind, part.path);
    } catch (err: unknown) {
      dash.error = err instanceof Error ? err.message : t("panel.openFailed");
    } finally {
      opening = null;
    }
  }

  function statusLabel(run: typeof current): string {
    if (!run) return t("panel.status.idle");
    if (run.status === "running") return t("panel.status.running");
    if (run.status === "failed") return run.exitCode == null ? t("panel.status.failed") : t("panel.status.failedCode", { code: run.exitCode });
    return run.exitCode == null ? t("panel.status.exited") : t("panel.status.exitedCode", { code: run.exitCode });
  }
</script>

<aside class="control-panel" aria-label={t("panel.title")} data-locale={locale}>
  <header class="panel-head">
    <div>
      {#if part}
        <div class="title-row">
          <span class="badge role role-{part.role}">{roleLabel(part.role)}</span>
          <h2 title={part.name}>{partTitle(part)}</h2>
        </div>
        <p class="path" title={part.path}>{shortPath(part.path)}</p>
      {:else}
        <h2>{t("panel.title")}</h2>
        <p class="path">{t("panel.pick")}</p>
      {/if}
    </div>
    <button class="icon-btn ghost" type="button" onclick={closePanel} title={t("panel.close")} aria-label={t("panel.close")}>
      <Icon name="close" />
    </button>
  </header>

  {#if !part}
    <div class="panel-empty">
      <p>{t("panel.empty")}</p>
    </div>
  {:else}
    <div class="panel-meta">
      {#if part.version}<span class="badge quiet">v{part.version}</span>{/if}
      {#if part.framework}<span class="badge framework">{part.framework}</span>{/if}
      {#if part.git}
        <span class="git">
          <span class="dot {part.git.dirty ? 'dirty' : 'clean'}"></span>
          {part.git.branch}
        </span>
      {/if}
      <span class="build" class:ok={part.build?.built} title={part.build?.stale ? t("build.stale") : undefined}>
        {part.build?.built ? t("build.ready", { time: timeAgo(part.build.lastBuiltAt) }) : t("build.missing")}
        {#if part.build?.built && part.build.stale} · {t("build.stale")}{/if}
      </span>
    </div>

    <section class="panel-section">
      <h3>{t("panel.about")}</h3>
      <div class="open-actions">
        <button
          class="btn"
          type="button"
          disabled={opening !== null}
          title={t("panel.terminalTitle")}
          onclick={() => openWith("terminal")}
        >
          <Icon name="terminal" size={14} />
          {opening === "terminal" ? t("panel.opening") : t("panel.terminal")}
        </button>
        <button
          class="btn"
          type="button"
          disabled={opening !== null || tools.vscodeInstalled === false}
          title={tools.vscodeInstalled === false ? t("panel.vscodeMissing") : t("panel.vscodeTitle")}
          onclick={() => openWith("vscode")}
        >
          <Icon name="code" size={14} />
          {opening === "vscode" ? t("panel.opening") : "VS Code"}
        </button>
      </div>
    </section>

    <section class="panel-section">
      <h3>{t("panel.autostart")}</h3>
      <label class="panel-autostart" class:off={!systemOn} title={autoTitle}>
        <input
          type="checkbox"
          checked={autoOn}
          disabled={!systemOn || dash.saving || !part}
          aria-label={t("autostart.appFull")}
          onchange={(event) => part && setAppAutostart(part.id, event.currentTarget.checked)}
        />
        {t("autostart.appFull")}
      </label>
    </section>

    <section class="panel-section">
      <h3>{t("panel.run")}</h3>
      {#if !startables.length}
        <p class="hint">{t("script.noneHint")}</p>
      {:else}
        <label class="script-picker">
          <span>{t("script.label")}</span>
          <select
            value={scriptName ?? ""}
            onchange={(event) => {
              const value = (event.currentTarget as HTMLSelectElement).value;
              if (value) selectPart(part.id, value, true);
            }}
          >
            {#each startables as script (`${script.kind}:${script.name}`)}
              <option value={script.name}>{scriptLabel(script)}</option>
            {/each}
          </select>
        </label>
        {#if selectedScript}
          <div class="command-preview">
            <div><strong>{t("script.willRun")}</strong> <code>{commandPreview}</code></div>
            <div class="muted">{t("script.in")} <code title={cwdPreview}>{shortPath(cwdPreview)}</code> · {sourceHint}</div>
          </div>
        {/if}
        {#if startWarned && !startBlocked}
          <p class="hint">{startWarned}</p>
        {/if}
        <div class="panel-actions">
          {#if live}
            <button
              class="btn danger"
              type="button"
              disabled={pending}
              onclick={() => scriptName && stopScript(part.id, scriptName)}
            >
              <Icon name="stop" size={12} /> {t("script.stop")}
            </button>
            <button
              class="btn"
              type="button"
              disabled={pending || Boolean(startBlocked)}
              title={startBlocked ?? t("script.restart")}
              onclick={() => scriptName && restartScript(part.id, scriptName, dash.groups)}
            >
              <Icon name="restart" size={14} /> {t("script.restart")}
            </button>
          {:else}
            <button
              class="btn primary"
              type="button"
              disabled={pending || !scriptName || Boolean(startBlocked)}
              title={startBlocked ?? t("script.start")}
              onclick={() => scriptName && startScript(part.id, scriptName, dash.groups)}
            >
              <Icon name="play" size={12} /> {t("script.start")}
            </button>
          {/if}
        </div>
      {/if}
      <div class="panel-actions secondary-actions">
        {#each builds as script (`build:${script.name}`)}
          {@const run = runningFor(part.id, script.name)}
          {@const building = isPending(part.id, script.name) || Boolean(run)}
          <button
            class="btn"
            type="button"
            disabled={building}
            title={script.command}
            onclick={() => (run ? stopScript(part.id, script.name) : startScript(part.id, script.name, dash.groups))}
          >
            {run ? t("build.running") : t("build.run")}
          </button>
        {/each}
        {#if missingDeps}
          <button
            class="btn"
            type="button"
            disabled={isPending(part.id, "install") || Boolean(runningFor(part.id, "install"))}
            title={t("deps.explain", { command: installCommand(part) })}
            onclick={() => startInstall(part.id, dash.groups)}
          >
            {runningFor(part.id, "install") || isPending(part.id, "install") ? t("deps.installing") : t("deps.install")}
          </button>
        {/if}
      </div>
    </section>

    <section class="panel-section">
      <h3>{t("panel.status")}</h3>
      <div class="panel-status">
        <span class="log-status status-{current?.status ?? 'idle'}">{statusLabel(current)}</span>
        <PortChips {ports} health={health ?? null} {expected} />
        <span class="health health-{health?.status ?? 'unknown'}" title={health?.url ?? ""}>
          {healthLabel(health)}
        </span>
        {#if current?.pid}
          <span class="muted">pid {current.pid}</span>
        {/if}
        {#if current?.status === "running"}
          <span class="muted">{formatUptime(current.startedAt)}</span>
          <ResourceChip usage={runMetrics(current.id)} />
        {/if}
      </div>
    </section>

    {#if warnings.length}
      <section class="panel-warnings panel-section scroll-region">
        <h3>{t("panel.warnings")}</h3>
        <WarningList {warnings} />
      </section>
    {/if}

    <section class="panel-section">
      <h3>{t("panel.ports")}</h3>
      <PortOverrides {part} {ports} />
    </section>

    <section class="panel-section panel-logs">
      <h3>{t("panel.logs")}</h3>
      <LogViewer runId={current?.id ?? null} />
    </section>
  {/if}
</aside>

<style>
  .panel-autostart {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    margin: 0;
    font-size: 0.88rem;
    cursor: pointer;
    user-select: none;
  }
  .panel-autostart input {
    margin: 0;
    accent-color: var(--accent);
  }
  .panel-autostart.off {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .panel-section {
    margin: 0 16px 14px;
  }
  .panel-section h3 {
    margin: 0 0 8px;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .command-preview {
    margin: 8px 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    font-size: 11.5px;
    line-height: 1.45;
    word-break: break-word;
  }
  .command-preview code {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }
  .secondary-actions {
    margin-top: 8px;
  }
</style>
