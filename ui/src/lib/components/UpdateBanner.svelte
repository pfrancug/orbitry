<script lang="ts">
  import { installUpdate, update } from "$lib/update.svelte";
</script>

{#if !update.dev && update.updateAvailable}
  <div class="update-banner" class:installing={update.installing}>
    <div class="copy">
      <p>
        Orbitry {update.latest} is available. You're on {update.current}.
      </p>
      {#if update.notesUrl}
        <a href={update.notesUrl} target="_blank" rel="noreferrer">Release notes</a>
      {/if}
      {#if update.error}
        <p class="error">{update.error}</p>
      {/if}
    </div>
    <button type="button" disabled={update.installing} onclick={() => installUpdate()}>
      {update.installing ? "Updating…" : `Update to ${update.latest}`}
    </button>
  </div>
{/if}

<style>
  .update-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin: 0 1rem;
    padding: 0.7rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--accent, #8b9cff) 40%, transparent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent, #8b9cff) 12%, transparent);
  }

  .copy {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.55rem 0.85rem;
  }

  p {
    margin: 0;
    font-size: 0.85rem;
  }

  a {
    font-size: 0.8rem;
    color: inherit;
    text-underline-offset: 0.15em;
  }

  .error {
    color: var(--danger, #f0a8a8);
    width: 100%;
  }

  button {
    flex-shrink: 0;
  }

  @media (max-width: 720px) {
    .update-banner {
      flex-direction: column;
      align-items: stretch;
    }
  }
</style>
