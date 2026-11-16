<script lang="ts">
  import { i18n, t } from "$lib/i18n.svelte";
  import { update, updateInProgress } from "$lib/update.svelte";

  const startingVersion = update.current;
  let dismissed = $state(false);
  const locale = $derived(i18n.locale);

  const visible = $derived(
    !dismissed && (updateInProgress() || Boolean(update.error && update.phase && update.phase !== "idle")),
  );

  const steps = $derived([
    { id: "downloading", label: t("update.step.download") },
    { id: "verifying", label: t("update.step.verify") },
    { id: "installing", label: t("update.step.install") },
    { id: "restarting", label: t("update.step.restart") },
  ] as const);

  function stepState(id: string): "done" | "active" | "wait" | "error" {
    const order = ["downloading", "verifying", "installing", "restarting"];
    const current =
      update.waitingForRestart && update.phase !== "error" ? "restarting" : update.phase;
    const ci = order.indexOf(current ?? "");
    const ti = order.indexOf(id);
    if (update.phase === "error" && ti === Math.max(ci, 0)) return "error";
    if (
      ti < ci ||
      (current === "restarting" &&
        id === "restarting" &&
        !update.waitingForRestart &&
        update.current !== startingVersion)
    )
      return "done";
    if (ti === ci || (update.waitingForRestart && id === "restarting" && update.phase !== "error"))
      return "active";
    return "wait";
  }
</script>


{#if visible}
  <div class="update-overlay" role="alertdialog" aria-live="polite" aria-labelledby="update-overlay-title">
    <div class="update-card">
      <h2 id="update-overlay-title">
        {#if update.phase === "error"}
          {t("update.failed")}
        {:else}
          {t("update.title")}
        {/if}
      </h2>
      {#if update.latest}
        <p class="update-sub">{t("update.fromTo", { from: startingVersion || t("update.thisVersion"), to: update.latest })}</p>
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
      {#if update.waitingForRestart && update.phase !== "error"}
        <p class="update-msg">{t("update.waiting")}</p>
      {/if}
      {#if update.phase === "error" || update.error}
        <p class="update-error">{update.error ?? update.message ?? t("update.failed")}</p>
        {#if update.rolledBack}
          <p class="update-msg">{t("update.rolledBack")}</p>
        {/if}
        <button class="btn primary" type="button" onclick={() => (dismissed = true)}>{t("update.close")}</button>
      {/if}
    </div>
  </div>
{/if}
