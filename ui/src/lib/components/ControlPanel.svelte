<script lang="ts">
  import { appAutostartEnabled, appAutostartTitle, dash, setAppAutostart, startBlockReason, startWarnReason, systemAutostartEnabled } from "$lib/dashboard.svelte";
  import { openPath } from "$lib/api";
  import {
    allScripts,
    desktopOs,
    displayPorts,
    formatUptime,
    healthLabel,
    installCommand,
    isJsLike,
    partTitle,
    roleLabel,
    scriptLabel,
    scriptSourceDiffers,
    shortPath,
    timeAgo,
    warningsForPart,
    warningsForRun,
    scriptEachKey,
  } from "$lib/format";
  import { i18n, t } from "$lib/i18n.svelte";
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
  import { tools } from "$lib/tools.svelte";
  import ResourceChip from "./ResourceChip.svelte";
  import { runMetrics } from "$lib/metrics.svelte";

  let { part }: { part: ProjectPart | null } = $props();

  const locale = $derived(i18n.locale);
  const systemOn = $derived(systemAutostartEnabled());
  const autoOn = $derived(part ? appAutostartEnabled(part.id) : false);
  const autoTitle = $derived(appAutostartTitle());
  const scripts = $derived(allScripts(part));
  let tab = $state<"details" | "logs">("details");

  $effect(() => {
    // Keep the chosen tab when switching apps; reset only when the panel has no part.
    if (!part) tab = "details";
  });

  const scriptName = $derived.by(() => {
    if (!part) return null;
    if (runState.selectedScript && scripts.some((item) => item.name === runState.selectedScript)) {
      return runState.selectedScript;
    }
    return runningFor(part.id)?.script ?? scripts[0]?.name ?? null;
  });

  const selectedScript = $derived(scripts.find((item) => item.name === scriptName) ?? null);

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
    selectedScript && scriptSourceDiffers(selectedScript)
      ? t("script.viaRoot", { name: selectedScript.sourceName || t("script.workspace") })
      : null,
  );
  const os = $derived(desktopOs());
  const explorerLabel = $derived(
    os === "windows" ? t("panel.explorer.windows") : os === "mac" ? t("panel.explorer.mac") : t("panel.explorer.linux"),
  );
  const explorerTitle = $derived(
    os === "windows" ? t("panel.explorerTitle.windows") : os === "mac" ? t("panel.explorerTitle.mac") : t("panel.explorerTitle.linux"),
  );

  let opening = $state<string | null>(null);

  async function openWith(kind: "terminal" | "vscode" | "explorer") {
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
    <div class="panel-tabs" role="tablist">
      <button
        class="panel-tab"
        class:active={tab === "details"}
        type="button"
        role="tab"
        aria-selected={tab === "details"}
        onclick={() => (tab = "details")}
      >
        {t("panel.tab.details")}
      </button>
      <button
        class="panel-tab"
        class:active={tab === "logs"}
        type="button"
        role="tab"
        aria-selected={tab === "logs"}
        onclick={() => (tab = "logs")}
      >
        {t("panel.tab.logs")}
      </button>
    </div>

    {#if tab === "details"}
      <div class="panel-body">
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
          <h3>{t("panel.autostart")}</h3>
          <label class="panel-autostart" class:off={!systemOn} title={autoTitle}>
            <input
              type="checkbox"
              checked={autoOn}
              disabled={!systemOn || !part}
              aria-label={t("autostart.appFull")}
              onchange={(event) => part && setAppAutostart(part.id, event.currentTarget.checked)}
            />
            {t("autostart.appFull")}
          </label>
        </section>

        <section class="panel-section">
          <h3>{t("panel.scripts")}</h3>
          {#if !scripts.length}
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
                {#each scripts as script (scriptEachKey(script))}
                  <option value={script.name}>{scriptLabel(script)}</option>
                {/each}
              </select>
            </label>
            {#if selectedScript}
              <div class="command-preview">
                <div class="preview-field">
                  <span class="preview-label">{t("script.commandLabel")}</span>
                  <pre class="preview-command"><code>{commandPreview}</code></pre>
                </div>
                <div class="preview-field">
                  <span class="preview-label">{t("script.cwdLabel")}</span>
                  <code class="preview-cwd" title={cwdPreview}>{cwdPreview}</code>
                </div>
                {#if sourceHint}
                  <div class="preview-field">
                    <span class="preview-label">{t("script.sourceLabel")}</span>
                    <span class="preview-source">{sourceHint}</span>
                  </div>
                {/if}
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
        </section>

        {#if missingDeps}
          <section class="panel-section">
            <h3>{t("deps.status")}</h3>
            <p class="hint">{t("deps.statusHint", { command: installCommand(part) })}</p>
            <div class="panel-actions">
              <button
                class="btn"
                type="button"
                disabled={isPending(part.id, "install") || Boolean(runningFor(part.id, "install"))}
                title={t("deps.explain", { command: installCommand(part) })}
                onclick={() => startInstall(part.id, dash.groups)}
              >
                {runningFor(part.id, "install") || isPending(part.id, "install") ? t("deps.installing") : t("deps.install")}
              </button>
            </div>
          </section>
        {/if}

        <section class="panel-section">
          <h3>{t("panel.status")}</h3>
          <div class="panel-status">
            <span class="log-status status-{current?.status ?? 'idle'}">{statusLabel(current)}</span>
            <PortChips {ports} health={health ?? null} {expected} {part} script={scriptName} />
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
          <section class="panel-section">
            <h3>{t("panel.warnings")}</h3>
            <WarningList {warnings} flat />
          </section>
        {/if}
      </div>
    {:else}
      <div class="panel-body panel-logs-tab">
        <LogViewer runId={current?.id ?? null} />
      </div>
    {/if}

    <footer class="panel-footer">
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
        <button
          class="btn"
          type="button"
          disabled={opening !== null}
          title={explorerTitle}
          onclick={() => openWith("explorer")}
        >
          <Icon name="folder" size={14} />
          {opening === "explorer" ? t("panel.opening") : explorerLabel}
        </button>
      </div>
    </footer>
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
  .panel-tabs {
    display: flex;
    gap: 0;
    margin: 0 16px;
    border-bottom: 1px solid var(--line);
    flex: 0 0 auto;
  }
  .panel-tab {
    appearance: none;
    background: transparent;
    border: 0;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    padding: 8px 12px;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .panel-tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .panel-tab:hover:not(.active) {
    color: var(--text);
    background: transparent;
  }
  .panel-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    padding-top: 12px;
  }
  .panel-logs-tab {
    display: flex;
    flex-direction: column;
    padding: 0;
    background: #0b0c10;
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
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .preview-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .preview-label {
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .preview-command {
    margin: 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.28);
    overflow-x: auto;
    overscroll-behavior: contain;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .preview-command code,
  .preview-cwd {
    font-family: var(--mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
    font-size: 12px;
  }
  .preview-cwd {
    display: block;
    overflow-wrap: anywhere;
  }
  .preview-source { font-size: 12px; color: var(--text); }
  .panel-footer {
    flex: 0 0 auto;
    margin-top: auto;
    padding: 12px 16px 16px;
    border-top: 1px solid var(--line);
    background: rgba(11, 11, 16, 0.96);
  }
  .panel-footer .open-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
