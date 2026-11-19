<script lang="ts">
  import { armAutoDismiss } from "$lib/autoDismiss.svelte";
  import { t } from "$lib/i18n.svelte";
  import { dismissUpdateToast, installUpdate, update } from "$lib/update.svelte";

  const visible = $derived(
    Boolean(update.toast) && !update.installing && !update.dev,
  );
  const kind = $derived(update.toast?.kind ?? null);

  let el = $state<HTMLElement | undefined>(undefined);
  $effect(() => {
    if (!visible || !kind) return;
    return armAutoDismiss(el, {
      ms: kind === "error" ? 10_000 : 6_000,
      dismiss: () => dismissUpdateToast(),
    });
  });
</script>

{#if visible && update.toast}
  <div class="update-toast" class:error={update.toast.kind === "error"} role="status" bind:this={el}>
    <div class="copy">
      {#if update.toast.kind === "available"}
        <p>{t("toast.update.available", { latest: update.latest, current: update.current })}</p>
        {#if update.notesUrl}
          <a href={update.notesUrl} target="_blank" rel="noreferrer">{t("settings.releaseNotes")}</a>
        {/if}
      {:else if update.toast.kind === "latest"}
        <p>{t("toast.update.latest", { version: update.current || update.latest })}</p>
      {:else}
        <p class="error">{update.error ?? t("toast.update.checkFailed")}</p>
      {/if}
    </div>
    <div class="actions">
      {#if update.toast.kind === "available"}
        <button type="button" disabled={update.installing} onclick={() => installUpdate()}>
          {update.installing ? t("settings.updating") : t("settings.update")}
        </button>
      {/if}
      <button class="icon-dismiss" type="button" onclick={dismissUpdateToast} title={t("common.close")} aria-label={t("common.close")}>
        ×
      </button>
    </div>
  </div>
{/if}

<style>
  .update-toast {
    display: flex;
    align-items: flex-start;
    gap: 0.85rem;
    width: 100%;
    padding: 0.8rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    border-radius: 14px;
    background: color-mix(in srgb, var(--sidebar) 88%, #000);
    box-shadow: var(--shadow);
    pointer-events: auto;
  }
  .update-toast.error {
    border-color: color-mix(in srgb, var(--danger) 45%, transparent);
  }
  .copy { min-width: 0; }
  p { margin: 0; font-size: 12px; }
  a {
    display: inline-block;
    margin-top: 0.3rem;
    font-size: 0.78rem;
    color: var(--accent);
    text-underline-offset: 0.15em;
  }
  .error { color: var(--danger); }
  .actions {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
  }
  .icon-dismiss {
    width: 28px;
    height: 28px;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
  }
  .icon-dismiss:hover { color: var(--text); }
</style>
