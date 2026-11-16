import { checkForUpdates, fetchVersion, startUpdate } from "./api";
import { runState } from "./runs.svelte";
import type { VersionInfo } from "./types";

export const update = $state({
  current: "",
  latest: null as string | null,
  updateAvailable: false,
  notesUrl: null as string | null,
  notes: null as string | null,
  checking: false,
  installing: false,
  error: null as string | null,
  autoCheck: true,
  dev: false,
});

function applyVersion(payload: VersionInfo) {
  update.current = payload.current ?? "";
  update.latest = payload.latest ?? null;
  update.updateAvailable = Boolean(payload.updateAvailable);
  update.notesUrl = payload.notesUrl ?? null;
  update.notes = payload.notes ?? null;
  update.checking = Boolean(payload.checking);
  update.installing = Boolean(payload.installing);
  update.error = payload.error ?? null;
  update.autoCheck = payload.autoCheck !== false;
  update.dev = Boolean(payload.dev);
}

export async function refreshVersion() {
  try {
    applyVersion(await fetchVersion());
  } catch {
    // Keep the last known version if the poll fails.
  }
}

export async function checkForUpdate() {
  update.checking = true;
  update.error = null;
  try {
    applyVersion(await checkForUpdates());
  } catch (err: unknown) {
    update.checking = false;
    update.error = err instanceof Error ? err.message : "Could not check for updates.";
  }
}

export async function installUpdate() {
  if (update.dev || update.installing) return;
  const running = (runState.runs ?? []).filter((run) => run.status === "running").length;
  const latest = update.latest ?? "the new version";
  const message =
    running > 0
      ? `${running} app${running === 1 ? " is" : "s are"} running. Updating Orbitry will stop them and restart. Continue?`
      : `Install Orbitry ${latest}? Orbitry will restart.`;
  if (!confirm(message)) return;
  update.installing = true;
  update.error = null;
  try {
    const result = await startUpdate(true);
    applyVersion(result);
    if (result.needsConfirm) {
      update.installing = false;
      return;
    }
    if (!result.started) {
      update.installing = false;
    }
  } catch (err: unknown) {
    update.installing = false;
    update.error = err instanceof Error ? err.message : "The update could not be started.";
  }
}
