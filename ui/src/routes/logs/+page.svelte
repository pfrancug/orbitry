<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import LogViewer from "$lib/components/LogViewer.svelte";
  import { dash } from "$lib/dashboard.svelte";
  import { findPart, groupNameFor, partTitle } from "$lib/format";
  import { i18n, t } from "$lib/i18n.svelte";
  import { openRun, runState } from "$lib/runs.svelte";

  const locale = $derived(i18n.locale);
  const requested = $derived(page.url.searchParams.get("run"));

  const tabs = $derived.by(() => {
    const running = runState.runs.filter((run) => run.status === "running");
    const selected = runState.runs.find((run) => run.id === runState.selectedId);
    if (selected && !running.some((run) => run.id === selected.id)) {
      return [...running, selected];
    }
    return running.length > 0 ? running : runState.runs.slice(0, 8);
  });

  const activeId = $derived(runState.selectedId ?? tabs[0]?.id ?? null);

  $effect(() => {
    if (requested && runState.runs.some((run) => run.id === requested) && runState.selectedId !== requested) {
      openRun(requested, false);
    }
  });

  function choose(id: string) {
    openRun(id, false);
    void goto(`/logs?run=${encodeURIComponent(id)}`, { replaceState: true, keepFocus: true, noScroll: true });
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
      {/each}
    </div>
    <LogViewer runId={activeId} />
  </div>
{/if}
