<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import type { AppConfig, ScriptKind, ScriptPattern, ScriptScope } from "$lib/types";

  let {
    config,
    saving,
    rootDraft = $bindable(""),
    onclose,
    onadd,
    onremove,
    onaddroot,
    onremoveroot,
  }: {
    config: AppConfig;
    saving: boolean;
    rootDraft: string;
    onclose: () => void;
    onadd: (pattern: ScriptPattern) => Promise<void>;
    onremove: (index: number) => Promise<void>;
    onaddroot: (event: Event) => Promise<void>;
    onremoveroot: (root: string) => Promise<void>;
  } = $props();

  let kind = $state<ScriptKind>("dev");
  let scope = $state<ScriptScope>("shared");
  let pattern = $state("");

  async function submit(event: Event) {
    event.preventDefault();
    const value = pattern.trim();
    if (!value) return;
    await onadd({ kind, scope, pattern: value });
    pattern = "";
  }
</script>

<div
  class="overlay"
  role="presentation"
  transition:fade={{ duration: 160 }}
  onclick={(event) => {
    if (event.target === event.currentTarget) onclose();
  }}
>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="settings-title" transition:fly={{ x: 28, duration: 260 }}>
    <header>
      <div>
        <h2 id="settings-title">Settings</h2>
        <p>Project folders and the extra script patterns Orbitry should detect.</p>
      </div>
      <button class="icon-btn" type="button" onclick={onclose}>Close</button>
    </header>

    <section class="settings-block">
      <p class="label">Project folders</p>
      <form class="root-form" onsubmit={onaddroot}>
        <label class="sr-only" for="root-path">Project root folder</label>
        <input
          id="root-path"
          bind:value={rootDraft}
          placeholder="C:\Projects\private"
          spellcheck="false"
        />
        <button type="submit" disabled={saving || !rootDraft.trim()}>Add folder</button>
      </form>
      <div class="root-list">
        {#each config.roots as root (root)}
          <button class="root-chip" type="button" onclick={() => onremoveroot(root)} title="Remove folder">
            <span>{root}</span>
            <i aria-hidden="true">×</i>
          </button>
        {/each}
        {#if config.roots.length === 0}
          <p class="hint">Point Orbitry at a folder of projects to begin.</p>
        {/if}
      </div>
    </section>

    <section class="settings-block">
      <p class="label">Built in</p>
      <div class="chips">
        {#each config.defaultPatterns as item (`${item.kind}-${item.pattern}`)}
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
        <button type="submit" disabled={saving || !pattern.trim()}>Add</button>
      </form>
      <div class="custom-list">
        {#if config.scriptPatterns.length === 0}
          <p class="hint">No custom patterns yet. They live here, not in a config file you have to edit.</p>
        {:else}
          {#each config.scriptPatterns as item, index (`${item.kind}-${item.scope}-${item.pattern}-${index}`)}
            <button class="root-chip" type="button" onclick={() => onremove(index)} title="Remove pattern">
              <span>{item.kind} · {item.scope} · {item.pattern}</span>
              <i aria-hidden="true">×</i>
            </button>
          {/each}
        {/if}
      </div>
    </section>
  </div>
</div>
