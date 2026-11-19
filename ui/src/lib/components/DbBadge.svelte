<script lang="ts">
  import { databaseEngineLabel, databaseSourceLabel, databaseSummary, partDatabases } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import type { DatabaseInfo, ProjectPart } from "$lib/types";
  import Icon from "./Icon.svelte";

  let {
    part = null,
    databases = null,
  }: {
    part?: ProjectPart | null;
    databases?: DatabaseInfo[] | null;
  } = $props();

  const items = $derived(
    (databases && databases.length ? databases : part ? partDatabases(part) : []).slice(0, 6),
  );
  const engines = $derived(
    [...new Set(items.map((db) => databaseEngineLabel(db.engine)))].slice(0, 3),
  );
  const label = $derived(
    engines.length === 0
      ? t("db.badge")
      : engines.length === 1
        ? engines[0]
        : `${engines[0]}+${engines.length - 1}`,
  );
  const title = $derived(
    items.length
      ? items.map((db) => databaseSummary(db)).join(", ")
      : t("db.badge"),
  );

  function lineFor(db: DatabaseInfo): string {
    const parts = [databaseEngineLabel(db.engine)];
    if (db.port != null) parts.push(`:${db.port}`);
    if (db.path) parts.push(db.path);
    const source = databaseSourceLabel(db.source);
    if (source) parts.push(`· ${source}`);
    if (db.detail) parts.push(`(${db.detail})`);
    return parts.join(" ");
  }
</script>

{#if items.length}
  <span class="warn-badge info db-badge" role="note" {title} aria-label={title}>
    <Icon name="db" size={11} />
    {label}
    <span class="warn-popover" role="tooltip">
      <strong>{t("db.title")}</strong>
      <ul class="db-list">
        {#each items as db, index (`${db.engine}-${db.port ?? ""}-${db.path ?? ""}-${index}`)}
          <li title={lineFor(db)}>
            <span class="db-engine">{databaseEngineLabel(db.engine)}</span>
            {#if db.port != null}
              <span class="db-port">:{db.port}</span>
            {:else if db.path}
              <span class="db-path">{db.path}</span>
            {/if}
            <span class="db-source">{databaseSourceLabel(db.source)}</span>
          </li>
        {/each}
      </ul>
    </span>
  </span>
{/if}

<style>
  .db-badge strong {
    display: block;
    font-size: 12px;
    margin-bottom: 6px;
  }
  .db-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .db-list li {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 12px;
    line-height: 1.35;
    min-width: 0;
  }
  .db-engine {
    font-weight: 600;
    flex: 0 0 auto;
  }
  .db-port {
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .db-path {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .db-source {
    margin-left: auto;
    color: var(--muted);
    font-size: 11px;
    flex: 0 0 auto;
  }
</style>
