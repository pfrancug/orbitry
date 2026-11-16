<script lang="ts">
  import { closeDockerLogs, dockerAction, dockerUi, loadDockerLogs } from "$lib/docker.svelte";
  import { shortPath } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import type { ComposeProject } from "$lib/types";

  let { compose }: { compose: ComposeProject } = $props();

  const isBusy = $derived(Boolean(dockerUi.busy && dockerUi.busy.split(":")[1] === compose.projectId));
  const logsOpen = $derived(dockerUi.logs?.projectId === compose.projectId);

  function stateLabel(state: string | undefined): string {
    const value = (state ?? "").toLowerCase();
    if (value.includes("run")) return t("docker.running");
    if (!value) return t("docker.stopped");
    if (["exited", "stopped", "created", "dead", "paused"].some((item) => value.includes(item))) {
      return t("docker.stopped");
    }
    return state ?? t("docker.stopped");
  }
</script>

<section class="compose">
  <header class="compose-head">
    <div>
      <h3>{t("docker.title")}</h3>
      <p class="muted" title={compose.file}>{shortPath(compose.file)}</p>
    </div>
    <div class="compose-actions">
      <button
        class="chip mini"
        type="button"
        disabled={!compose.dockerAvailable || isBusy}
        onclick={() => dockerAction("up", compose.projectId)}
      >
        {t("docker.up")}
      </button>
      <button
        class="chip mini"
        type="button"
        disabled={!compose.dockerAvailable || isBusy}
        onclick={() => dockerAction("down", compose.projectId)}
      >
        {t("docker.down")}
      </button>
      <button
        class="chip mini"
        type="button"
        disabled={!compose.dockerAvailable || isBusy}
        onclick={() => dockerAction("restart", compose.projectId)}
      >
        {t("docker.restart")}
      </button>
      <button
        class="chip mini"
        type="button"
        disabled={!compose.dockerAvailable || isBusy}
        onclick={() => (logsOpen ? closeDockerLogs() : loadDockerLogs(compose.projectId))}
      >
        {t("docker.logs")}
      </button>
    </div>
  </header>

  {#if !compose.dockerAvailable}
    <p class="hint">{t("docker.unavailable")}</p>
  {/if}
  {#if compose.error}
    <p class="hint error">{t("docker.error", { error: compose.error })}</p>
  {/if}

  {#if compose.services?.length}
    <ul class="service-list">
      {#each compose.services as service (service.name)}
        <li>
          <span class="svc-name">{service.name}</span>
          <span class="muted">{stateLabel(service.state)}{service.status ? ` · ${service.status}` : ""}</span>
          {#if service.ports?.length}
            <span class="muted ports">{service.ports.join(", ")}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="hint">{t("docker.none")}</p>
  {/if}

  {#if logsOpen}
    <div class="compose-logs">
      {#if dockerUi.logError}
        <p class="hint error">{dockerUi.logError}</p>
      {/if}
      <pre>{dockerUi.logs?.text || ""}</pre>
    </div>
  {/if}
</section>

<style>
  .compose {
    margin: 0 10px 12px;
    padding: 10px 12px 12px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: color-mix(in srgb, var(--card) 80%, transparent);
  }
  .compose-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  h3 { margin: 0; font-size: 13px; font-weight: 650; }
  .muted { color: var(--muted); font-size: 12px; margin: 2px 0 0; }
  .compose-actions { display: flex; flex-wrap: wrap; gap: 6px; }
  .service-list {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .service-list li {
    display: grid;
    grid-template-columns: minmax(6rem, 1fr) max-content minmax(0, 1.2fr);
    gap: 8px;
    align-items: baseline;
    font-size: 12px;
  }
  .svc-name { font-weight: 600; }
  .ports { text-align: right; overflow-wrap: anywhere; }
  .compose-logs pre {
    margin: 10px 0 0;
    max-height: 220px;
    overflow: auto;
    overscroll-behavior: contain;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.25);
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .error { color: var(--danger); }
</style>
