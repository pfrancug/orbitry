<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import type { AiSignal } from "$lib/models.svelte";
  import Icon from "./Icon.svelte";
  import HoverPopover from "./HoverPopover.svelte";

  let {
    signals = [],
    models = [],
  }: {
    signals?: AiSignal[];
    /** @deprecated Prefer `signals`. Still accepted for callers that only have model names. */
    models?: string[];
  } = $props();

  const items = $derived.by(() => {
    if (signals?.length) return signals;
    return (models ?? [])
      .map((label) => label.trim())
      .filter(Boolean)
      .map((label) => ({ kind: "model" as const, label }));
  });

  const title = $derived(
    items.length
      ? items.map((item) => (item.kind === "dependency" ? t("ai.depReason", { name: item.label }) : item.label)).join("\n")
      : t("ai.badge"),
  );
</script>

{#if items.length}
  <HoverPopover>
    {#snippet children()}
      <span class="tag-badge ai-badge" role="note" {title} aria-label={title}>
        <Icon name="models" size={11} />
        {t("ai.badge")}
      </span>
    {/snippet}
    {#snippet content()}
      <strong>{t("ai.title")}</strong>
      <ul class="ai-list">
        {#each items as item (`${item.kind}:${item.label}`)}
          <li title={item.label}>
            {#if item.kind === "dependency"}
              <span class="kind">{t("ai.depKind")}</span>
              {item.label}
            {:else}
              <span class="kind">{t("ai.modelKind")}</span>
              {item.label}
            {/if}
          </li>
        {/each}
      </ul>
    {/snippet}
  </HoverPopover>
{/if}

<style>
  .ai-list {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .ai-list li {
    display: flex;
    gap: 6px;
    align-items: baseline;
    font-size: 12px;
    line-height: 1.35;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    color: var(--muted);
    font-size: 11px;
    flex: none;
  }
  :global(.hover-popover-layer) strong {
    display: block;
    font-size: 12px;
  }
</style>
