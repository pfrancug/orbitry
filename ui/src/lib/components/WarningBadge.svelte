<script lang="ts">
  import type { AppWarning } from "$lib/types";
  import { issueWarnings } from "$lib/format";
  import Icon from "./Icon.svelte";

  let { warnings = [] }: { warnings?: AppWarning[] } = $props();

  const issues = $derived(issueWarnings(warnings ?? []));
  const infos = $derived((warnings ?? []).filter((item) => item.severity === "info"));
  const count = $derived(issues.length || infos.length);
  const danger = $derived(issues.some((item) => item.severity === "error"));
  const title = $derived((warnings ?? []).map((item) => item.message).join("\n"));
</script>

{#if count}
  <span class="warn-badge" class:danger class:info={!issues.length} title={title}>
    <Icon name="alert" size={11} />
    {count}
  </span>
{/if}
