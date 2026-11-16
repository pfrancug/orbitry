<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { formatCheckedAt, fmtBytes, shortPath } from "$lib/format";
  import { models, modelsScanning, refreshModels, rescanModels } from "$lib/models.svelte";
  import { i18n, t } from "$lib/i18n.svelte";
  const locale = $derived(i18n.locale);
  import type { CacheSource, ModelUsage, OllamaInventory } from "$lib/types";

  onMount(() => {
    refreshModels();
  });

  const inventory = $derived(models.inventory);
  const scanning = $derived(modelsScanning());
  const ollama = $derived(inventory?.ollama);
  const totals = $derived(inventory?.totals ?? []);
  const caches = $derived(inventory?.caches ?? []);
  const projects = $derived(inventory?.projectFiles ?? []);

  function usedByLabel(usages: ModelUsage[] | undefined): string {
    if (!usages?.length) return "—";
    return usages.map((item) => item.projectName).join(", ");
  }

  function usedByTitle(usages: ModelUsage[] | undefined): string {
    if (!usages?.length) return "";
    return usages
      .map((item) => {
        const files = (item.files ?? []).map((file) => shortPath(file)).join(", ");
        return files ? `${item.projectName}: ${files}` : item.projectName;
      })
      .join("\n");
  }

  function ollamaStatus(info: OllamaInventory | null | undefined): string {
    if (!info) return "";
    const bits: string[] = [];
    if (info.installed) {
      bits.push(info.version ? `Ollama ${info.version}` : "Ollama jest zainstalowana");
    } else {
      bits.push("Nie znaleziono instalacji Ollamy");
    }
    if (info.serviceReachable) {
      bits.push(`serwis działa (${info.serviceUrl})`);
    } else {
      bits.push("serwis nie odpowiada — lista z katalogu manifestów");
    }
    if (info.modelsDir) {
      bits.push(`katalog ${shortPath(info.modelsDir)} (${fmtBytes(info.modelsDirBytes)})`);
    }
    return bits.join(" · ");
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

  function cacheHint(cache: CacheSource): string {
    if (!cache.present) return "Brak tego cache'u na tym komputerze.";
    if (!cache.models?.length) return "Katalog jest, ale nie znaleziono modeli.";
    return `${cache.models.length} · ${fmtBytes(cache.totalBytes)}`;
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
    <div class="table-wrap"><div class="skeleton models-skel"></div></div>
  </div>
{:else if inventory}
  <div class="models-page">
    <section class="models-section">
      <header class="models-head">
        <h2>Suma</h2>
        <p class="muted">{fmtBytes(inventory.grandTotalBytes)} łącznie</p>
      </header>
      {#if totals.length}
        <div class="table-wrap">
          <table class="run-table compact">
            <thead>
              <tr>
                <th>Źródło</th>
                <th>Katalog</th>
                <th class="num">Razem</th>
              </tr>
            </thead>
            <tbody>
              {#each totals as row (row.id)}
                <tr>
                  <td>{row.label}</td>
                  <td class="muted path" title={row.path ?? ""}>{row.path ? shortPath(row.path) : "—"}</td>
                  <td class="num tabular">{fmtBytes(row.bytes)}</td>
                </tr>
              {/each}
              <tr class="total-row">
                <td>Razem</td>
                <td></td>
                <td class="num tabular">{fmtBytes(inventory.grandTotalBytes)}</td>
              </tr>
            </tbody>
          </table>
        </div>
      {:else}
        <p class="muted">Brak danych o zajętości, dopóki skanowanie się nie skończy.</p>
      {/if}
    </section>

    <section class="models-section">
      <header class="models-head">
        <h2>Ollama</h2>
        <p class="muted">{ollamaStatus(ollama)}</p>
      </header>
      {#if ollama?.models?.length}
        <div class="table-wrap">
          <table class="run-table compact">
            <thead>
              <tr>
                <th>Model</th>
                <th>ID</th>
                <th class="num">Rozmiar</th>
                <th>Zmodyfikowano</th>
                <th>Używane przez</th>
              </tr>
            </thead>
            <tbody>
              {#each ollama.models as model (model.name)}
                <tr>
                  <td><span class="mono" title={model.name}>{model.name}</span></td>
                  <td class="muted tabular">{model.id ?? "—"}</td>
                  <td class="num tabular">{fmtBytes(model.sizeBytes)}</td>
                  <td class="muted">{formatCheckedAt(model.modifiedAt) || "—"}</td>
                  <td title={usedByTitle(model.usedBy)}>{usedByLabel(model.usedBy)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <p class="muted">
          {#if ollama?.installed || ollama?.modelsDir}
            Brak modeli w Ollamie.
          {:else}
            Nie znaleziono instalacji Ollamy ani katalogu modeli.
          {/if}
        </p>
      {/if}
    </section>

    <section class="models-section">
      <header class="models-head">
        <h2>Cache'e</h2>
      </header>
      {#each caches as cache (cache.id)}
        <article class="cache-block">
          <header class="cache-head">
            <h3>{cache.label}</h3>
            <p class="muted" title={cache.path}>
              {#if cache.path}{shortPath(cache.path)} · {/if}{cacheHint(cache)}
            </p>
          </header>
          {#if cache.models?.length}
            <div class="table-wrap">
              <table class="run-table compact">
                <thead>
                  <tr>
                    <th>Model</th>
                    <th class="num">Rozmiar</th>
                    <th>Używane przez</th>
                  </tr>
                </thead>
                <tbody>
                  {#each cache.models as model (model.path)}
                    <tr>
                      <td>
                        <span class="mono" title={model.path}>{model.name}</span>
                      </td>
                      <td class="num tabular">{fmtBytes(model.sizeBytes)}</td>
                      <td title={usedByTitle(model.usedBy)}>{usedByLabel(model.usedBy)}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </article>
      {/each}
    </section>

    <section class="models-section">
      <header class="models-head">
        <h2>Wagi w projektach</h2>
        <p class="muted">
          {#if projects.length}
            {projects.length} {projects.length === 1 ? "projekt" : "projekty"} · {fmtBytes(projects.reduce((sum, item) => sum + (item.totalBytes ?? 0), 0))}
          {:else}
            Brak plików wag w skonfigurowanych katalogach projektów.
          {/if}
        </p>
      </header>
      {#each projects as project (project.projectId)}
        <article class="cache-block">
          <header class="cache-head">
            <h3 title={project.projectPath}>{project.projectName}</h3>
            <p class="muted">{shortPath(project.projectPath)} · {fmtBytes(project.totalBytes)}</p>
          </header>
          <div class="table-wrap">
            <table class="run-table compact">
              <thead>
                <tr>
                  <th>Plik</th>
                  <th class="num">Rozmiar</th>
                  <th>Używane przez</th>
                </tr>
              </thead>
              <tbody>
                {#each project.files ?? [] as file (file.path)}
                  <tr>
                    <td>
                      <span class="mono" title={file.path}>{file.name}</span>
                      <div class="muted path">{shortPath(file.path)}</div>
                    </td>
                    <td class="num tabular">{fmtBytes(file.sizeBytes)}</td>
                    <td title={usedByTitle(file.usedBy)}>{usedByLabel(file.usedBy)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </article>
      {/each}
    </section>
  </div>
{/if}

<style>
  .models-page {
    display: flex;
    flex-direction: column;
    gap: 1.4rem;
    padding-bottom: 2rem;
  }
  .models-section { min-width: 0; }
  .models-head, .cache-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
    margin-bottom: 0.55rem;
  }
  .models-head h2, .cache-head h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 650;
  }
  .cache-block { margin: 0.7rem 0 1rem; }
  .muted { color: var(--muted); font-size: 12px; }
  .path { word-break: break-all; }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .num { text-align: right; white-space: nowrap; }
  .total-row td { font-weight: 700; border-top: 1px solid var(--line); }
  .models-skel { height: 8rem; }
</style>
