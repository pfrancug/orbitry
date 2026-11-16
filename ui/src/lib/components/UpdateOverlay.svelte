<script lang="ts">
  import { onMount } from "svelte";
  import { fetchUpdateStatus, fetchVersion } from "$lib/api";
  import { applyVersion, update } from "$lib/update.svelte";

  const startingVersion = update.current;
  let waitingForRestart = $state(false);
  let dismissed = $state(false);
  let reconnectError = $state<string | null>(null);

  const visible = $derived(!dismissed && (update.installing || waitingForRestart || update.phase === "error" || Boolean(update.error && update.phase && update.phase !== "idle")));

  const steps = [
    { id: "downloading", label: "Pobieranie" },
    { id: "verifying", label: "Weryfikacja SHA-256" },
    { id: "installing", label: "Instalacja" },
    { id: "restarting", label: "Restart" },
  ] as const;

  function stepState(id: string): "done" | "active" | "wait" | "error" {
    if (update.phase === "error") {
      const order = steps.map((step) => step.id);
      const current = order.indexOf(update.phase);
      const thisIndex = order.indexOf(id);
      if (thisIndex <= current || update.phase === "error") {
        // mark the failed step below
      }
    }
    const order = ["downloading", "verifying", "installing", "restarting"];
    const current = waitingForRestart && update.phase !== "error" ? "restarting" : update.phase;
    const ci = order.indexOf(current ?? "");
    const ti = order.indexOf(id);
    if (update.phase === "error" && ti === Math.max(ci, 0)) return "error";
    if (ti < ci || (current === "restarting" && id === "restarting" && !waitingForRestart && update.current !== startingVersion)) return "done";
    if (ti === ci || (waitingForRestart && id === "restarting" && update.phase !== "error")) return "active";
    return "wait";
  }

  async function pollStatus() {
    try {
      applyVersion(await fetchUpdateStatus());
      reconnectError = null;
      if (update.phase === "restarting" || update.installing) {
        if (update.phase === "restarting") waitingForRestart = true;
      }
    } catch {
      waitingForRestart = true;
    }
  }

  async function pollUntilNewVersion() {
    try {
      const info = await fetchVersion();
      applyVersion(info);
      reconnectError = null;
      if (info.current && info.current !== startingVersion) {
        window.location.reload();
        return;
      }
      if (!info.installing && info.phase === "idle" && info.current === startingVersion && update.rolledBack) {
        waitingForRestart = false;
      }
    } catch (err: unknown) {
      reconnectError = "Oczekiwanie na ponowne uruchomienie…";
    }
  }

  onMount(() => {
    const statusTimer = setInterval(() => {
      if (update.installing && !waitingForRestart) void pollStatus();
    }, 300);
    const versionTimer = setInterval(() => {
      if (waitingForRestart && update.phase !== "error") void pollUntilNewVersion();
    }, 700);
    return () => {
      clearInterval(statusTimer);
      clearInterval(versionTimer);
    };
  });
</script>

{#if visible}
  <div class="update-overlay" role="alertdialog" aria-live="polite" aria-labelledby="update-overlay-title">
    <div class="update-card">
      <h2 id="update-overlay-title">
        {#if update.phase === "error"}
          Aktualizacja nie powiodła się
        {:else}
          Aktualizacja Orbitry
        {/if}
      </h2>
      {#if update.latest}
        <p class="update-sub">{startingVersion || "ta wersja"} → {update.latest}</p>
      {/if}

      <ol class="update-steps">
        {#each steps as step}
          {@const state = stepState(step.id)}
          <li class="update-step" class:done={state === "done"} class:active={state === "active"} class:failed={state === "error"}>
            <span class="update-step-mark"></span>
            <span class="update-step-label">
              {step.label}
              {#if step.id === "downloading" && state === "active" && update.progress != null}
                <em>{update.progress}%</em>
              {/if}
            </span>
            {#if step.id === "downloading" && state === "active" && update.progress != null}
              <span class="update-bar" style="--pct: {update.progress}%"></span>
            {/if}
          </li>
        {/each}
      </ol>

      {#if update.message && update.phase !== "error"}
        <p class="update-msg">{update.message}</p>
      {/if}
      {#if reconnectError && update.phase !== "error"}
        <p class="update-msg">{reconnectError}</p>
      {/if}
      {#if update.phase === "error" || update.error}
        <p class="update-error">{update.error ?? update.message ?? "Aktualizacja nie powiodła się."}</p>
        {#if update.rolledBack}
          <p class="update-msg">Przywrócono poprzednią wersję i uruchomiono ją ponownie.</p>
        {/if}
        <button class="btn primary" type="button" onclick={() => (dismissed = true)}>Zamknij</button>
      {/if}
    </div>
  </div>
{/if}
