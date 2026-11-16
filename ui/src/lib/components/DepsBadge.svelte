<script lang="ts">
  import { installCommand } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { isPending, runningFor, startInstall } from "$lib/runs.svelte";
  import { dash } from "$lib/dashboard.svelte";
  import type { ProjectPart } from "$lib/types";
  import Icon from "./Icon.svelte";

  let { part }: { part: ProjectPart } = $props();

  const command = $derived(installCommand(part));
  const title = $derived(t("deps.statusHint", { command }));
  const installLive = $derived(runningFor(part.id, "install"));
  const installPending = $derived(isPending(part.id, "install"));
</script>

<span
  class="warn-badge deps-badge"
  role="note"
  {title}
  aria-label={title}
>
  <Icon name="alert" size={11} />
  {t("deps.badge")}
  <span class="warn-popover" role="tooltip">
    <strong>{t("deps.status")}</strong>
    <span class="hint">{t("deps.statusHint", { command })}</span>
    <button
      class="btn"
      type="button"
      disabled={installPending || Boolean(installLive)}
      title={t("deps.explain", { command })}
      onclick={(event) => {
        event.stopPropagation();
        startInstall(part.id, dash.groups);
      }}
    >
      {installLive || installPending ? t("deps.installing") : t("deps.install")}
    </button>
  </span>
</span>

<style>
  .deps-badge .hint {
    display: block;
    margin: 4px 0 8px;
    color: var(--muted);
    font-size: 12px;
    font-weight: 400;
  }
  .deps-badge strong {
    display: block;
    font-size: 12px;
  }
</style>
