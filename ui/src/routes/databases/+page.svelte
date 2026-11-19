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
    return `${(db.engine || "").toLowerCase()}|${db.port ?? ""}|${(db.path || "").toLowerCase()}|${index}`;
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

    <div class="db-table">
      <div class="db-head" role="row">
        <button type="button" class="sort-btn" class:active={sortKey === "project"} onclick={() => toggleSort("project")}>
          {t("databases.col.project")}{sortIndicator("project")}
        </button>
        <button type="button" class="sort-btn num" class:active={sortKey === "size"} onclick={() => toggleSort("size")}>
          {t("databases.col.size")}{sortIndicator("size")}
        </button>
        <span>{t("databases.col.detail")}</span>
      </div>
      <ul class="db-list">
        {#each buckets as bucket (bucket.id)}
          <li class="db-group">
            <div class="db-project">
              <span class="project-name" title={bucket.path}>{bucket.name}</span>
              <span class="num tabular">{formatSize(bucket.totalBytes)}</span>
              <span class="muted">{bucket.databases.length}</span>
            </div>
            <ul class="db-children">
              {#each bucket.databases as db (db.key)}
                <li class="db-child">
                  <span class="engine">{databaseEngineLabel(db.engine)}</span>
                  <span class="num tabular">{formatSize(sizeOf(db))}</span>
                  <span class="detail muted" title={detailFor(db)}>
                    <em>{databaseSourceLabel(db.source)}</em>
                    {detailFor(db)}
                  </span>
                </li>
              {/each}
            </ul>
          </li>
        {/each}
      </ul>
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
  .db-table {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--card);
  }
  .db-head,
  .db-project,
  .db-child {
    display: grid;
    grid-template-columns: minmax(8rem, 1.6fr) max-content minmax(8rem, 1.4fr);
    gap: 10px;
    align-items: center;
  }
  .db-head {
    padding: 4px 6px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .sort-btn {
    margin: 0;
    padding: 4px 6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    text-align: left;
    cursor: pointer;
  }
  .sort-btn.num { text-align: right; }
  .sort-btn:hover,
  .sort-btn.active { color: var(--text); background: rgba(255, 255, 255, 0.04); }
  .db-list, .db-children {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .db-group + .db-group { border-top: 1px solid rgba(255, 255, 255, 0.04); }
  .db-project {
    width: 100%;
    padding: 9px 12px;
    color: inherit;
  }
  .project-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num { text-align: right; white-space: nowrap; }
  .db-children { padding: 0 12px 8px 28px; }
  .db-child {
    padding: 5px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.03);
    font-size: 12px;
  }
  .engine { font-weight: 600; }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: flex;
    gap: 8px;
    min-width: 0;
  }
  .detail em {
    font-style: normal;
    color: var(--muted);
    flex: none;
  }
  .db-skel { height: 8rem; }
  @media (max-width: 800px) {
    .db-head { display: none; }
    .db-project, .db-child { grid-template-columns: 1fr auto; }
    .detail { grid-column: 1 / -1; }
  }
</style>
