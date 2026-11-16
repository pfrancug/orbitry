<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { fmtBytes, formatCheckedAt, shortPath } from "$lib/format";
  import { i18n, t } from "$lib/i18n.svelte";
  import { models, modelsScanning, rescanModels } from "$lib/models.svelte";
  import type { ModelUsage, ModelsInventory } from "$lib/types";

  const locale = $derived(i18n.locale);
  const inventory = $derived(models.inventory);
  const scanning = $derived(modelsScanning());

  type FlatModel = {
    key: string;
    name: string;
    sizeBytes: number;
    location: string;
    path?: string | null;
    id?: string | null;
    modifiedAt?: number | null;
    usedBy?: ModelUsage[];
  };

  function flatten(inv: ModelsInventory): FlatModel[] {
    const rows: FlatModel[] = [];
    for (const model of inv.ollama?.models ?? []) {
      rows.push({
        key: `ollama:${model.name}:${model.id ?? ""}`,
        name: model.name,
        sizeBytes: model.sizeBytes ?? 0,
        location: inv.ollama?.modelsDir ? shortPath(inv.ollama.modelsDir) : "Ollama",
        path: inv.ollama?.modelsDir ?? null,
        id: model.id ?? null,
        modifiedAt: model.modifiedAt ?? null,
        usedBy: model.usedBy,
      });
    }
    for (const cache of inv.caches ?? []) {
      for (const model of cache.models ?? []) {
        rows.push({
          key: `cache:${model.path}`,
          name: model.name,
          sizeBytes: model.sizeBytes ?? 0,
          location: cache.label,
          path: model.path,
          usedBy: model.usedBy,
        });
      }
    }
    for (const project of inv.projectFiles ?? []) {
      for (const file of project.files ?? []) {
        rows.push({
          key: `project:${file.path}`,
          name: file.name,
          sizeBytes: file.sizeBytes ?? 0,
          location: project.projectName,
          path: file.path,
          usedBy: file.usedBy,
        });
      }
    }
    return rows.sort((a, b) => b.sizeBytes - a.sizeBytes || a.name.localeCompare(b.name));
  }

  const rows = $derived(inventory ? flatten(inventory) : []);
  const sourceCount = $derived.by(() => {
    if (!inventory) return 0;
    const labels = new Set<string>();
    if (inventory.ollama?.models?.length) labels.add("ollama");
    for (const cache of inventory.caches ?? []) {
      if (cache.models?.length) labels.add(cache.id);
    }
    for (const project of inventory.projectFiles ?? []) {
      if (project.files?.length) labels.add(project.projectId);
    }
    return labels.size;
  });

  let expanded = $state(new Set<string>());

  function toggle(key: string) {
    const next = new Set(expanded);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expanded = next;
  }


  function usagePaths(usages: ModelUsage[] | undefined): string[] {
    const paths: string[] = [];
    for (const usage of usages ?? []) {
      const files = (usage.files ?? []).map((file) => file.trim()).filter(Boolean);
      if (files.length) {
        for (const file of files) {
          if (!paths.includes(file)) paths.push(file);
        }
      } else if (usage.projectPath && !paths.includes(usage.projectPath)) {
        paths.push(usage.projectPath);
      }
    }
    return paths;
  }

  function countLabel(count: number, one: string, many: string): string {
    return count === 1 ? t(one) : t(many, { count });
  }

  function scanLine(): string {
    if (models.loading && !inventory) return t("models.loading");
    if (scanning) {
      return inventory?.scan?.scannedAt
        ? t("models.scanProgress", { time: formatCheckedAt(inventory.scan.scannedAt) })
        : t("models.scanning");
    }
    if (inventory?.scan?.error) return t("models.scanFailed", { error: inventory.scan.error });
    if (inventory?.scan?.scannedAt) {
      return inventory.scan.stale
        ? t("models.lastScanStale", { time: formatCheckedAt(inventory.scan.scannedAt) })
        : t("models.lastScan", { time: formatCheckedAt(inventory.scan.scannedAt) });
    }
    return t("models.subtitle");
  }
</script>

<div class="page-head" data-locale={locale}>
  <div>
    <h1>{t("models.title")}</h1>
    <p>{scanLine()}</p>
  </div>
  <div class="page-actions">
    <button
      class="btn"
      type="button"
      disabled={scanning || models.loading}
      onclick={() => rescanModels()}
    >
      <Icon name="refresh" size={14} />
      {scanning ? t("models.scanning") : t("models.scanAgain")}
    </button>
  </div>
</div>

{#if inventory?.scan?.error}
  <p class="hint error">{t("models.scanFailed", { error: inventory.scan.error })}</p>
{/if}
{#if models.error && !inventory}
  <div class="empty-card">
    <strong>{t("models.loadError")}</strong>
    <p>{models.error}</p>
  </div>
{:else if models.loading && !inventory}
  <div class="models-page">
    <div class="skeleton models-skel"></div>
  </div>
{:else if inventory}
  <div class="models-page">
    <section class="models-summary" aria-label={t("models.totals")}>
      <div>
        <strong>{countLabel(rows.length, "models.summaryCountOne", "models.summaryCount")}</strong>
      </div>
      <div>
        <strong>{fmtBytes(inventory.grandTotalBytes)}</strong>
        <span class="muted">{t("models.together")}</span>
      </div>
      <div>
        <strong>{countLabel(sourceCount, "models.summarySourcesOne", "models.summarySources")}</strong>
      </div>
    </section>

    {#if rows.length}
      <ul class="model-list">
        {#each rows as row (row.key)}
          {@const open = expanded.has(row.key)}
          <li class="model-item" class:open>
            <button
              class="model-line"
              type="button"
              aria-expanded={open}
              title={open ? t("models.collapse") : t("models.expand")}
              onclick={() => toggle(row.key)}
            >
              <span class="model-name" title={row.name}>{row.name}</span>
              <span class="num tabular">{fmtBytes(row.sizeBytes)}</span>
              <span class="muted loc" title={row.path ?? ""}>{row.location}</span>
              <span class="used">
                {#if usagePaths(row.usedBy).length}
                  {#each usagePaths(row.usedBy) as path (path)}
                    <span class="path-chip" title={path}>{path}</span>
                  {/each}
                {:else}
                  <span class="muted">{t("models.usedByNone")}</span>
                {/if}
              </span>
            </button>
            {#if open}
              <dl class="model-details">
                {#if row.path}
                  <div>
                    <dt>{t("models.location")}</dt>
                    <dd class="mono" title={row.path}>{row.path}</dd>
                  </div>
                {/if}
                {#if row.id}
                  <div>
                    <dt>{t("models.col.id")}</dt>
                    <dd class="mono">{row.id}</dd>
                  </div>
                {/if}
                {#if row.modifiedAt}
                  <div>
                    <dt>{t("models.col.modified")}</dt>
                    <dd>{formatCheckedAt(row.modifiedAt)}</dd>
                  </div>
                {/if}
              </dl>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">{t("models.empty")}</p>
    {/if}
  </div>
{/if}

<style>
  .models-page {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 2rem;
  }
  .models-summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--card);
  }
  .models-summary strong { display: block; font-size: 0.95rem; }
  .muted { color: var(--muted); font-size: 12px; }
  .model-list {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--card);
    overflow: visible;
  }
  .model-item + .model-item { border-top: 1px solid rgba(255, 255, 255, 0.04); }
  .model-line {
    display: grid;
    grid-template-columns: minmax(8rem, 1.4fr) max-content minmax(6rem, 1fr) minmax(7rem, 1.2fr);
    gap: 10px;
    align-items: center;
    width: 100%;
    padding: 9px 12px;
    background: transparent;
    border: 0;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .model-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num { text-align: right; white-space: nowrap; }
  .loc { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .used { display: flex; flex-wrap: wrap; gap: 4px; justify-content: flex-end; min-width: 0; }
  .path-chip {
    display: block;
    max-width: 100%;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.04);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 11px;
    font-family: var(--mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
  }
  .model-details {
    margin: 0;
    padding: 0 12px 12px 12px;
    display: grid;
    gap: 6px;
  }
  .model-details div {
    display: grid;
    grid-template-columns: 8rem minmax(0, 1fr);
    gap: 8px;
    font-size: 12px;
  }
  dt { color: var(--muted); margin: 0; }
  dd { margin: 0; overflow-wrap: anywhere; }
  .mono { font-family: var(--mono); font-size: 12px; }
  .models-skel { height: 8rem; }
  .error { color: var(--danger); }
  @media (max-width: 800px) {
    .models-summary, .model-line { grid-template-columns: 1fr 1fr; }
    .model-details div { grid-template-columns: 1fr; }
  }
</style>
