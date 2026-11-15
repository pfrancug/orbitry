<script lang="ts">
  import { goto } from "$app/navigation";
  import { dash, startBlockReason } from "$lib/dashboard.svelte";
  import {
    displayRunPorts,
    findPart,
    formatUptime,
    groupNameFor,
    healthLabel,
    issueWarnings,
    partTitle,
    portUrl,
    warningsForRun,
  } from "$lib/format";
  import { favoriteKey, loadFavorites, loadRecent, toggleFavorite, type RecentScript } from "$lib/recent";
  import {
    isPending,
    openRun,
    restartScript,
    runState,
    selectPart,
    startScript,
    stopAll,
    stopScript,
  } from "$lib/runs.svelte";
  import type { Run } from "$lib/types";
  import Icon from "$lib/components/Icon.svelte";
  import PortChips from "$lib/components/PortChips.svelte";
  import WarningBadge from "$lib/components/WarningBadge.svelte";

  let now = $state(Date.now());
  let confirmStop = $state(false);
  let recent = $state<RecentScript[]>(loadRecent());
  let favorites = $state<string[]>(loadFavorites());
  let clock: ReturnType<typeof setInterval> | undefined;

  $effect(() => {
    clock = setInterval(() => {
      now = Date.now();
      recent = loadRecent();
      favorites = loadFavorites();
    }, 1000);
    return () => clearInterval(clock);
  });

  const running = $derived(runState.runs.filter((run) => run.status === "running"));
  const portsInUse = $derived(
    new Set(running.flatMap((run) => displayRunPorts(run).map((item) => item.port))).size,
  );
  const healthy = $derived(running.filter((run) => run.health?.status === "ok").length);
  const allWarnings = $derived([...(runState.warnings ?? []), ...(dash.warnings ?? [])]);
  const issues = $derived(issueWarnings(allWarnings));
  const unhealthy = $derived(
    runState.runs.filter((run) => run.status === "failed").length +
      running.filter((run) => run.health?.status === "fail" && !warningsForRun(run, allWarnings).some((item) => item.code === "health_fail")).length +
      issues.length,
  );
  const recentDone = $derived(
    runState.runs
      .filter((run) => run.status === "exited" || run.status === "failed")
      .slice()
      .sort((a, b) => (b.endedAt ?? b.startedAt) - (a.endedAt ?? a.startedAt))
      .slice(0, 8),
  );

  const quickStarts = $derived.by(() => {
    const fav = favorites
      .map((key) => {
        const at = key.indexOf("::");
        if (at < 0) return null;
        const partId = key.slice(0, at);
        const script = key.slice(at + 2);
        const part = findPart(partId, dash.groups);
        if (!part) return null;
        return {
          partId,
          partName: partTitle(part),
          groupName: groupNameFor(partId, dash.groups),
          script,
          favorite: true,
        };
      })
      .filter((item): item is NonNullable<typeof item> => item !== null);
    const rest = recent.filter(
      (item) => !favorites.includes(favoriteKey(item.partId, item.script)) && findPart(item.partId, dash.groups),
    );
    return [...fav, ...rest].slice(0, 8);
  });

  function partLabel(run: Run): string {
    const part = findPart(run.partId, dash.groups);
    return part ? partTitle(part) : run.partName;
  }

  function showLogs(run: Run) {
    openRun(run.id, false);
    void goto("/logs?run=" + encodeURIComponent(run.id));
  }

  function inspect(run: Run) {
    selectPart(run.partId, run.script, true);
  }

  async function onStopAll() {
    if (!confirmStop) {
      confirmStop = true;
      setTimeout(() => (confirmStop = false), 3000);
      return;
    }
    confirmStop = false;
    await stopAll();
  }
</script>

<div class="page-head">
  <div>
    <h1>Overview</h1>
    <p>Everything currently running, in one place.</p>
  </div>
  <div class="page-actions">
    {#if running.length > 0}
      <button class="btn danger" type="button" disabled={runState.stoppingAll} onclick={onStopAll}>
        <Icon name="stop" size={12} />
        {#if runState.stoppingAll}
          Stopping...
        {:else if confirmStop}
          Confirm stop all
        {:else}
          Stop all
        {/if}
      </button>
    {/if}
  </div>
</div>

<div class="kpi-grid">
  <div class="kpi" class:live={running.length > 0}>
    <span class="kpi-label"><Icon name="activity" size={14} /> Running</span>
    <b>{running.length}</b>
    <em>{dash.loading ? "Scanning..." : `${dash.groups.length} projects`}</em>
  </div>
  <div class="kpi">
    <span class="kpi-label"><Icon name="port" size={14} /> Ports in use</span>
    <b>{portsInUse}</b>
    <em>live bindings</em>
  </div>
  <div class="kpi" class:ok={healthy > 0}>
    <span class="kpi-label"><Icon name="heart" size={14} /> Healthy</span>
    <b>{healthy}</b>
    <em>of {running.length} running</em>
  </div>
  <div class="kpi" class:alert={unhealthy > 0}>
    <span class="kpi-label"><Icon name="alert" size={14} /> Issues</span>
    <b>{unhealthy}</b>
    <em>failures and warnings</em>
  </div>
</div>

<section class="section">
  <header class="section-head">
    <h2>Running processes</h2>
    <span class="muted">{running.length === 0 ? "None" : running.length === 1 ? "1 process" : `${running.length} processes`}</span>
  </header>

  {#if running.length === 0}
    <div class="empty-card">
      <strong>Nothing is running.</strong>
      <p>Start a recent script here, or open Projects to pick an app.</p>
      {#if quickStarts.length > 0}
        <div class="quick-list">
          {#each quickStarts as item (`${item.partId}::${item.script}`)}
            <button
              class="quick-item"
              type="button"
              disabled={isPending(item.partId, item.script) || Boolean(startBlockReason(item.partId, item.script))}
              title={startBlockReason(item.partId, item.script) ?? `Start ${item.script}`}
              onclick={() => startScript(item.partId, item.script, dash.groups)}
            >
              <Icon name="play" size={12} />
              <span>
                <strong>{item.partName}</strong>
                <em>{item.groupName} · {item.script}</em>
              </span>
            </button>
          {/each}
        </div>
      {:else}
        <a class="btn primary" href="/projects">Open projects</a>
      {/if}
    </div>
  {:else}
    <div class="table-wrap">
      <table class="run-table">
        <thead>
          <tr>
            <th>Project</th>
            <th>App</th>
            <th>Script</th>
            <th>Health</th>
            <th>Port</th>
            <th>Uptime</th>
            <th>PID</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each running as run (run.id)}
            {@const ports = displayRunPorts(run)}
            {@const port = ports[0]}
            {@const pending = isPending(run.partId, run.script)}
            {@const warnings = warningsForRun(run, allWarnings)}
            <tr class:selected={runState.selectedId === run.id && runState.panelOpen}>
              <td class="muted">{groupNameFor(run.partId, dash.groups) || "—"}</td>
              <td>
                <button class="linkish" type="button" onclick={() => inspect(run)}>
                  {partLabel(run)}
                  <WarningBadge {warnings} />
                </button>
              </td>
              <td><span class="badge quiet">{run.script}</span></td>
              <td>
                <span class="health-cell health-{run.health?.status ?? 'unknown'}">
                  <span class="status-dot" class:on={run.health?.status === "ok"} class:fail={run.health?.status === "fail"}></span>
                  {healthLabel(run.health)}
                </span>
              </td>
              <td>
                <PortChips {ports} health={run.health} />
              </td>
              <td class="tabular">{formatUptime(run.startedAt, now)}</td>
              <td class="tabular muted">{run.pid ?? "—"}</td>
              <td class="row-actions">
                {#if port}
                  <a class="icon-btn ghost" href={portUrl(port.port, port.url)} target="_blank" rel="noopener noreferrer" title="Open in browser">
                    <Icon name="external" />
                  </a>
                {/if}
                <button class="icon-btn ghost" type="button" title="Logs" onclick={() => showLogs(run)}>
                  <Icon name="logs" />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={startBlockReason(run.partId, run.script) ?? "Restart"}
                  disabled={pending || Boolean(startBlockReason(run.partId, run.script))}
                  onclick={() => restartScript(run.partId, run.script, dash.groups)}
                >
                  <Icon name="restart" />
                </button>
                <button
                  class="icon-btn ghost danger"
                  type="button"
                  title="Stop"
                  disabled={pending}
                  onclick={() => stopScript(run.partId, run.script)}
                >
                  <Icon name="stop" size={12} />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

{#if recentDone.length > 0}
  <section class="section">
    <header class="section-head">
      <h2>Recently stopped</h2>
    </header>
    <div class="table-wrap">
      <table class="run-table compact">
        <thead>
          <tr>
            <th>Project</th>
            <th>App</th>
            <th>Script</th>
            <th>Status</th>
            <th>Ended</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each recentDone as run (run.id)}
            <tr>
              <td class="muted">{groupNameFor(run.partId, dash.groups) || "—"}</td>
              <td>{partLabel(run)}</td>
              <td><span class="badge quiet">{run.script}</span></td>
              <td>
                <span class="log-status status-{run.status}">
                  {run.status}{run.exitCode != null ? ` (${run.exitCode})` : ""}
                </span>
              </td>
              <td class="muted">{formatUptime(run.endedAt, now)} ago</td>
              <td class="row-actions">
                <button class="icon-btn ghost" type="button" title="Logs" onclick={() => showLogs(run)}>
                  <Icon name="logs" />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={startBlockReason(run.partId, run.script) ?? "Start again"}
                  disabled={isPending(run.partId, run.script) || Boolean(startBlockReason(run.partId, run.script))}
                  onclick={() => startScript(run.partId, run.script, dash.groups)}
                >
                  <Icon name="play" size={12} />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={favorites.includes(favoriteKey(run.partId, run.script)) ? "Unfavorite" : "Favorite"}
                  onclick={() => (favorites = toggleFavorite(run.partId, run.script))}
                >
                  <Icon name={favorites.includes(favoriteKey(run.partId, run.script)) ? "star-fill" : "star"} />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
{/if}
