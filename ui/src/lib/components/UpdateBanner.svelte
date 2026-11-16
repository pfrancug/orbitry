<script lang="ts">
  import { dismissUpdateToast, installUpdate, update } from "$lib/update.svelte";

  const visible = $derived(
    Boolean(update.toast) && !update.installing && !update.dev,
  );
</script>

{#if visible && update.toast}
  <div class="update-toast" class:error={update.toast.kind === "error"} role="status">
    <div class="copy">
      {#if update.toast.kind === "available"}
        <p>Dostępna {update.latest}. Masz {update.current}.</p>
        {#if update.notesUrl}
          <a href={update.notesUrl} target="_blank" rel="noreferrer">Informacje o wydaniu</a>
        {/if}
      {:else if update.toast.kind === "latest"}
        <p>Masz najnowszą wersję ({update.current || update.latest}).</p>
      {:else}
        <p class="error">{update.error ?? "Nie udało się sprawdzić aktualizacji."}</p>
      {/if}
    </div>
    <div class="actions">
      {#if update.toast.kind === "available"}
        <button type="button" disabled={update.installing} onclick={() => installUpdate()}>
          {update.installing ? "Aktualizowanie…" : "Aktualizuj"}
        </button>
      {/if}
      <button class="icon-dismiss" type="button" onclick={dismissUpdateToast} title="Zamknij" aria-label="Zamknij">
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
  p { margin: 0; font-size: 0.85rem; }
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
    width: 1.6rem;
    height: 1.6rem;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 1.15rem;
    line-height: 1;
    cursor: pointer;
  }
  .icon-dismiss:hover { color: var(--text); }
</style>
