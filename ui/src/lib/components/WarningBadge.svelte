<script lang="ts">
  import type { AppWarning } from "$lib/types";
  import { issueWarnings, warningSummaryLabel } from "$lib/format";
  import Icon from "./Icon.svelte";
  import WarningList from "./WarningList.svelte";
  import HoverPopover from "./HoverPopover.svelte";

  let { warnings = [] }: { warnings?: AppWarning[] } = $props();

  const issues = $derived(issueWarnings(warnings ?? []));
  const infos = $derived((warnings ?? []).filter((item) => item.severity === "info"));
  const count = $derived(issues.length || infos.length);
  const danger = $derived(issues.some((item) => item.severity === "error"));
  const title = $derived(
    [warningSummaryLabel(warnings ?? []), ...(warnings ?? []).map((item) => item.message)]
      .filter(Boolean)
      .join("\n"),
  );
</script>

{#if count}
  <HoverPopover>
    {#snippet children()}
      <span
        class="tag-badge warn-badge"
        class:danger
        class:info={!issues.length}
        {title}
        aria-label={title}
      >
        <Icon name="alert" size={11} />
        {count}
      </span>
    {/snippet}
    {#snippet content()}
      <WarningList {warnings} compact />
    {/snippet}
  </HoverPopover>
{/if}
