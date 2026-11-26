<script lang="ts">
  import { groupDatabases, isJsLike, shortPath, warningsForPart } from "$lib/format";
  import { composeFor, dash, groupMarker as markerForGroup, removedPartsFor, visibleWarnings } from "$lib/dashboard.svelte";
  import { t } from "$lib/i18n.svelte";
  import { aiSignalsForProject } from "$lib/models.svelte";
  import { runState } from "$lib/runs.svelte";
  import type { RepoGroup as RepoGroupType } from "$lib/types";
  import AppRow from "./AppRow.svelte";
  import ComposeSection from "./ComposeSection.svelte";
  import AiBadge from "./AiBadge.svelte";
  import DepsBadge from "./DepsBadge.svelte";
  import DbBadge from "./DbBadge.svelte";
  import TechBadge from "./TechBadge.svelte";
  import WarningBadge from "./WarningBadge.svelte";

  let {
    group,
    open,
    ontoggle,
  }: {
    group: RepoGroupType;
    open: boolean;
    ontoggle: () => void;
  } = $props();

  const running = $derived(
    runState.runs.filter(
      (run) => run.status === "running" && (group.parts ?? []).some((part) => part.id === run.partId),
    ),
  );
  const needsInstall = $derived(
    (group.parts ?? []).some((item) => isJsLike(item.kind) && !item.nodeModules),
  );
  const versions = $derived(
    [...new Set((group.parts ?? []).map((part) => part.version).filter((value): value is string => Boolean(value)))],
  );
  const groupMarker = $derived(markerForGroup(group.id));
  const removedParts = $derived(removedPartsFor(group.id));
  const warnings = $derived(
    visibleWarnings(
      (group.parts ?? []).flatMap((part) =>
        warningsForPart(part.id, runState.runs, [...(runState.warnings ?? []), ...(dash.warnings ?? [])]),
      ),
    ),
  );
  const compose = $derived(composeFor(group.id));
  const databases = $derived(groupDatabases(group));
  const aiSignals = $derived(aiSignalsForProject({ group }));
  const techKind = $derived(
    group.kind === "swift" || (group.parts ?? []).some((part) => part.kind === "swift")
      ? "swift"
      : group.kind,
  );
</script>

<section class="project" class:open class:live={running.length > 0} class:removed={groupMarker === "removed"}>
  <button class="project-row" type="button" onclick={ontoggle} aria-expanded={open}>
    <span class="chevron" class:open aria-hidden="true"></span>
    <div class="project-id">
      <div class="title-row">
        <h2 title={group.name}>{group.name}</h2>
        {#if running.length > 0}
          <span class="pulse" title={t("projects.runningLabel")} aria-label={t("projects.runningLabel")}></span>
          <span class="run-count">{running.length}</span>
        {/if}
        <!-- Tag order: tech (Swift) → AI → DB → deps → warning (last). -->
        <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
          <TechBadge kind={techKind} />
        </span>
        <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
          <AiBadge signals={aiSignals} />
        </span>
        {#if databases.length}
          <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
            <DbBadge {databases} />
          </span>
        {/if}
        {#if needsInstall}
          {@const missing = (group.parts ?? []).find((item) => isJsLike(item.kind) && !item.nodeModules)}
          {#if missing}
            <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
              <DepsBadge part={missing} />
            </span>
          {/if}
        {/if}
        <span onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()} role="presentation">
          <WarningBadge {warnings} />
        </span>
        {#if groupMarker === "new"}<span class="marker new">{t("common.new")}</span>{/if}
        {#if groupMarker === "removed"}<span class="marker removed">{t("common.removed")}</span>{/if}
      </div>
      <p class="path" title={group.path}>{shortPath(group.path)}</p>
    </div>
    <div class="project-meta">
      {#if group.git}
        <span class="git" title={group.git.dirty ? t("projects.dirty") : t("projects.clean")}>
          <span class="dot {group.git.dirty ? 'dirty' : 'clean'}"></span>
          {group.git.branch}
        </span>
      {:else}
        <span class="muted">{t("projects.noGit")}</span>
      {/if}
      {#if versions[0]}
        <span class="badge quiet">v{versions[0]}</span>
      {/if}
    </div>
  </button>

  <div class="reveal" class:open>
    <div class="reveal-inner">
      <div class="app-list">
        {#each (group.parts ?? []) as part (part.id)}
          <AppRow {part} />
        {/each}
        {#each removedParts as item (item.partId)}
          <div class="app-row removed">
            <span class="role"></span>
            <div class="app-main">
              <div class="app-id">
                <div class="app-name">
                  <strong>{item.part ?? item.partId}</strong>
                  <span class="marker removed">{t("common.removed")}</span>
                </div>
              </div>
            </div>
            <span class="port-cell"></span>
            <div class="app-actions"></div>
            <div class="app-status"></div>
          </div>
        {/each}
      </div>
      {#if compose}
        <ComposeSection {compose} />
      {/if}
    </div>
  </div>
</section>
