<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import LogViewer from "$lib/components/LogViewer.svelte";
  import { dash } from "$lib/dashboard.svelte";
  import { findPart, groupNameFor, partTitle } from "$lib/format";
  import { i18n, t } from "$lib/i18n.svelte";
  import { closeLogTab, logTabRuns, openRun, pinLogTab, runState } from "$lib/runs.svelte";

  const locale = $derived(i18n.locale);
  const requested = $derived(page.url.searchParams.get("run"));

  const tabs = $derived(logTabRuns());

  const activeId = $derived(runState.selectedId ?? tabs[0]?.id ?? null);

  $effect(() => {
    if (requested && runState.runs.some((run) => run.id === requested)) {
      pinLogTab(requested);
      if (runState.selectedId !== requested) {
        openRun(requested, false);
      }
    }
  });

  function choose(id: string) {
    openRun(id, false);
    void goto(`/logs?run=${encodeURIComponent(id)}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  function closeTab(id: string, event: MouseEvent) {
    event.stopPropagation();
    event.preventDefault();
    closeLogTab(id);
    const next = logTabRuns();
    if (next[0]) {
      void goto(`/logs?run=${encodeURIComponent(next[0].id)}`, { replaceState: true, keepFocus: true, noScroll: true });
    } else {
      void goto("/logs", { replaceState: true, keepFocus: true, noScroll: true });
    }
  }

  function label(runId: string): string {
    const run = runState.runs.find((item) => item.id === runId);
    if (!run) return runId;
    const part = findPart(run.partId, dash.groups);
    const name = part ? partTitle(part) : run.partName;
    return `${name} · ${run.script}`;
  }
</script>

<div class="page-head" data-locale={locale}>
  <div>
    <h1>{t("logs.title")}</h1>
    <p>
      {#if tabs.length === 0}
        {t("logs.noneHint")}
      {:else}
        {t("logs.subtitle")}
      {/if}
    </p>
  </div>
</div>

{#if tabs.length === 0}
  <div class="empty-card">
    <strong>{t("logs.emptyTitle")}</strong>
    <p>{t("logs.emptyBody")}</p>
    <a class="btn primary" href="/projects">{t("logs.openProjects")}</a>
  </div>
{:else}
  <div class="log-page console">
    <div class="log-tabs" role="tablist">
      {#each tabs as run (run.id)}
        <div class="log-tab-wrap" class:active={run.id === activeId} class:running={run.status === "running"}>
          <button
            class="log-tab"
            class:active={run.id === activeId}
            class:running={run.status === "running"}
            type="button"
            role="tab"
            aria-selected={run.id === activeId}
            title={groupNameFor(run.partId, dash.groups)}
            onclick={() => choose(run.id)}
          >
            {#if run.status === "running"}<span class="pulse" aria-hidden="true"></span>{/if}
            <span class="tab-name">{label(run.id)}</span>
          </button>
          <button
            class="log-tab-close"
            type="button"
            title={t("logs.closeTab")}
            aria-label={t("logs.closeTab")}
            onclick={(event) => closeTab(run.id, event)}
          >
            ×
          </button>
        </div>
      {/each}
    </div>
    <LogViewer runId={activeId} />
  </div>
{/if}

<style>
  .log-tab-wrap {
    display: inline-flex;
    align-items: stretch;
    flex: 0 0 auto;
    border-right: 1px solid var(--line);
  }
  .log-tab-wrap :global(.log-tab) {
    border-right: 0;
  }
  .log-tab-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
  }
  .log-tab-wrap:hover .log-tab-close,
  .log-tab-wrap.active .log-tab-close {
    color: var(--text);
  }
  .log-tab-close:hover {
    background: rgba(255, 255, 255, 0.06);
  }
</style>
