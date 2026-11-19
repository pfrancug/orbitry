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

  function locationFor(db: DatabaseInfo): string {
    if (db.port != null) return db.host ? `${db.host}:${db.port}` : `:${db.port}`;
    if (db.path) return db.path;
    if (db.host) return db.host;
    return "—";
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

    <div class="grid-table data-table" role="table" aria-label={t("databases.title")}>
      <div class="grid-head" role="row">
        <div role="columnheader">
          <button type="button" class="sort-btn" class:active={sortKey === "project"} onclick={() => toggleSort("project")}>
            {t("databases.col.project")}{sortIndicator("project")}
          </button>
        </div>
        <div class="num" role="columnheader">
          <button type="button" class="sort-btn num" class:active={sortKey === "size"} onclick={() => toggleSort("size")}>
            {t("databases.col.size")}{sortIndicator("size")}
          </button>
        </div>
        <div role="columnheader">{t("databases.col.source")}</div>
        <div role="columnheader">{t("databases.col.location")}</div>
      </div>
      {#each buckets as bucket (bucket.id)}
        <div class="grid-row project" role="row">
          <div class="cell name project-name" role="cell" title={bucket.path}>
            <span class="project-icon" aria-hidden="true">▦</span>
            <strong>{bucket.name}</strong>
            <span class="project-count">
              {bucket.databases.length}
              {bucket.databases.length === 1 ? t("databases.card.database") : t("databases.card.databases")}
            </span>
          </div>
          <div class="cell num tabular" role="cell">{formatSize(bucket.totalBytes)}</div>
          <div class="cell muted" role="cell">—</div>
          <div class="cell muted" role="cell">—</div>
        </div>
        {#each bucket.databases as db (db.key)}
          <div class="grid-row child" role="row">
            <div class="cell name child-name" role="cell">{databaseEngineLabel(db.engine)}</div>
            <div class="cell num tabular" role="cell">{formatSize(sizeOf(db))}</div>
            <div class="cell muted" role="cell">{databaseSourceLabel(db.source) || "—"}</div>
            <div class="cell loc muted" role="cell" title={locationFor(db)}>{locationFor(db)}</div>
          </div>
        {/each}
      {/each}
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
  .grid-table {
    --cols: minmax(10rem, 1.35fr) 6.75rem 5.5rem minmax(8rem, 1.4fr);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--card);
    overflow: hidden;
  }
  .grid-head,
  .grid-row {
    display: grid;
    grid-template-columns: var(--cols);
    column-gap: 12px;
    align-items: center;
    padding: 0 12px;
    min-width: 0;
  }
  .grid-head {
    position: sticky;
    top: 0;
    z-index: 2;
    min-height: 34px;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--card) 94%, #000);
    color: var(--muted);
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .grid-row {
    min-height: 34px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
  }
  .grid-row.project {
    min-height: 38px;
    border-top-color: rgba(255, 255, 255, 0.1);
    background: color-mix(in srgb, var(--panel, #15161c) 72%, #000);
  }
  .grid-row.project:first-of-type,
  .grid-head + .grid-row { border-top: 0; }
  .grid-row.child {
    background: transparent;
  }
  .cell {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cell.name { font-weight: 500; }
  .project-name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .project-name strong {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .project-icon {
    flex: none;
    width: 1.1rem;
    text-align: center;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
  }
  .project-count {
    flex: none;
    margin-left: auto;
    color: var(--muted);
    font-size: 11px;
    font-weight: 550;
  }
  .child-name {
    font-weight: 500;
    padding-left: 1.85rem;
    color: var(--text);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .loc { font-family: var(--mono); font-size: 12px; }
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
  .db-skel { height: 8rem; }
  @media (max-width: 900px) {
    .grid-table { --cols: minmax(8rem, 1.2fr) 5.5rem minmax(6rem, 1fr); }
    .grid-head > :nth-child(3),
    .grid-row > :nth-child(3) { display: none; }
  }
</style>
