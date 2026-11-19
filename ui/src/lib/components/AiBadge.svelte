<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import HoverPopover from "./HoverPopover.svelte";

  let {
    models = [],
  }: {
    models?: string[];
  } = $props();

  const names = $derived([...new Set((models ?? []).map((name) => name.trim()).filter(Boolean))].slice(0, 8));
  const title = $derived(
    names.length ? names.join(", ") : t("ai.badge"),
  );
</script>

{#if names.length}
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
        {#each names as name (name)}
          <li title={name}>{name}</li>
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
    font-size: 12px;
    line-height: 1.35;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.hover-popover-layer) strong {
    display: block;
    font-size: 12px;
  }
</style>
