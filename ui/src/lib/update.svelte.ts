import { checkForUpdates, fetchUpdateStatus, fetchVersion, startUpdate } from "./api";
import { t } from "./i18n.svelte";
import type { UpdatePhase, VersionInfo } from "./types";

const ACTIVE_PHASES = new Set(["downloading", "verifying", "installing", "restarting", "error"]);

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
  waitingForRestart: false,
});

let baselineVersion = "";
let pollTimer: ReturnType<typeof setInterval> | null = null;
let polling = false;
let inFlight = false;

export function updateInProgress(): boolean {
  return (
    update.installing ||
    update.waitingForRestart ||
    ACTIVE_PHASES.has(update.phase)
  );
}

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
  if (update.current && !baselineVersion) baselineVersion = update.current;
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
  if (update.dev || update.installing || update.waitingForRestart) return;
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
    update.error = err instanceof Error ? err.message : t("settings.checkFailed");
    update.toast = { kind: "error" };
  }
}

export async function installUpdate() {
  if (update.dev || update.installing) return;
  const latest = update.latest ?? t("update.aNewVersion");
  if (!confirm(t("update.confirm", { version: latest }))) return;
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
    update.error = err instanceof Error ? err.message : t("update.startFailed");
  }
}

export function startUpdatePolling() {
  if (polling) return;
  polling = true;
  if (!baselineVersion) baselineVersion = update.current;
  void pollUpdateStatus();
  armPollTimer();
}

function armPollTimer() {
  if (pollTimer) clearInterval(pollTimer);
  const ms = updateInProgress() ? 300 : 1000;
  pollTimer = setInterval(() => {
    void pollUpdateStatus();
    const next = updateInProgress() ? 300 : 1000;
    if (next !== ms) armPollTimer();
  }, ms);
}

async function pollUpdateStatus() {
  if (inFlight) return;
  inFlight = true;
  try {
    const info = await fetchUpdateStatus();
    applyVersion(info);
    if (!baselineVersion && info.current) baselineVersion = info.current;
    if (info.phase === "restarting") {
      update.waitingForRestart = true;
    }
    if (
      update.waitingForRestart &&
      info.current &&
      baselineVersion &&
      info.current !== baselineVersion
    ) {
      window.location.reload();
      return;
    }
    if (
      update.waitingForRestart &&
      !info.installing &&
      info.phase === "idle" &&
      info.rolledBack &&
      info.current === baselineVersion
    ) {
      update.waitingForRestart = false;
    }
  } catch {
    if (updateInProgress() && update.phase !== "error") {
      update.waitingForRestart = true;
    }
  } finally {
    inFlight = false;
  }
}
