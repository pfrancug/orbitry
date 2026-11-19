<script lang="ts">
  import { goto } from "$app/navigation";
  import { dash, startBlockReason } from "$lib/dashboard.svelte";
  import { findPart, formatUptime, groupNameFor, partTitle, runExitLabel } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { MAX_RECENT, RECENT_PREVIEW, type RecentScript } from "$lib/recent";
  import { isPending, openRun, startScript } from "$lib/runs.svelte";
  import type { Run } from "$lib/types";
  import Icon from "./Icon.svelte";

  type Mode = "recent" | "stopped";

  let {
    mode,
    recent = [],
    stopped = [],
    now = Date.now(),
    onClearAll,
    onRemoveRecent,
    onDismissStopped,
  }: {
    mode: Mode;
    recent?: RecentScript[];
    stopped?: Run[];
    now?: number;
    onClearAll: () => void;
    onRemoveRecent?: (partId: string, script: string) => void;
    onDismissStopped?: (run: Run) => void;
  } = $props();

  let expanded = $state(false);

  const items = $derived(
    mode === "recent"
      ? recent.slice(0, MAX_RECENT)
      : stopped.slice(0, MAX_RECENT),
  );
  const visible = $derived(expanded ? items : items.slice(0, RECENT_PREVIEW));
  const hiddenCount = $derived(Math.max(0, items.length - RECENT_PREVIEW));

  function partLabel(run: Run): string {
    const part = findPart(run.partId, dash.groups);
    return part ? partTitle(part) : run.partName;
  }

  function showLogs(run: Run) {
    openRun(run.id, false);
    void goto("/logs?run=" + encodeURIComponent(run.id));
  }

  function recentPartName(item: RecentScript): string {
    const part = findPart(item.partId, dash.groups);
    return part ? partTitle(part) : item.partName;
  }

  function recentGroupName(item: RecentScript): string {
    return groupNameFor(item.partId, dash.groups) || item.groupName || "—";
  }
</script>

{#if items.length > 0}
  <header class="section-head">
    <h2>{mode === "recent" ? t("overview.recentlyRun") : t("overview.recentStopped")}</h2>
    <button class="btn ghost" type="button" onclick={onClearAll}>{t("overview.clearAll")}</button>
  </header>
  <div class="table-wrap">
    <table class="run-table compact">
      <thead>
        <tr>
          <th>{t("overview.col.project")}</th>
          <th>{t("overview.col.app")}</th>
          <th>{t("overview.col.script")}</th>
          {#if mode === "stopped"}
            <th>{t("overview.col.status")}</th>
            <th>{t("overview.col.ended")}</th>
          {:else}
            <th>{t("overview.col.ended")}</th>
          {/if}
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#if mode === "recent"}
          {#each visible as item (`${item.partId}::${item.script}::${item.at}`)}
            <tr>
              <td class="muted">{recentGroupName(item)}</td>
              <td>{recentPartName(item)}</td>
              <td><span class="badge quiet">{item.script}</span></td>
              <td class="muted">{t("overview.ago", { time: formatUptime(item.at, now) })}</td>
              <td class="row-actions">
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={startBlockReason(item.partId, item.script) ?? `${t("script.start")} ${item.script}`}
                  disabled={isPending(item.partId, item.script) || Boolean(startBlockReason(item.partId, item.script))}
                  onclick={(event) => {
                    event.stopPropagation();
                    startScript(item.partId, item.script, dash.groups);
                  }}
                >
                  <Icon name="play" size={12} />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={t("overview.clearItem")}
                  onclick={() => onRemoveRecent?.(item.partId, item.script)}
                >×</button>
              </td>
            </tr>
          {/each}
        {:else}
          {#each visible as run (run.id)}
            <tr>
              <td class="muted">{groupNameFor(run.partId, dash.groups) || "—"}</td>
              <td>{partLabel(run)}</td>
              <td><span class="badge quiet">{run.script}</span></td>
              <td>
                <span
                  class="log-status"
                  class:status-failed={run.status === "failed"}
                  class:status-exited={run.status === "exited"}
                  title={runExitLabel(run, t)}
                >
                  {runExitLabel(run, t)}
                </span>
              </td>
              <td class="muted">{t("overview.ago", { time: formatUptime(run.endedAt, now) })}</td>
              <td class="row-actions">
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={t("overview.logs")}
                  onclick={(event) => {
                    event.stopPropagation();
                    showLogs(run);
                  }}
                >
                  <Icon name="logs" />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={startBlockReason(run.partId, run.script) ?? t("overview.startAgain")}
                  disabled={isPending(run.partId, run.script) || Boolean(startBlockReason(run.partId, run.script))}
                  onclick={(event) => {
                    event.stopPropagation();
                    startScript(run.partId, run.script, dash.groups);
                  }}
                >
                  <Icon name="play" size={12} />
                </button>
                <button
                  class="icon-btn ghost"
                  type="button"
                  title={t("overview.clearItem")}
                  onclick={() => onDismissStopped?.(run)}
                >×</button>
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
  {#if hiddenCount > 0}
    <div class="show-more-row">
      <button class="btn ghost" type="button" onclick={() => (expanded = !expanded)}>
        {expanded ? t("overview.showLess") : t("overview.showMore", { count: hiddenCount })}
      </button>
    </div>
  {/if}
{/if}

<style>
  .show-more-row {
    display: flex;
    justify-content: flex-start;
    margin-top: 8px;
  }
</style>
