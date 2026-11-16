<script lang="ts">
  import { goto } from "$app/navigation";
  import { dash, startBlockReason, visibleWarnings } from "$lib/dashboard.svelte";
  import { i18n, t } from "$lib/i18n.svelte";
  import {
    displayRunPorts,
    findPart,
    formatUptime,
    groupNameFor,
    healthLabel,
    issueWarnings,
    partTitle,
    warningsForRun,
  } from "$lib/format";
  import { clearAllDone, clearAllRecent, dismissDone, doneKey, favoriteKey, loadDismissedDone, loadFavorites, loadRecent, removeRecent, toggleFavorite, type RecentScript } from "$lib/recent";
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
  import ActionMenu from "$lib/components/ActionMenu.svelte";
  import PortChips from "$lib/components/PortChips.svelte";
  import WarningBadge from "$lib/components/WarningBadge.svelte";
  import ResourceMeter from "$lib/components/ResourceMeter.svelte";
  import ResourceChip from "$lib/components/ResourceChip.svelte";
  import { metrics, runMetrics } from "$lib/metrics.svelte";
  import { fmtBytes, fmtPct } from "$lib/format";

  let now = $state(Date.now());
  let confirmStop = $state(false);
  const locale = $derived(i18n.locale);
  let recent = $state<RecentScript[]>(loadRecent());
  let favorites = $state<string[]>(loadFavorites());
  let dismissedDone = $state<string[]>(loadDismissedDone());
  let clock: ReturnType<typeof setInterval> | undefined;

  $effect(() => {
    clock = setInterval(() => {
      now = Date.now();
      recent = loadRecent();
      favorites = loadFavorites();
      dismissedDone = loadDismissedDone();
    }, 1000);
    return () => clearInterval(clock);
  });

  const running = $derived(runState.runs.filter((run) => run.status === "running"));
  const portsInUse = $derived(
    new Set(running.flatMap((run) => displayRunPorts(run).map((item) => item.port))).size,
  );
  const healthy = $derived(running.filter((run) => run.health?.status === "ok").length);
  const allWarnings = $derived(visibleWarnings([...(runState.warnings ?? []), ...(dash.warnings ?? [])]));
  const issues = $derived(issueWarnings(allWarnings));
  const unhealthy = $derived(
    runState.runs.filter((run) => run.status === "failed").length +
      running.filter((run) => run.health?.status === "fail" && !warningsForRun(run, allWarnings).some((item) => item.code === "health_fail")).length +
      issues.length,
  );
  const recentDone = $derived(
    runState.runs
      .filter((run) => run.status === "exited" || run.status === "failed")
      .filter((run) => !dismissedDone.includes(doneKey(run.partId, run.script, run.endedAt, run.id)))
      .slice()
      .sort((a, b) => (b.endedAt ?? b.startedAt) - (a.endedAt ?? a.startedAt))
      .slice(0, 8),
  );

  function clearRecentList() {
    recent = clearAllRecent();
  }

  function removeRecentItem(partId: string, script: string) {
    recent = removeRecent(partId, script);
  }

  function dismissStopped(run: Run) {
    dismissedDone = dismissDone(doneKey(run.partId, run.script, run.endedAt, run.id));
  }

  function clearStopped() {
    dismissedDone = clearAllDone(recentDone.map((run) => doneKey(run.partId, run.script, run.endedAt, run.id)));
  }

  function processCount(n: number): string {
    if (n === 0) return t("overview.none");
    if (n == 1) return `1 ${t("overview.process.one")}`;
    return `${n} ${t("overview.process.many")}`;
  }

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

<div class="page-head" data-locale={locale}>
  <div>
    <h1>{t("overview.title")}</h1>
    <p>{t("overview.subtitle")}</p>
  </div>
  <div class="page-actions">
    {#if running.length > 0}
      <button class="btn danger" type="button" disabled={runState.stoppingAll} onclick={onStopAll}>
        <Icon name="stop" size={12} />
        {#if runState.stoppingAll}
          {t("overview.stopping")}
        {:else if confirmStop}
          {t("overview.stopAllConfirm")}
        {:else}
          {t("overview.stopAll")}
        {/if}
      </button>
    {/if}
  </div>
</div>

<div class="kpi-grid">
  <div class="kpi" class:live={running.length > 0}>
    <span class="kpi-label"><Icon name="activity" size={14} /> {t("overview.kpi.running")}</span>
    <b>{running.length}</b>
    <em>{dash.loading ? t("overview.kpi.scanning") : t("overview.kpi.projects", { count: dash.groups.length })}</em>
  </div>
  <div class="kpi">
    <span class="kpi-label"><Icon name="port" size={14} /> {t("overview.kpi.ports")}</span>
    <b>{portsInUse}</b>
    <em>{t("overview.kpi.portsSub")}</em>
  </div>
  <div class="kpi" class:ok={healthy > 0}>
    <span class="kpi-label"><Icon name="heart" size={14} /> {t("overview.kpi.healthy")}</span>
    <b>{healthy}</b>
    <em>{t("overview.kpi.healthySub", { count: running.length })}</em>
  </div>
  <div class="kpi" class:alert={unhealthy > 0}>
    <span class="kpi-label"><Icon name="alert" size={14} /> {t("overview.kpi.issues")}</span>
    <b>{unhealthy}</b>
    <em>{t("overview.kpi.issuesSub")}</em>
  </div>
</div>

{#if metrics.snapshot?.system}
  {@const system = metrics.snapshot.system}
  {@const ramPct = system.memTotal ? (system.memUsed / system.memTotal) * 100 : null}
  <section class="section resource-section">
    <header class="section-head">
      <h2>{t("overview.resources")}</h2>
      <span class="muted">{t("overview.resourcesSub")}</span>
    </header>
    <div class="resource-grid">
      <ResourceMeter label={t("common.cpu")} color="#818cf8" pct={system.cpuPct} value={fmtPct(system.cpuPct)} values={metrics.snapshot.history?.cpu ?? []} />
      <ResourceMeter
        label={t("common.ram")}
        color="#22d3ee"
        pct={ramPct}
        value={fmtPct(ramPct)}
        sub="{fmtBytes(system.memUsed)} / {fmtBytes(system.memTotal)}"
        values={metrics.snapshot.history?.memPct ?? []}
      />
    </div>
  </section>
{/if}

<section class="section">
  <header class="section-head">
    <h2>{t("overview.running")}</h2>
    <span class="muted">{processCount(running.length)}</span>
  </header>

  {#if running.length === 0}
    <div class="empty-card">
      <strong>{t("overview.emptyTitle")}</strong>
      <p>{t("overview.emptyBody")}</p>
      {#if quickStarts.length > 0}
        <div class="quick-toolbar">
          <button class="btn ghost" type="button" onclick={clearRecentList}>{t("overview.clearAll")}</button>
        </div>
        <div class="quick-list">
          {#each quickStarts as item (`${item.partId}::${item.script}`)}
            <div class="quick-row">
              <button
                class="quick-item"
                type="button"
                disabled={isPending(item.partId, item.script) || Boolean(startBlockReason(item.partId, item.script))}
                title={startBlockReason(item.partId, item.script) ?? `${t("script.start")} ${item.script}`}
                onclick={() => startScript(item.partId, item.script, dash.groups)}
              >
                <Icon name="play" size={12} />
                <span>
                  <strong>{item.partName}</strong>
                  <em>{item.groupName} · {item.script}</em>
                </span>
              </button>
              {#if !item.favorite}
                <button class="icon-btn ghost" type="button" title={t("overview.clearItem")} onclick={() => removeRecentItem(item.partId, item.script)}>×</button>
              {/if}
            </div>
          {/each}
        </div>
      {:else}
        <a class="btn primary" href="/projects">{t("overview.openProjects")}</a>
      {/if}
    </div>
  {:else}
    <div class="table-wrap">
      <table class="run-table">
        <thead>
          <tr>
            <th>{t("overview.col.project")}</th>
            <th>{t("overview.col.app")}</th>
            <th>{t("overview.col.script")}</th>
            <th>{t("overview.col.health")}</th>
            <th>{t("overview.col.port")}</th>
            <th>{t("overview.col.uptime")}</th>
            <th>{t("overview.col.cpuRam")}</th>
            <th>{t("overview.col.pid")}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each running as run (run.id)}
            {@const part = findPart(run.partId, dash.groups) ?? null}
            {@const ports = displayRunPorts(run, part)}
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
                <PortChips {ports} health={run.health} {part} script={run.script} />
              </td>
              <td class="tabular">{formatUptime(run.startedAt, now)}</td>
              <td><ResourceChip usage={runMetrics(run.id)} /></td>
              <td class="tabular muted">{run.pid ?? "—"}</td>
              <td class="row-actions">
                <button class="icon-btn ghost" type="button" title={t("overview.logs")} onclick={() => showLogs(run)}>
                  <Icon name="logs" />
                </button>
                <ActionMenu title={t("overview.more")}>
                  <button
                    type="button"
                    role="menuitem"
                    disabled={pending || Boolean(startBlockReason(run.partId, run.script))}
                    onclick={() => restartScript(run.partId, run.script, dash.groups)}
                  >
                    {t("overview.restart")}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    class="danger"
                    disabled={pending}
                    onclick={() => stopScript(run.partId, run.script)}
                  >
                    {t("overview.stop")}
                  </button>
                </ActionMenu>
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
      <h2>{t("overview.recentStopped")}</h2>
      <button class="btn ghost" type="button" onclick={clearStopped}>{t("overview.clearAll")}</button>
    </header>
    <div class="table-wrap">
      <table class="run-table compact">
        <thead>
          <tr>
            <th>{t("overview.col.project")}</th>
            <th>{t("overview.col.app")}</th>
            <th>{t("overview.col.script")}</th>
            <th>{t("overview.col.status")}</th>
            <th>{t("overview.col.ended")}</th>
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
              <td class="muted">{t("overview.ago", { time: formatUptime(run.endedAt, now) })}</td>
              <td class="row-actions">
                <button class="icon-btn ghost" type="button" title={t("overview.logs")} onclick={() => showLogs(run)}>
                  <Icon name="logs" />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={startBlockReason(run.partId, run.script) ?? t("overview.startAgain")}
                  disabled={isPending(run.partId, run.script) || Boolean(startBlockReason(run.partId, run.script))}
                  onclick={() => startScript(run.partId, run.script, dash.groups)}
                >
                  <Icon name="play" size={12} />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={favorites.includes(favoriteKey(run.partId, run.script)) ? t("overview.unfavorite") : t("overview.favorite")}
                  onclick={() => (favorites = toggleFavorite(run.partId, run.script))}
                >
                  <Icon name={favorites.includes(favoriteKey(run.partId, run.script)) ? "star-fill" : "star"} />
                </button>
                <button class="icon-btn ghost" type="button" title={t("overview.clearItem")} onclick={() => dismissStopped(run)}>×</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
{/if}
