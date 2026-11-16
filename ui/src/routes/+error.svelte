<script lang="ts">
  import { page } from "$app/state";
  import { resetClientStorage } from "$lib/storage";

  const status = $derived(page.status || 500);
  const message = $derived(page.error?.message || "Something went wrong.");

  function resetCache() {
    resetClientStorage();
    location.href = "/";
  }
</script>

<div class="page-head">
  <div>
    <h1>{status} {status === 404 ? "Not Found" : "Internal Error"}</h1>
    <p class="muted">{status === 404 ? "This page does not exist." : "Orbitry hit a problem loading this page."}</p>
  </div>
</div>

<div class="empty-card">
  <strong>{message}</strong>
  <p class="muted">You can go back to the dashboard, or clear Orbitry's saved browser data (language, recent scripts, and similar) and reload. Your apps and the Orbitry config on disk are not affected.</p>
  <div class="page-actions" style="margin-top: 0.85rem">
    <a class="btn primary" href="/">Back to Overview</a>
    <button class="btn" type="button" onclick={resetCache}>Reset local cache</button>
  </div>
</div>
