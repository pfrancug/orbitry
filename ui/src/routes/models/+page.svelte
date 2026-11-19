<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { dash } from "$lib/dashboard.svelte";
  import { fmtBytes, formatCheckedAt, scannedProjectNameForUsage, shortPath } from "$lib/format";
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

  type SortKey = "name" | "size" | "location" | "usedBy";
  type SortDir = "asc" | "desc";

  let sortKey = $state<SortKey>("name");
  let sortDir = $state<SortDir>("asc");

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
    return rows;
  }

  function usageNames(usages: ModelUsage[] | undefined): string[] {
    const names: string[] = [];
    for (const usage of usages ?? []) {
      const name = scannedProjectNameForUsage(usage, dash.groups).trim();
      if (name && !names.includes(name)) names.push(name);
    }
    return names;
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

  function compare(a: FlatModel, b: FlatModel): number {
    const dir = sortDir === "asc" ? 1 : -1;
    if (sortKey === "size") {
      return (a.sizeBytes - b.sizeBytes) * dir || a.name.localeCompare(b.name);
    }
    if (sortKey === "location") {
      return a.location.localeCompare(b.location) * dir || a.name.localeCompare(b.name);
    }
    if (sortKey === "usedBy") {
      const an = usageNames(a.usedBy).join(", ");
      const bn = usageNames(b.usedBy).join(", ");
      return an.localeCompare(bn) * dir || a.name.localeCompare(b.name);
    }
    return a.name.localeCompare(b.name) * dir || b.sizeBytes - a.sizeBytes;
  }

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortKey = key;
      sortDir = key === "size" ? "desc" : "asc";
    }
  }

  function sortIndicator(key: SortKey): string {
    if (sortKey !== key) return "";
    return sortDir === "asc" ? " ↑" : " ↓";
  }

  const rows = $derived(inventory ? flatten(inventory).sort(compare) : []);
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
        <strong>{rows.length}</strong>
        <span class="muted">{rows.length === 1 ? t("models.card.model") : t("models.card.models")}</span>
      </div>
      <div>
        <strong>{fmtBytes(inventory.grandTotalBytes)}</strong>
        <span class="muted">{t("models.card.total")}</span>
      </div>
      <div>
        <strong>{sourceCount}</strong>
        <span class="muted">{sourceCount === 1 ? t("models.card.source") : t("models.card.sources")}</span>
      </div>
    </section>

    {#if rows.length}
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>
                <button type="button" class="sort-btn" class:active={sortKey === "name"} onclick={() => toggleSort("name")}>
                  {t("models.col.model")}{sortIndicator("name")}
                </button>
              </th>
              <th class="num">
                <button type="button" class="sort-btn num" class:active={sortKey === "size"} onclick={() => toggleSort("size")}>
                  {t("models.col.size")}{sortIndicator("size")}
                </button>
              </th>
              <th>
                <button type="button" class="sort-btn" class:active={sortKey === "location"} onclick={() => toggleSort("location")}>
                  {t("models.col.location")}{sortIndicator("location")}
                </button>
              </th>
              <th>
                <button type="button" class="sort-btn" class:active={sortKey === "usedBy"} onclick={() => toggleSort("usedBy")}>
                  {t("models.col.usedBy")}{sortIndicator("usedBy")}
                </button>
              </th>
            </tr>
          </thead>
          <tbody>
            {#each rows as row (row.key)}
              {@const open = expanded.has(row.key)}
              {@const names = usageNames(row.usedBy)}
              {@const paths = usagePaths(row.usedBy)}
              <tr class="model-row" class:open>
                <td>
                  <button
                    class="name-btn"
                    type="button"
                    aria-expanded={open}
                    title={open ? t("models.collapse") : t("models.expand")}
                    onclick={() => toggle(row.key)}
                  >
                    <span class="model-name" title={row.name}>{row.name}</span>
                  </button>
                </td>
                <td class="num tabular">{fmtBytes(row.sizeBytes)}</td>
                <td class="muted loc" title={row.path ?? ""}>{row.location}</td>
                <td>
                  <span class="used">
                    {#if names.length}
                      {#each names as name (name)}
                        <span class="path-chip" title={name}>{name}</span>
                      {/each}
                    {:else}
                      <span class="muted">{t("models.usedByNone")}</span>
                    {/if}
                  </span>
                </td>
              </tr>
              {#if open}
                <tr class="details-row">
                  <td colspan="4">
                    <dl class="model-details">
                      {#if row.path}
                        <div>
                          <dt>{t("models.location")}</dt>
                          <dd class="mono" title={row.path}>{row.path}</dd>
                        </div>
                      {/if}
                      {#if paths.length}
                        <div>
                          <dt>{t("models.usedPaths")}</dt>
                          <dd>
                            <ul class="used-paths">
                              {#each paths as path (path)}
                                <li class="mono" title={path}>{path}</li>
                              {/each}
                            </ul>
                          </dd>
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
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      </div>
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
  .models-summary strong {
    display: block;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.15;
  }
  .models-summary .muted {
    display: block;
    margin-top: 2px;
    font-size: 12px;
    text-transform: lowercase;
  }
  .muted { color: var(--muted); font-size: 12px; }
  .table-wrap {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--card);
    overflow: visible;
  }
  .data-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  .data-table thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 8px 12px;
    background: color-mix(in srgb, var(--card) 92%, #000);
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    text-align: left;
  }
  .data-table thead th:nth-child(1) { width: 32%; }
  .data-table thead th:nth-child(2) { width: 7.5rem; }
  .data-table thead th:nth-child(3) { width: 28%; }
  .data-table thead th:nth-child(4) { width: auto; }
  .sort-btn {
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .sort-btn.num { width: 100%; text-align: right; }
  .sort-btn:hover,
  .sort-btn.active { color: var(--text); }
  .data-table tbody td {
    padding: 8px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    vertical-align: middle;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .model-row:first-child td { border-top: 0; }
  .name-btn {
    display: block;
    width: 100%;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  .model-name {
    display: block;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num { text-align: right; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .loc { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .used { display: flex; flex-wrap: wrap; gap: 4px; justify-content: flex-end; min-width: 0; }
  .details-row td {
    white-space: normal;
    border-top: 0;
    padding-top: 0;
  }
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
  .used-paths {
    margin: 0;
    padding-left: 1.1rem;
    display: grid;
    gap: 4px;
  }
  .models-skel { height: 8rem; }
  .error { color: var(--danger); }
  @media (max-width: 800px) {
    .models-summary { grid-template-columns: 1fr 1fr; }
    .data-table thead { display: none; }
    .model-details div { grid-template-columns: 1fr; }
  }
</style>
