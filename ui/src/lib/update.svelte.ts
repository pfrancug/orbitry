import { checkForUpdates, fetchVersion, startUpdate } from "./api";
import { runState } from "./runs.svelte";
import type { UpdatePhase, VersionInfo } from "./types";

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
  phase: "idle" as UpdatePhase,
  progress: null as number | null,
  message: null as string | null,
  rolledBack: false,
  checkedAt: null as number | null,
  toast: null as null | { kind: "available" | "latest" | "error" },
});

export function applyVersion(payload: VersionInfo) {
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
  update.phase = payload.phase ?? (payload.installing ? "installing" : "idle");
  update.progress = payload.progress ?? null;
  update.message = payload.message ?? null;
  update.rolledBack = Boolean(payload.rolledBack);
  update.checkedAt = payload.checkedAt ?? update.checkedAt;
}

export async function refreshVersion() {
  try {
    applyVersion(await fetchVersion());
    maybeShowUpdateToast(false);
  } catch {
    // Keep the last known version if the poll fails.
  }
}

export function dismissUpdateToast() {
  update.toast = null;
  if (update.updateAvailable && update.latest) {
    try {
      localStorage.setItem("orbitry.updateToastDismissed", update.latest);
    } catch {
      // Private mode can block storage.
    }
  }
}

function dismissedVersion(): string | null {
  try {
    return localStorage.getItem("orbitry.updateToastDismissed");
  } catch {
    return null;
  }
}

export function maybeShowUpdateToast(manual: boolean) {
  if (update.dev || update.installing) return;
  if (update.error) {
    if (manual) update.toast = { kind: "error" };
    return;
  }
  if (update.updateAvailable && update.latest) {
    if (dismissedVersion() !== update.latest || manual) {
      update.toast = { kind: "available" };
    }
    return;
  }
  if (manual && update.latest) {
    update.toast = { kind: "latest" };
    return;
  }
  if (manual) {
    update.toast = { kind: "latest" };
  }
}

export async function checkForUpdate() {
  update.checking = true;
  update.error = null;
  try {
    applyVersion(await checkForUpdates());
    maybeShowUpdateToast(true);
  } catch (err: unknown) {
    update.checking = false;
    update.error = err instanceof Error ? err.message : "Nie udało się sprawdzić aktualizacji.";
    update.toast = { kind: "error" };
  }
}

export async function installUpdate() {
  if (update.dev || update.installing) return;
  const latest = update.latest ?? "nową wersję";
  const message = `Zainstalować Orbitry ${latest}? Orbitry uruchomi się ponownie. Uruchomione aplikacje pozostaną włączone.`;
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
    } else {
      update.phase = result.phase ?? "downloading";
    }
  } catch (err: unknown) {
    update.installing = false;
    update.error = err instanceof Error ? err.message : "The update could not be started.";
  }
}
