<script lang="ts">
  import { dash } from "$lib/dashboard.svelte";
  import { dedupeWarnings, issueWarnings } from "$lib/format";
  import { runState } from "$lib/runs.svelte";
  import Icon from "./Icon.svelte";
  import WarningList from "./WarningList.svelte";

  let open = $state(false);

  const warnings = $derived(dedupeWarnings([...(runState.warnings ?? []), ...(dash.warnings ?? [])]));
  const issues = $derived(issueWarnings(warnings));
  const infos = $derived(warnings.filter((item) => item.severity === "info"));
  const conflicts = $derived(issues.filter((item) => item.code === "port_conflict" || item.code === "port_occupied_foreign").length);
  const danger = $derived(issues.some((item) => item.severity === "error"));

  function summary(): string {
    const parts: string[] = [];
    if (conflicts) parts.push(conflicts === 1 ? "1 conflict" : `${conflicts} conflicts`);
    const otherIssues = issues.length - conflicts;
    if (otherIssues > 0) parts.push(otherIssues === 1 ? "1 warning" : `${otherIssues} warnings`);
    if (!issues.length && infos.length) parts.push(infos.length === 1 ? "1 notice" : `${infos.length} notices`);
    return parts.join(" · ");
  }
</script>

{#if warnings.length}
  <div class="alert-strip" class:danger class:warn={!danger}>
    <button class="alert-toggle" type="button" onclick={() => (open = !open)} aria-expanded={open}>
      <Icon name="alert" size={14} />
      <span>{summary()}</span>
      <span class="chevron" class:open></span>
    </button>
    {#if open}
      <div class="alert-body">
        <WarningList {warnings} />
      </div>
    {/if}
  </div>
{/if}
