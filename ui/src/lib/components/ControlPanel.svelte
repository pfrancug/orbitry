<script lang="ts">
  import { appAutostartEnabled, appAutostartTitle, dash, setAppAutostart, startBlockReason, systemAutostartEnabled } from "$lib/dashboard.svelte";
  import { openPath } from "$lib/api";
  import {
    displayPorts,
    formatUptime,
    healthLabel,
    partTitle,
    roleLabel,
    shortPath,
    timeAgo,
    warningsForPart,
    warningsForRun,
  } from "$lib/format";
  import {
    closePanel,
    isPending,
    latestRun,
    restartScript,
    runningFor,
    runState,
    selectPart,
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

  const systemOn = $derived(systemAutostartEnabled());
  const autoOn = $derived(part ? appAutostartEnabled(part.id) : false);
  const autoTitle = $derived(appAutostartTitle());

  const scriptName = $derived.by(() => {
    if (!part) return null;
    if (runState.selectedScript && (part.scripts ?? []).some((item) => item.name === runState.selectedScript)) {
      return runState.selectedScript;
    }
    return runningFor(part.id)?.script ?? (part.scripts ?? [])[0]?.name ?? null;
  });

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

  let opening = $state<string | null>(null);

  async function openWith(kind: "terminal" | "vscode") {
    if (!part) return;
    opening = kind;
    dash.error = null;
    try {
      await openPath(kind, part.path);
    } catch (err: unknown) {
      dash.error = err instanceof Error ? err.message : "Could not open the folder.";
    } finally {
      opening = null;
    }
  }

  function statusLabel(run: typeof current): string {
    if (!run) return "idle";
    if (run.status === "running") return "running";
    if (run.status === "failed") return run.exitCode == null ? "failed" : `failed (${run.exitCode})`;
    return run.exitCode == null ? "exited" : `exited (${run.exitCode})`;
  }
</script>

<aside class="control-panel" aria-label="Control panel">
  <header class="panel-head">
    <div>
      {#if part}
        <div class="title-row">
          <span class="badge role role-{part.role}">{roleLabel(part.role)}</span>
          <h2 title={part.name}>{partTitle(part)}</h2>
        </div>
        <p class="path" title={part.path}>{shortPath(part.path)}</p>
      {:else}
        <h2>Control panel</h2>
        <p class="path">Select an app to inspect it</p>
      {/if}
    </div>
    <button class="icon-btn ghost" type="button" onclick={closePanel} title="Close panel" aria-label="Close panel">
      <Icon name="close" />
    </button>
  </header>

  {#if !part}
    <div class="panel-empty">
      <p>Pick an app from Projects or a running process from Overview.</p>
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
      <span class="build" class:ok={part.build?.built}>
        {part.build?.built ? `Built ${timeAgo(part.build.lastBuiltAt)}` : "Not built"}
      </span>
    </div>

    <div class="open-actions">
      <button
        class="btn"
        type="button"
        disabled={opening !== null}
        title="Otwórz folder w terminalu"
        onclick={() => openWith("terminal")}
      >
        <Icon name="terminal" size={14} />
        {opening === "terminal" ? "Otwieranie..." : "Terminal"}
      </button>
      <button
        class="btn"
        type="button"
        disabled={opening !== null || tools.vscodeInstalled === false}
        title={tools.vscodeInstalled === false ? "VS Code nie jest zainstalowany" : "Otwórz w VS Code"}
        onclick={() => openWith("vscode")}
      >
        <Icon name="code" size={14} />
        {#if opening === "vscode"}
          Otwieranie...
        {:else if tools.vscodeInstalled === false}
          VS Code
        {:else}
          VS Code
        {/if}
      </button>
    </div>

    <label class="panel-autostart" class:off={!systemOn} title={autoTitle}>
      <input
        type="checkbox"
        checked={autoOn}
        disabled={!systemOn || dash.saving || !part}
        aria-label="Uruchamiaj przy starcie Orbitry"
        onchange={(event) => part && setAppAutostart(part.id, event.currentTarget.checked)}
      />
      Uruchamiaj przy starcie Orbitry
    </label>

    <div class="panel-controls">
      {#if !(part.scripts ?? []).length}
        <p class="hint">No dev or preview scripts on this app.</p>
      {:else}
        <label class="script-picker">
          <span>Script</span>
          <select
            value={scriptName ?? ""}
            onchange={(event) => {
              const value = (event.currentTarget as HTMLSelectElement).value;
              if (value) selectPart(part.id, value, true);
            }}
          >
            {#each part.scripts ?? [] as script (script.kind + script.name)}
              <option value={script.name}>{script.name} · {script.scope}</option>
            {/each}
          </select>
        </label>
        <div class="panel-actions">
          {#if live}
            <button
              class="btn danger"
              type="button"
              disabled={pending}
              onclick={() => scriptName && stopScript(part.id, scriptName)}
            >
              <Icon name="stop" size={12} /> Stop
            </button>
            <button
              class="btn"
              type="button"
              disabled={pending || Boolean(startBlocked)}
              title={startBlocked ?? "Restart"}
              onclick={() => scriptName && restartScript(part.id, scriptName, dash.groups)}
            >
              <Icon name="restart" size={14} /> Restart
            </button>
          {:else}
            <button
              class="btn primary"
              type="button"
              disabled={pending || !scriptName || Boolean(startBlocked)}
              title={startBlocked ?? "Start"}
              onclick={() => scriptName && startScript(part.id, scriptName, dash.groups)}
            >
              <Icon name="play" size={12} /> Start
            </button>
          {/if}

        </div>
      {/if}
    </div>

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

    {#if warnings.length}
      <section class="panel-warnings scroll-region">
        <h3>Warnings</h3>
        <WarningList {warnings} />
      </section>
    {/if}

    <PortOverrides {part} {ports} />

    <LogViewer runId={current?.id ?? null} />
  {/if}
</aside>

<style>
  .panel-autostart {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    margin: 0 16px 10px;
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
</style>
