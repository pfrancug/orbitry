<script lang="ts">
  import { dash, savePortOverrides } from "$lib/dashboard.svelte";
  import type { PortInfo, PortOverride, ProjectPart } from "$lib/types";

  let { part, ports = [] }: { part: ProjectPart; ports?: PortInfo[] } = $props();

  type Draft = { port: string; name: string; healthUrl: string; ignore: boolean };

  const saved = $derived((dash.config?.portOverrides ?? []).filter((item) => item.partId === part.id));

  let drafts = $state<Draft[]>([]);
  let loadedFor = $state<string | null>(null);
  let adding = $state("");
  let saving = $state(false);

  $effect(() => {
    const key = `${part.id}:${saved.map((item) => `${item.port}:${item.name}:${item.healthUrl}:${item.ignore}`).join("|")}:${(ports ?? []).map((item) => item.port).join(",")}`;
    if (loadedFor === key) return;
    loadedFor = key;
    drafts = mergeDrafts(part, ports ?? [], saved);
  });

  function mergeDrafts(current: ProjectPart, live: PortInfo[], overrides: PortOverride[]): Draft[] {
    const byPort = new Map<number, Draft>();
    for (const item of [...(current.ports ?? []), ...live]) {
      byPort.set(item.port, {
        port: String(item.port),
        name: item.name ?? "",
        healthUrl: item.healthUrl ?? "",
        ignore: false,
      });
    }
    for (const item of overrides) {
      const existing = byPort.get(item.port);
      byPort.set(item.port, {
        port: String(item.port),
        name: item.name ?? existing?.name ?? "",
        healthUrl: item.healthUrl ?? existing?.healthUrl ?? "",
        ignore: Boolean(item.ignore),
      });
    }
    return [...byPort.values()].sort((a, b) => Number(a.port) - Number(b.port));
  }

  function addPort() {
    const port = Number(adding);
    if (!Number.isInteger(port) || port < 1 || port > 65535) return;
    if (drafts.some((item) => Number(item.port) === port)) {
      adding = "";
      return;
    }
    drafts = [...drafts, { port: String(port), name: "", healthUrl: "", ignore: false }];
    adding = "";
  }

  async function save() {
    if (!dash.config) return;
    saving = true;
    try {
      const others = (dash.config.portOverrides ?? []).filter((item) => item.partId !== part.id);
      const next: PortOverride[] = drafts
        .map((item) => ({
          partId: part.id,
          port: Number(item.port),
          name: item.name.trim() || null,
          healthUrl: item.healthUrl.trim() || null,
          ignore: item.ignore,
        }))
        .filter((item) => item.port > 0 && (item.name || item.healthUrl || item.ignore));
      await savePortOverrides([...others, ...next]);
    } finally {
      saving = false;
    }
  }

  function removeDraft(port: string) {
    drafts = drafts.filter((item) => item.port !== port);
  }
</script>

<section class="panel-overrides">
  <h3>Port overrides</h3>
  <p class="hint">Rename a port, set a health URL, or ignore a false positive. Saved in Orbitry config.</p>

  {#if drafts.length === 0}
    <p class="hint">No ports on this app yet. Add one if you want to name or ignore it ahead of time.</p>
  {:else}
    <div class="override-list">
      {#each drafts as item, index (item.port)}
        <div class="override-row">
          <span class="tabular">:{item.port}</span>
          <input bind:value={drafts[index].name} placeholder="Name" spellcheck="false" />
          <input bind:value={drafts[index].healthUrl} placeholder="Health URL" spellcheck="false" />
          <label class="ignore">
            <input type="checkbox" bind:checked={drafts[index].ignore} />
            Ignore
          </label>
          <button class="icon-btn ghost" type="button" title="Remove override" onclick={() => removeDraft(item.port)}>
            ×
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="override-actions">
    <input
      class="add-port"
      bind:value={adding}
      inputmode="numeric"
      placeholder="Add port"
      spellcheck="false"
      onkeydown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          addPort();
        }
      }}
    />
    <button class="btn" type="button" onclick={addPort} disabled={!adding.trim()}>Add</button>
    <button class="btn primary" type="button" disabled={saving} onclick={save}>
      {saving ? "Saving..." : "Save overrides"}
    </button>
  </div>
</section>
