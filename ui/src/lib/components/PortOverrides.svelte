<script lang="ts">
  import { dash, savePortOverrides } from "$lib/dashboard.svelte";
  import type { PortInfo, PortOverride, ProjectPart } from "$lib/types";

  let { part, ports = [] }: { part: ProjectPart; ports?: PortInfo[] } = $props();

  type Draft = { port: number; ignore: boolean };

  const saved = $derived((dash.config?.portOverrides ?? []).filter((item) => item.partId === part.id));

  let drafts = $state<Draft[]>([]);
  let loadedFor = $state<string | null>(null);
  let saving = $state(false);

  $effect(() => {
    const key = `${part.id}:${saved.map((item) => `${item.port}:${item.ignore}`).join("|")}:${(ports ?? []).map((item) => item.port).join(",")}`;
    if (loadedFor === key) return;
    loadedFor = key;
    const byPort = new Map<number, boolean>();
    for (const item of [...(ports ?? []), ...(part.ports ?? [])]) {
      if (!byPort.has(item.port)) byPort.set(item.port, false);
    }
    for (const item of saved) {
      byPort.set(item.port, Boolean(item.ignore));
    }
    drafts = [...byPort.entries()]
      .map(([port, ignore]) => ({ port, ignore }))
      .sort((a, b) => a.port - b.port);
  });

  async function toggle(port: number, ignore: boolean) {
    if (!dash.config) return;
    drafts = drafts.map((item) => (item.port === port ? { ...item, ignore } : item));
    saving = true;
    try {
      const others = (dash.config.portOverrides ?? []).filter((item) => item.partId !== part.id);
      const existing = (dash.config.portOverrides ?? []).filter((item) => item.partId === part.id);
      const next: PortOverride[] = drafts
        .map((item) => {
          const prev = existing.find((row) => row.port === item.port);
          return {
            partId: part.id,
            port: item.port,
            name: prev?.name ?? null,
            healthUrl: prev?.healthUrl ?? null,
            ignore: item.ignore,
          };
        })
        .filter((item) => item.ignore || item.name || item.healthUrl);
      await savePortOverrides([...others, ...next]);
    } finally {
      saving = false;
    }
  }
</script>

{#if drafts.length}
  <section class="panel-overrides">
    <h3>Porty</h3>
    <div class="override-list">
      {#each drafts as item (item.port)}
        <label
          class="ignore-row"
          title="Ukryj ten port, jeśli Orbitry wykryło go przez pomyłkę."
        >
          <input
            type="checkbox"
            checked={item.ignore}
            disabled={saving}
            onchange={(event) => toggle(item.port, event.currentTarget.checked)}
          />
          <span>Ignoruj port <span class="tabular">:{item.port}</span></span>
        </label>
      {/each}
    </div>
  </section>
{/if}
