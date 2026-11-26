<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import type { ProjectKind } from "$lib/types";
  import HoverPopover from "./HoverPopover.svelte";

  let {
    kind,
  }: {
    kind?: ProjectKind | null;
  } = $props();

  const isSwift = $derived(kind === "swift");
  const label = $derived(isSwift ? t("tech.swift") : "");
  const title = $derived(isSwift ? t("tech.swiftHint") : "");
</script>

{#if isSwift}
  <HoverPopover>
    {#snippet children()}
      <span class="tag-badge tech-badge tech-swift" role="note" aria-label={title || label}>
        {label}
      </span>
    {/snippet}
    {#snippet content()}
      <strong>{label}</strong>
      <p>{title}</p>
    {/snippet}
  </HoverPopover>
{/if}
