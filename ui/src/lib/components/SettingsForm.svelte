<script lang="ts">
  import { addPattern, addRoot, browseFolder, dash, removePattern, removeRoot, setAutoCheckUpdates, setConfigBridge, setLogLimitMb, setStartWithSystem } from "$lib/dashboard.svelte";
  import { formatCheckedAt } from "$lib/format";
  import { checkForUpdate, installUpdate, update } from "$lib/update.svelte";
  import type { ConfigBridgeMode, ScriptKind, ScriptScope } from "$lib/types";

  let kind = $state<ScriptKind>("dev");
  let scope = $state<ScriptScope>("shared");
  let pattern = $state("");

  async function submit(event: Event) {
    event.preventDefault();
    const value = pattern.trim();
    if (!value) return;
    await addPattern({ kind, scope, pattern: value });
    pattern = "";
  }
</script>

{#if dash.config}
  <section class="settings-block">
    <p class="label">Project folders</p>
    <form class="root-form" onsubmit={addRoot}>
      <label class="sr-only" for="root-path">Project root folder</label>
      <input
        id="root-path"
        bind:value={dash.rootDraft}
        placeholder="C:\Projects\private"
        spellcheck="false"
      />
      <button type="button" disabled={dash.saving} onclick={() => browseFolder()}>Browse</button>
      <button type="submit" disabled={dash.saving || !dash.rootDraft.trim()}>Add folder</button>
    </form>
    <div class="root-list">
      {#each dash.config.roots as root (root)}
        <button class="root-chip" type="button" onclick={() => removeRoot(root)} title="Remove folder">
          <span>{root}</span>
          <i aria-hidden="true">×</i>
        </button>
      {/each}
      {#if dash.config.roots.length === 0}
        <p class="hint">Point Orbitry at a folder of projects to begin.</p>
      {/if}
    </div>
  </section>

  <section class="settings-block">
    <p class="label">Built in</p>
    <div class="chips">
      {#each dash.config.defaultPatterns as item (`${item.kind}-${item.pattern}`)}
        <span class="chip quiet">
          <em>{item.kind}</em>
          {item.pattern}
        </span>
      {/each}
    </div>
  </section>

  <section class="settings-block">
    <p class="label">Custom script patterns</p>
    <form class="pattern-form" onsubmit={submit}>
      <select bind:value={kind}>
        <option value="dev">dev</option>
        <option value="preview">preview / start</option>
      </select>
      <select bind:value={scope}>
        <option value="shared">shared</option>
        <option value="fe">frontend</option>
        <option value="be">backend</option>
      </select>
      <input bind:value={pattern} placeholder="e.g. dev:mobile or serve*" spellcheck="false" />
      <button type="submit" disabled={dash.saving || !pattern.trim()}>Add</button>
    </form>
    <div class="custom-list">
      {#if dash.config.scriptPatterns.length === 0}
        <p class="hint">No custom patterns yet. They live here, not in a config file you have to edit.</p>
      {:else}
        {#each dash.config.scriptPatterns as item, index (`${item.kind}-${item.scope}-${item.pattern}-${index}`)}
          <button class="root-chip" type="button" onclick={() => removePattern(index)} title="Remove pattern">
            <span>{item.kind} · {item.scope} · {item.pattern}</span>
            <i aria-hidden="true">×</i>
          </button>
        {/each}
      {/if}
    </div>
  </section>

  <section class="settings-block">
    <p class="label">Config bridge</p>
    <p class="hint">When a project has a Vite or Next config, Orbitry can read the real port from it. This is optional and needs Node.</p>
    <div class="bridge-row">
      <span class="badge quiet">{dash.config.bridgeStatus?.label ?? (dash.config.configBridge === "off" ? "disabled" : "unknown")}</span>
    </div>
    {#if dash.config.bridgeStatus?.nodeVersion}
      <p class="hint">Node {dash.config.bridgeStatus.nodeVersion}{dash.config.bridgeStatus.nodePath ? ` · ${dash.config.bridgeStatus.nodePath}` : ""}</p>
    {:else if dash.config.bridgeStatus?.state === "node-missing"}
      <p class="hint">Node was not found on this machine. The bridge stays off until it is installed and on your PATH.</p>
    {:else if dash.config.bridgeStatus?.state === "plugin-missing"}
      <p class="hint">The config-bridge plugin is missing.</p>
    {:else if dash.config.bridgeStatus?.error}
      <p class="hint">{dash.config.bridgeStatus.error}</p>
    {/if}
    <div class="segmented">
      {#each ["auto", "on", "off"] as mode (mode)}
        <button
          type="button"
          class:active={(dash.config.configBridge ?? "auto") === mode}
          disabled={dash.saving}
          onclick={() => setConfigBridge(mode as ConfigBridgeMode)}
        >
          {mode === "auto" ? "Auto" : mode === "on" ? "On" : "Off"}
        </button>
      {/each}
    </div>
  </section>

  <section class="settings-block">
    <p class="label">Maks. rozmiar logów na aplikację</p>
    <p class="hint">
      Domyślnie 10 MB na aplikację (5 MB plus jedna kopia obrotowa). Zakres 2–100 MB.
      Limit globalny rośnie wraz z tą wartością. Zmiana obowiązuje od razu, bez restartu.
    </p>
    <label class="log-limit-row">
      <select
        value={dash.config.logLimitMb ?? 10}
        disabled={dash.saving}
        onchange={(event) => setLogLimitMb(Number((event.currentTarget as HTMLSelectElement).value))}
      >
        {#each [2, 5, 10, 25, 50, 100] as size (size)}
          <option value={size}>{size} MB</option>
        {/each}
      </select>
    </label>
  </section>

  <section class="settings-block">
    <label class="check-row autostart-row">
      <input
        type="checkbox"
        checked={dash.config.autostart?.enabled === true}
        disabled={dash.saving}
        onchange={(event) => setStartWithSystem(event.currentTarget.checked)}
      />
      Uruchamiaj Orbitry razem z systemem
    </label>
    <p class="hint">
      Po zalogowaniu Orbitry wystartuje w zasobniku, bez otwierania przeglądarki.
      Gdy to włączysz, wybrane aplikacje można uruchamiać automatycznie przy starcie Orbitry.
    </p>
    {#if dash.config.autostart?.stale}
      <p class="hint">
        Zarejestrowana ścieżka do Orbitry jest nieaktualna i zostanie zaktualizowana.
      </p>
    {/if}
  </section>

  <section class="settings-block">
    <p class="label">Updates</p>
    <p class="hint">
      You're on {update.current || "a development build"}.
      {#if update.dev}
        Development builds show the version but don't install updates.
      {:else if update.updateAvailable}
        {update.latest} is available.
      {:else if update.latest}
        You're up to date.
      {/if}
    </p>
    <label class="check-row">
      <input
        type="checkbox"
        checked={dash.config.autoCheckUpdates !== false}
        disabled={dash.saving}
        onchange={(event) => setAutoCheckUpdates(event.currentTarget.checked)}
      />
      Check for updates automatically
    </label>
    <div class="bridge-row update-check-row">
      <button type="button" disabled={update.checking || update.dev} onclick={() => checkForUpdate()}>
        {update.checking ? "Sprawdzanie…" : "Check now"}
      </button>
      <div class="update-inline">
        {#if update.checking}
          <span class="hint">Sprawdzanie aktualizacji…</span>
        {:else if update.error}
          <span class="hint error">{update.error}</span>
        {:else if update.updateAvailable && update.latest}
          <span class="hint">
            Dostępna {update.latest}.
            {#if update.notesUrl}
              <a href={update.notesUrl} target="_blank" rel="noreferrer">Informacje o wydaniu</a>
            {/if}
          </span>
          <button type="button" class="primary" disabled={update.installing || update.dev} onclick={() => installUpdate()}>
            {update.installing ? "Aktualizowanie…" : "Aktualizuj"}
          </button>
        {:else if update.current}
          <span class="hint">
            Masz najnowszą wersję ({update.current})
            {#if update.checkedAt}
              · sprawdzono {formatCheckedAt(update.checkedAt)}
            {/if}
          </span>
        {/if}
      </div>
    </div>
  </section>
{/if}

<style>
  .check-row {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    margin: 0.65rem 0 0.85rem;
    font-size: 0.88rem;
  }
  .autostart-row { margin: 0 0 0.45rem; }
  .log-limit-row { margin-top: 0.55rem; }
  .log-limit-row select { min-width: 8rem; }
  .update-check-row {
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 0.75rem;
  }
  .update-inline {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.55rem 0.75rem;
    min-width: 0;
    flex: 1;
  }
  .update-inline .hint { margin: 0; }
  .update-inline .error { color: var(--danger); }
  .update-inline a { color: var(--accent); }
</style>
