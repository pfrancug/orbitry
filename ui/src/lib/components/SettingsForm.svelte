<script lang="ts">
  import { addPattern, addRoot, browseFolder, dash, removePattern, removeRoot, setAutoCheckUpdates, setConfigBridge, setLogLimitMb, addIgnoredProject, removeIgnoredProject, setDesktopNotifications, setOpenInTarget, setStartWithSystem } from "$lib/dashboard.svelte";
  import { desktopOs, formatCheckedAt, pathsEqual, resolveOpenIn, shortPath } from "$lib/format";
  import { i18n, setLocale, t, type Locale } from "$lib/i18n.svelte";
  import { checkForUpdate, installUpdate, update } from "$lib/update.svelte";
  import type { ConfigBridgeMode, OpenInKind, ScriptKind, ScriptScope } from "$lib/types";

  const locale = $derived(i18n.locale);

  let kind = $state<ScriptKind>("dev");
  let scope = $state<ScriptScope>("shared");
  let pattern = $state("");
  let ignoreDraft = $state("");

  async function submit(event: Event) {
    event.preventDefault();
    const value = pattern.trim();
    if (!value) return;
    await addPattern({ kind, scope, pattern: value });
    pattern = "";
  }

  const openIn = $derived(resolveOpenIn(dash.config));
  const explorerSettingLabel = $derived(
    desktopOs() === "windows"
      ? t("panel.explorer.windows")
      : desktopOs() === "mac"
        ? t("panel.explorer.mac")
        : t("panel.explorer.linux"),
  );
  const openInOptions = $derived(
    [
      { kind: "terminal" as OpenInKind, label: t("panel.terminal"), checked: openIn.terminal },
      { kind: "vscode" as OpenInKind, label: "VS Code", checked: openIn.vscode },
      { kind: "cursor" as OpenInKind, label: t("panel.cursor"), checked: openIn.cursor },
      { kind: "explorer" as OpenInKind, label: explorerSettingLabel, checked: openIn.explorer },
    ] as const,
  );
  const ignored = $derived(dash.config?.ignoredProjects ?? []);
  const ignorable = $derived(
    (dash.groups ?? []).filter(
      (group) => !ignored.some((id) => pathsEqual(id, group.id) || pathsEqual(id, group.path) || id === group.name),
    ),
  );

  function ignoredLabel(id: string): string {
    const group = (dash.groups ?? []).find(
      (item) => pathsEqual(item.id, id) || pathsEqual(item.path, id) || item.name === id,
    );
    return group?.name ?? shortPath(id);
  }

  function isKnownIgnored(id: string): boolean {
    return (dash.groups ?? []).some((group) => pathsEqual(group.id, id) || pathsEqual(group.path, id));
  }

  async function submitIgnore(event: Event) {
    event.preventDefault();
    if (!ignoreDraft) return;
    await addIgnoredProject(ignoreDraft);
    ignoreDraft = ignorable[0]?.id ?? "";
  }
</script>

{#if dash.config}
  <section class="settings-card" data-locale={locale}>
    <h2 class="settings-heading">{t("settings.language")}</h2>
    <p class="settings-hint">{t("settings.languageHint")}</p>
    <div class="segmented">
      {#each ([["en", "settings.lang.en"], ["pl", "settings.lang.pl"]] as const) as option (option[0])}
        <button
          type="button"
          class:active={locale === option[0]}
          onclick={() => setLocale(option[0] as Locale)}
        >
          {t(option[1])}
        </button>
      {/each}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.folders")}</h2>
    <p class="settings-hint">{t("settings.foldersHint")}</p>
    <form class="root-form" onsubmit={addRoot}>
      <label class="sr-only" for="root-path">{t("settings.folderLabel")}</label>
      <input
        id="root-path"
        bind:value={dash.rootDraft}
        placeholder={t("settings.folderPh")}
        spellcheck="false"
      />
      <button type="button" disabled={dash.saving} onclick={() => browseFolder()}>{t("settings.browse")}</button>
      <button type="submit" disabled={dash.saving || !dash.rootDraft.trim()}>{t("settings.addFolder")}</button>
    </form>
    <div class="root-list">
      {#each dash.config.roots as root (root)}
        <button class="root-chip" type="button" onclick={() => removeRoot(root)} title={t("settings.removeFolder")}>
          <span>{root}</span>
          <i aria-hidden="true">×</i>
        </button>
      {/each}
      {#if dash.config.roots.length === 0}
        <p class="hint">{t("settings.noFolders")}</p>
      {/if}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.builtin")}</h2>
    <div class="chips">
      {#each dash.config.defaultPatterns as item (`${item.kind}-${item.pattern}`)}
        <span class="chip quiet">
          <em>{item.kind}</em>
          {item.pattern}
        </span>
      {/each}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.custom")}</h2>
    <p class="settings-hint">{t("settings.customHint")}</p>
    <form class="pattern-form" onsubmit={submit}>
      <select bind:value={kind}>
        <option value="dev">{t("settings.kind.dev")}</option>
        <option value="preview">{t("settings.kind.preview")}</option>
        <option value="build">{t("settings.kind.build")}</option>
      </select>
      <select bind:value={scope}>
        <option value="shared">{t("settings.scope.shared")}</option>
        <option value="fe">{t("settings.scope.fe")}</option>
        <option value="be">{t("settings.scope.be")}</option>
      </select>
      <input bind:value={pattern} placeholder={t("settings.patternPh")} spellcheck="false" />
      <button type="submit" disabled={dash.saving || !pattern.trim()}>{t("settings.add")}</button>
    </form>
    <div class="custom-list">
      {#if dash.config.scriptPatterns.length === 0}
        <p class="hint">{t("settings.noPatterns")}</p>
      {:else}
        {#each dash.config.scriptPatterns as item, index (`${item.kind}-${item.scope}-${item.pattern}-${index}`)}
          <button class="root-chip" type="button" onclick={() => removePattern(index)} title={t("settings.removePattern")}>
            <span>{item.kind} · {item.scope} · {item.pattern}</span>
            <i aria-hidden="true">×</i>
          </button>
        {/each}
      {/if}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.bridge")}</h2>
    <p class="hint">{t("settings.bridgeHint")}</p>
    <div class="bridge-row">
      <span class="badge quiet">{dash.config.bridgeStatus?.label ?? (dash.config.configBridge === "off" ? t("settings.bridgeOff") : t("settings.bridgeUnknown"))}</span>
    </div>
    {#if dash.config.bridgeStatus?.nodeVersion}
      <p class="hint">Node {dash.config.bridgeStatus.nodeVersion}{dash.config.bridgeStatus.nodePath ? ` · ${dash.config.bridgeStatus.nodePath}` : ""}</p>
    {:else if dash.config.bridgeStatus?.state === "node-missing"}
      <p class="hint">{t("settings.nodeMissing")}</p>
    {:else if dash.config.bridgeStatus?.state === "plugin-missing"}
      <p class="hint">{t("settings.pluginMissing")}</p>
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
          {mode === "auto" ? t("settings.auto") : mode === "on" ? t("settings.on") : t("settings.off")}
        </button>
      {/each}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.logs")}</h2>
    <p class="hint">{t("settings.logsHint")}</p>
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

  <section class="settings-card">
    <label class="check-row autostart-row">
      <input
        type="checkbox"
        checked={dash.config.autostart?.enabled === true}
        disabled={dash.saving}
        onchange={(event) => setStartWithSystem(event.currentTarget.checked)}
      />
      {t("autostart.system")}
    </label>
    <p class="hint">{t("autostart.systemHint")}</p>
    {#if dash.config.autostart?.stale}
      <p class="hint">{t("autostart.stale")}</p>
    {/if}
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.notifications")}</h2>
    <label class="check-row autostart-row">
      <input
        type="checkbox"
        checked={dash.config.desktopNotifications === true}
        disabled={dash.saving}
        onchange={(event) => setDesktopNotifications(event.currentTarget.checked)}
      />
      {t("settings.notifications")}
    </label>
    <p class="hint">{t("settings.notificationsHint")}</p>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.openIn")}</h2>
    <p class="settings-hint">{t("settings.openInHint")}</p>
    <div class="open-in-list">
      {#each openInOptions as option (option.kind)}
        <label class="check-row autostart-row">
          <input
            type="checkbox"
            checked={option.checked}
            disabled={dash.saving}
            onchange={(event) => setOpenInTarget(option.kind, event.currentTarget.checked)}
          />
          {option.label}
        </label>
      {/each}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.ignore")}</h2>
    <p class="settings-hint">{t("settings.ignoreHint")}</p>
    <form class="root-form" onsubmit={submitIgnore}>
      <label class="sr-only" for="ignore-project">{t("settings.ignoreSelect")}</label>
      <select
        id="ignore-project"
        bind:value={ignoreDraft}
        disabled={dash.saving || ignorable.length === 0}
      >
        <option value="">{t("settings.ignoreSelect")}</option>
        {#each ignorable as group (group.id)}
          <option value={group.id}>{group.name}</option>
        {/each}
      </select>
      <button type="submit" data-ignore-add disabled={dash.saving || !ignoreDraft}>{t("settings.ignoreAdd")}</button>
    </form>
    <div class="root-list" data-ignore-list>
      {#each ignored as id (id)}
        <button
          class="root-chip"
          type="button"
          data-ignore-chip={id}
          onclick={() => removeIgnoredProject(id)}
          title={t("settings.ignoreRemove")}
        >
          <span title={id}>{ignoredLabel(id)}</span>
          {#if !isKnownIgnored(id)}
            <em class="muted">{t("settings.ignoreMissing")}</em>
          {/if}
          <i aria-hidden="true">×</i>
        </button>
      {/each}
      {#if ignored.length === 0}
        <p class="hint">{t("settings.ignoreEmpty")}</p>
      {/if}
    </div>
  </section>

  <section class="settings-card">
    <h2 class="settings-heading">{t("settings.updates")}</h2>
    <p class="hint">
      {t("settings.updatesOn", { version: update.current || "dev" })}
      {#if update.dev}
        {t("settings.updatesDev")}
      {:else if update.updateAvailable && update.latest}
        {t("settings.updatesAvailable", { version: update.latest })}
      {:else if update.latest}
        {t("settings.updatesCurrent")}
      {/if}
    </p>
    <label class="check-row">
      <input
        type="checkbox"
        checked={dash.config.autoCheckUpdates !== false}
        disabled={dash.saving}
        onchange={(event) => setAutoCheckUpdates(event.currentTarget.checked)}
      />
      {t("settings.autoCheck")}
    </label>
    <div class="bridge-row update-check-row">
      <button type="button" disabled={update.checking || update.dev} onclick={() => checkForUpdate()}>
        {update.checking ? t("settings.checking") : t("settings.checkNow")}
      </button>
      <div class="update-inline">
        {#if update.checking}
          <span class="hint">{t("settings.checkingUpdates")}</span>
        {:else if update.error}
          <span class="hint error">{update.error}</span>
        {:else if update.updateAvailable && update.latest}
          <span class="hint">
            {t("settings.updatesAvailable", { version: update.latest })}
            {#if update.notesUrl}
              <a href={update.notesUrl} target="_blank" rel="noreferrer">{t("settings.releaseNotes")}</a>
            {/if}
          </span>
          <button type="button" class="primary" disabled={update.installing || update.dev} onclick={() => installUpdate()}>
            {update.installing ? t("settings.updating") : t("settings.update")}
          </button>
        {:else if update.current}
          <span class="hint">
            {t("settings.latest", { version: update.current })}
            {#if update.checkedAt}
              · {t("settings.checked", { time: formatCheckedAt(update.checkedAt) })}
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
  .open-in-list {
    display: grid;
    gap: 2px;
    margin-top: 0.35rem;
  }
  .open-in-list .check-row {
    margin: 0;
    min-height: 28px;
  }
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
