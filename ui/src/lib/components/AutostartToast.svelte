<script lang="ts">
  import { dash, dismissAppAutostartToast } from "$lib/dashboard.svelte";
  import type { AppAutostartEvent } from "$lib/types";

  const report = $derived(dash.appAutostart);
  const visible = $derived.by(() => {
    const current = report;
    if (!current || current.state !== "done") return false;
    const empty =
      !(current.started?.length || current.skipped?.length || current.blocked?.length || current.failed?.length);
    if (empty) return false;
    return dash.appAutostartDismissedAt !== (current.completedAt ?? null);
  });
  const sticky = $derived((report?.blocked?.length ?? 0) > 0 || (report?.failed?.length ?? 0) > 0);

  $effect(() => {
    if (!visible || sticky) return;
    const completedAt = report?.completedAt ?? null;
    const timer = setTimeout(() => {
      if (dash.appAutostart?.completedAt === completedAt) dismissAppAutostartToast();
    }, 10_000);
    return () => clearTimeout(timer);
  });

  function names(events: AppAutostartEvent[] | undefined): string {
    return (events ?? []).map((event) => event.partName).join(", ");
  }

  function blockedLine(event: AppAutostartEvent): string {
    if (event.port != null && event.process) {
      return `Nie uruchomiono ${event.partName} — port ${event.port} zajmuje ${event.process}.`;
    }
    return `Nie uruchomiono ${event.partName}${event.reason ? ` — ${event.reason}` : "."}`;
  }
</script>

{#if visible}
  <aside class="autostart-toast" class:alert={sticky} role="status">
    <div class="copy">
      <p><strong>Autostart aplikacji</strong></p>
      {#if report?.started?.length}
        <p>Uruchomiono: {names(report.started)}.</p>
      {/if}
      {#if report?.skipped?.length}
        <p>Pominięto (już działa lub niedostępne): {names(report.skipped)}.</p>
      {/if}
      {#if report?.blocked?.length}
        {#each report.blocked as event (event.partId + String(event.port ?? "") + (event.process ?? ""))}
          <p class="alert-line">{blockedLine(event)}</p>
        {/each}
      {/if}
      {#if report?.failed?.length}
        {#each report.failed as event (event.partId + (event.reason ?? ""))}
          <p class="alert-line">
            Nie udało się uruchomić {event.partName}{event.reason ? `: ${event.reason}` : "."}
          </p>
        {/each}
      {/if}
    </div>
    <button class="icon-dismiss" type="button" aria-label="Zamknij" onclick={dismissAppAutostartToast}>×</button>
  </aside>
{/if}

<style>
  .autostart-toast {
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
  .autostart-toast.alert {
    border-color: color-mix(in srgb, var(--warn, var(--danger)) 50%, transparent);
  }
  .copy { min-width: 0; }
  p { margin: 0; font-size: 0.85rem; }
  p + p { margin-top: 0.3rem; }
  .alert-line { color: var(--warn, var(--danger)); }
  .icon-dismiss {
    width: 1.6rem;
    height: 1.6rem;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 1.15rem;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
  }
  .icon-dismiss:hover { color: var(--text); }
</style>
