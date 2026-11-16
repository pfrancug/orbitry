<script lang="ts">
  import { addPattern, addRoot, browseFolder, dash, removePattern, removeRoot, setConfigBridge } from "$lib/dashboard.svelte";
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
{/if}
