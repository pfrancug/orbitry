<script lang="ts">
  import { dash } from "$lib/dashboard.svelte";
  import {
    databaseEngineLabel,
    databaseSourceLabel,
    fmtBytes,
    groupDatabases,
  } from "$lib/format";
  import { i18n, t } from "$lib/i18n.svelte";
  import type { DatabaseInfo, RepoGroup } from "$lib/types";

  const locale = $derived(i18n.locale);

  type DbRow = DatabaseInfo & { key: string };
  type ProjectBucket = {
    id: string;
    name: string;
    path: string;
    databases: DbRow[];
    totalBytes: number | null;
  };

  type SortKey = "project" | "size";
  type SortDir = "asc" | "desc";

  let sortKey = $state<SortKey>("project");
  let sortDir = $state<SortDir>("asc");

  function dbKey(db: DatabaseInfo, index: number): string {
    return `${(db.engine || "").toLowerCase()}|${db.port ?? ""}|${(db.path || "").toLowerCase()}|${db.host ?? ""}|${index}`;
  }

  function sizeOf(db: DatabaseInfo): number | null {
    return typeof db.sizeBytes === "number" && Number.isFinite(db.sizeBytes) ? db.sizeBytes : null;
  }

  function formatSize(bytes: number | null | undefined): string {
    if (bytes == null) return "—";
    return fmtBytes(bytes);
  }

  function buildBuckets(groups: RepoGroup[]): ProjectBucket[] {
    const buckets: ProjectBucket[] = [];
    for (const group of groups) {
      const dbs = groupDatabases(group);
      if (!dbs.length) continue;
      let total: number | null = 0;
      const rows: DbRow[] = dbs.map((db, index) => {
        const size = sizeOf(db);
        if (size == null) total = null;
        else if (total != null) total += size;
        return { ...db, key: dbKey(db, index) };
      });
      buckets.push({
        id: group.id,
        name: group.name,
        path: group.path,
        databases: rows,
        totalBytes: total,
      });
    }
    return buckets;
  }

  function compare(a: ProjectBucket, b: ProjectBucket): number {
    const dir = sortDir === "asc" ? 1 : -1;
    if (sortKey === "size") {
      const av = a.totalBytes ?? -1;
      const bv = b.totalBytes ?? -1;
      return (av - bv) * dir || a.name.localeCompare(b.name);
    }
    return a.name.localeCompare(b.name) * dir;
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

  function detailFor(db: DatabaseInfo): string {
    if (db.port != null) return `:${db.port}`;
    if (db.path) return db.path;
    if (db.host) return db.host;
    return databaseSourceLabel(db.source) || "—";
  }

  const buckets = $derived(buildBuckets(dash.groups).sort(compare));
  const dbCount = $derived(buckets.reduce((sum, item) => sum + item.databases.length, 0));
</script>

<div class="page-head" data-locale={locale}>
  <div>
    <h1>{t("databases.title")}</h1>
    <p>{t("databases.subtitle")}</p>
  </div>
</div>

{#if dash.loading && !dash.groups.length}
  <div class="skeleton db-skel"></div>
{:else if !buckets.length}
  <p class="muted">{t("databases.empty")}</p>
{:else}
  <div class="db-page">
    <section class="db-summary" aria-label={t("databases.totals")}>
      <div>
        <strong>{buckets.length}</strong>
        <span class="muted">{buckets.length === 1 ? t("databases.card.project") : t("databases.card.projects")}</span>
      </div>
      <div>
        <strong>{dbCount}</strong>
        <span class="muted">{dbCount === 1 ? t("databases.card.database") : t("databases.card.databases")}</span>
      </div>
    </section>

    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>
              <button type="button" class="sort-btn" class:active={sortKey === "project"} onclick={() => toggleSort("project")}>
                {t("databases.col.project")}{sortIndicator("project")}
              </button>
            </th>
            <th class="num">
              <button type="button" class="sort-btn num" class:active={sortKey === "size"} onclick={() => toggleSort("size")}>
                {t("databases.col.size")}{sortIndicator("size")}
              </button>
            </th>
            <th>{t("databases.col.detail")}</th>
          </tr>
        </thead>
        <tbody>
          {#each buckets as bucket (bucket.id)}
            <tr class="project-row">
              <td class="project-name" title={bucket.path}>{bucket.name}</td>
              <td class="num tabular">{formatSize(bucket.totalBytes)}</td>
              <td class="muted">{bucket.databases.length}</td>
            </tr>
            {#each bucket.databases as db (db.key)}
              <tr class="child-row">
                <td class="engine">{databaseEngineLabel(db.engine)}</td>
                <td class="num tabular">{formatSize(sizeOf(db))}</td>
                <td class="detail muted" title={detailFor(db)}>
                  <em>{databaseSourceLabel(db.source)}</em>
                  <span>{detailFor(db)}</span>
                </td>
              </tr>
            {/each}
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}

<style>
  .db-page {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 2rem;
  }
  .db-summary {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--card);
  }
  .db-summary strong {
    display: block;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.15;
  }
  .db-summary .muted {
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
    overflow: auto;
    max-height: min(70vh, 40rem);
  }
  .data-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  .data-table th,
  .data-table td {
    padding: 8px 12px;
    text-align: left;
    vertical-align: middle;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .data-table th:nth-child(1),
  .data-table td:nth-child(1) { width: 42%; }
  .data-table th:nth-child(2),
  .data-table td:nth-child(2) { width: 7.5rem; }
  .data-table th:nth-child(3),
  .data-table td:nth-child(3) { width: auto; }
  .data-table thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: color-mix(in srgb, var(--card) 92%, #000);
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
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
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .project-row td {
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-weight: 600;
  }
  .project-row:first-child td { border-top: 0; }
  .child-row td {
    border-top: 1px solid rgba(255, 255, 255, 0.03);
    font-size: 12px;
    font-weight: 400;
  }
  .child-row td:first-child {
    padding-left: 28px;
  }
  .engine { font-weight: 600; }
  .detail {
    display: flex;
    gap: 8px;
    min-width: 0;
  }
  .detail em {
    font-style: normal;
    color: var(--muted);
    flex: none;
  }
  .detail span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .db-skel { height: 8rem; }
  @media (max-width: 800px) {
    .data-table th:nth-child(3),
    .data-table td:nth-child(3) { display: none; }
  }
</style>
