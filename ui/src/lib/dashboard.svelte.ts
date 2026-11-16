import { fetchConfig, fetchProjects, pickFolder, requestScan, saveConfig } from "./api";
import { refreshRuns, restoreSelection, runState } from "./runs.svelte";
import { findPart, normalizeScanDiff } from "./format";
import { t } from "./i18n.svelte";
import type { AppAutostartReport, AppConfig, AppWarning, ComposeProject, ConfigBridgeMode, PortOverride, ProjectsResponse, RepoGroup, ScanChange, ScanStatus, ScriptPattern } from "./types";

const PROJECTS_CACHE_KEY = "orbitry.projects-cache";

function readLocalProjects(): { groups: RepoGroup[]; warnings: AppWarning[]; scan: ScanStatus | null } | null {
  try {
    if (typeof localStorage === "undefined") return null;
    const raw = localStorage.getItem(PROJECTS_CACHE_KEY);
    if (!raw) return null;
    const data = JSON.parse(raw) as { groups?: RepoGroup[]; warnings?: AppWarning[]; scan?: ScanStatus | null };
    if (!Array.isArray(data.groups) || data.groups.length === 0) return null;
    return {
      groups: data.groups,
      warnings: data.warnings ?? [],
      scan: data.scan ?? null,
    };
  } catch {
    return null;
  }
}

function writeLocalProjects(payload: ProjectsResponse) {
  try {
    if (typeof localStorage === "undefined") return;
    if (!payload.groups?.length) return;
    localStorage.setItem(
      PROJECTS_CACHE_KEY,
      JSON.stringify({
        groups: payload.groups,
        warnings: payload.warnings ?? [],
        scan: payload.scan ?? null,
      }),
    );
  } catch {
    // Private mode can block storage.
  }
}

function asVerifying(scan: ScanStatus | null): ScanStatus {
  return {
    state: "cached",
    stale: true,
    scannedAt: scan?.scannedAt ?? null,
    startedAt: scan?.startedAt ?? null,
    diff: null,
    error: null,
  };
}

const localProjects = readLocalProjects();

export const dash = $state({
  groups: (localProjects?.groups ?? []) as RepoGroup[],
  warnings: (localProjects?.warnings ?? []) as AppWarning[],
  scan: (localProjects ? asVerifying(localProjects.scan) : null) as ScanStatus | null,
  dismissedScannedAt: null as number | null,
  config: null as AppConfig | null,
  query: "",
  rootDraft: "",
  error: null as string | null,
  loading: !localProjects,
  saving: false,
  refreshing: false,
  appAutostart: null as AppAutostartReport | null,
  appAutostartDismissedAt: null as number | null,
  compose: [] as ComposeProject[],
});

let scanPoll: ReturnType<typeof setInterval> | null = null;

export function applyProjectSnapshot(payload: ProjectsResponse) {
  dash.groups = payload.groups ?? [];
  dash.warnings = payload.warnings ?? [];
  dash.scan = payload.scan ?? null;
  dash.compose = payload.compose ?? [];
  applyAppAutostartReport(payload.appAutostart);
  if (payload.scan?.error) {
    dash.error = payload.scan.error;
  }
  writeLocalProjects(payload);
  restoreSelection(dash.groups.flatMap((group) => group.parts ?? []));
  watchScan();
}

export function isDiffEmpty(diff: { added?: ScanChange[]; removed?: ScanChange[]; changed?: ScanChange[] } | null | undefined): boolean {
  const normalized = normalizeScanDiff(diff);
  return normalized.added.length + normalized.removed.length + normalized.changed.length === 0;
}

export function changesVisible(): boolean {
  const scan = dash.scan;
  if (!scan || scan.state !== "fresh" || isDiffEmpty(scan.diff) || scan.scannedAt == null) return false;
  return dash.dismissedScannedAt !== scan.scannedAt;
}

export function dismissChanges() {
  dash.dismissedScannedAt = dash.scan?.scannedAt ?? null;
}

export function startsLocked(): boolean {
  return dash.scan?.state !== "fresh";
}

export function isRemovedEntry(partId?: string, script?: string, projectId?: string): boolean {
  if (!changesVisible()) return false;
  return (dash.scan?.diff?.removed ?? []).some((item) => {
    if (projectId && item.kind === "project" && item.projectId === projectId && !item.partId) return true;
    if (partId && item.partId === partId && (!item.script || item.script === script || !script)) {
      if (item.kind === "script" && script) return item.script === script;
      if (item.kind !== "script") return true;
    }
    return false;
  });
}

export function startBlockReason(partId?: string, script?: string): string | null {
  if (startsLocked()) return t("script.verifying");
  if (partId && isRemovedEntry(partId, script)) return t("script.removed");
  if (partId && script) {
    const part = findPart(partId, dash.groups);
    const selected = part?.scripts?.find((item) => item.name === script);
    if (selected?.kind === "preview" && part && !part.build?.built) {
      return t("build.previewBlocked");
    }
  }
  return null;
}

export function startWarnReason(partId?: string, script?: string): string | null {
  if (!partId || !script) return null;
  const part = findPart(partId, dash.groups);
  const selected = part?.scripts?.find((item) => item.name === script);
  if (selected?.kind === "preview" && part?.build?.built && part.build.stale) {
    return t("build.previewStale");
  }
  return null;
}

export function groupMarker(projectId: string): "new" | "changed" | "removed" | null {
  if (!changesVisible()) return null;
  const diff = normalizeScanDiff(dash.scan?.diff);
  if (diff.removed.some((item) => item.kind === "project" && item.projectId === projectId)) return "removed";
  if (diff.added.some((item) => item.kind === "project" && item.projectId === projectId)) return "new";
  if (
    diff.added.some((item) => item.projectId === projectId) ||
    diff.changed.some((item) => item.projectId === projectId) ||
    diff.removed.some((item) => item.projectId === projectId)
  ) {
    return "changed";
  }
  return null;
}

export function partMarker(partId: string, projectId?: string): "new" | "changed" | "removed" | null {
  if (!changesVisible()) return null;
  const diff = normalizeScanDiff(dash.scan?.diff);
  if (isRemovedEntry(partId, undefined, projectId)) return "removed";
  if (diff.added.some((item) => item.partId === partId || (item.kind === "project" && item.projectId === projectId))) {
    return "new";
  }
  if (diff.changed.some((item) => item.partId === partId || (item.kind === "project" && item.projectId === projectId))) {
    return "changed";
  }
  return null;
}

export function removedPartsFor(projectId: string): ScanChange[] {
  if (!changesVisible()) return [];
  return (dash.scan?.diff?.removed ?? []).filter(
    (item) => item.projectId === projectId && item.kind !== "project" && Boolean(item.partId),
  );
}

export function removedProjects(): ScanChange[] {
  if (!changesVisible()) return [];
  return (dash.scan?.diff?.removed ?? []).filter((item) => item.kind === "project");
}

export function watchScan() {
  if (dash.scan?.state === "scanning") {
    if (!scanPoll) {
      scanPoll = setInterval(() => {
        void pollScan();
      }, 700);
    }
  } else if (scanPoll) {
    clearInterval(scanPoll);
    scanPoll = null;
    dash.refreshing = false;
  }
}

async function pollScan() {
  try {
    applyProjectSnapshot(await fetchProjects());
  } catch {
    // Keep the last snapshot if a poll fails.
  }
}

export const allParts = () => dash.groups.flatMap((group) => group.parts ?? []);

export const partCount = () => dash.groups.reduce((sum, group) => sum + (group.parts ?? []).length, 0);

export async function loadDashboard() {
  const [nextConfig, nextProjects] = await Promise.all([fetchConfig(), fetchProjects()]);
  dash.config = nextConfig;
  applyProjectSnapshot(nextProjects);
}

export async function refreshAll(force = false) {
  dash.refreshing = true;
  dash.error = null;
  try {
    if (force) {
      const [nextConfig, nextProjects] = await Promise.all([fetchConfig(), requestScan()]);
      dash.config = nextConfig;
      applyProjectSnapshot(nextProjects);
      await refreshRuns();
    } else {
      await Promise.all([loadDashboard(), refreshRuns()]);
    }
  } catch (err: unknown) {
    dash.error = err instanceof Error ? err.message : "Could not reach Orbitry.";
    dash.refreshing = false;
  } finally {
    dash.loading = false;
    if (dash.scan?.state !== "scanning") dash.refreshing = false;
  }
}

function withOverrides(next: {
  roots: string[];
  scriptPatterns: ScriptPattern[];
  portOverrides?: PortOverride[];
  configBridge?: ConfigBridgeMode;
  autoCheckUpdates?: boolean;
  logLimitMb?: number;
  autostartApps?: string[];
  desktopNotifications?: boolean;
}) {
  return {
    roots: next.roots,
    scriptPatterns: next.scriptPatterns,
    portOverrides: next.portOverrides ?? dash.config?.portOverrides ?? [],
    configBridge: next.configBridge ?? dash.config?.configBridge ?? "auto",
    autoCheckUpdates: next.autoCheckUpdates ?? dash.config?.autoCheckUpdates ?? true,
    logLimitMb: next.logLimitMb ?? dash.config?.logLimitMb ?? 10,
    autostartApps: next.autostartApps ?? dash.config?.autostartApps ?? [],
    desktopNotifications: next.desktopNotifications ?? dash.config?.desktopNotifications ?? false,
  };
}

export async function persist(next: {
  roots: string[];
  scriptPatterns: ScriptPattern[];
  portOverrides?: PortOverride[];
  configBridge?: ConfigBridgeMode;
  autoCheckUpdates?: boolean;
  logLimitMb?: number;
  autostartApps?: string[];
  desktopNotifications?: boolean;
}) {
  dash.saving = true;
  dash.error = null;
  const payload = withOverrides(next);
  const previous = dash.config;
  if (dash.config) {
    dash.config = { ...dash.config, ...payload };
  }
  try {
    dash.config = await saveConfig(payload);
    applyProjectSnapshot(await fetchProjects());
  } catch (err: unknown) {
    if (previous) dash.config = previous;
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  } finally {
    dash.saving = false;
  }
}

export async function addRoot(event: Event) {
  event.preventDefault();
  const root = dash.rootDraft.trim();
  if (!root || !dash.config) return;
  if (dash.config.roots.some((existing) => existing.toLowerCase() === root.toLowerCase())) {
    dash.rootDraft = "";
    return;
  }
  await persist({ roots: [...dash.config.roots, root], scriptPatterns: dash.config.scriptPatterns });
  dash.rootDraft = "";
}

export async function removeRoot(root: string) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots.filter((item) => item !== root),
    scriptPatterns: dash.config.scriptPatterns,
  });
}

export async function addPattern(pattern: ScriptPattern) {
  if (!dash.config) return;
  const exists = dash.config.scriptPatterns.some(
    (item) =>
      item.kind === pattern.kind &&
      item.scope === pattern.scope &&
      item.pattern.toLowerCase() === pattern.pattern.toLowerCase(),
  );
  if (exists) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: [...dash.config.scriptPatterns, pattern],
  });
}

export async function removePattern(index: number) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns.filter((_, i) => i !== index),
  });
}

export async function browseFolder() {
  dash.error = null;
  try {
    const picked = await pickFolder();
    if (!picked?.path) return;
    dash.rootDraft = picked.path;
  } catch (err: unknown) {
    dash.error = err instanceof Error ? err.message : "Could not open the folder dialog.";
  }
}

export async function savePortOverrides(portOverrides: PortOverride[]) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    portOverrides,
  });
}

export async function setConfigBridge(mode: ConfigBridgeMode) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    configBridge: mode,
  });
}

export function bannerMessage(): string | null {
  return dash.error || runState.error;
}

export function dismissBanner() {
  dash.error = null;
  runState.error = null;
}

export function composeFor(projectId: string): ComposeProject | null {
  return dash.compose.find((item) => item.projectId === projectId) ?? null;
}

export async function setDesktopNotifications(enabled: boolean) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    desktopNotifications: enabled,
  });
}

export async function setAutoCheckUpdates(enabled: boolean) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    autoCheckUpdates: enabled,
  });
}

export async function setLogLimitMb(limitMb: number) {
  if (!dash.config) return;
  const value = Math.min(100, Math.max(2, Math.round(limitMb)));
  dash.saving = true;
  dash.error = null;
  const previous = dash.config;
  dash.config = { ...dash.config, logLimitMb: value };
  try {
    const payload = withOverrides({
      roots: dash.config.roots,
      scriptPatterns: dash.config.scriptPatterns,
      logLimitMb: value,
    });
    dash.config = await saveConfig(payload);
  } catch (err: unknown) {
    if (previous) dash.config = previous;
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  } finally {
    dash.saving = false;
  }
}

export async function setStartWithSystem(enabled: boolean) {
  if (!dash.config) return;
  dash.saving = true;
  dash.error = null;
  const previous = dash.config;
  dash.config = {
    ...dash.config,
    autostart: {
      enabled,
      command: dash.config.autostart?.command,
      stale: false,
    },
  };
  try {
    const payload = {
      ...withOverrides({
        roots: dash.config.roots,
        scriptPatterns: dash.config.scriptPatterns,
      }),
      startWithSystem: enabled,
    };
    dash.config = await saveConfig(payload);
  } catch (err: unknown) {
    if (previous) dash.config = previous;
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  } finally {
    dash.saving = false;
  }
}

export function applyAppAutostartReport(report?: AppAutostartReport | null) {
  if (!report) return;
  dash.appAutostart = report;
}

export function dismissAppAutostartToast() {
  dash.appAutostartDismissedAt = dash.appAutostart?.completedAt ?? Date.now();
}

export function pathsEqual(a: string, b: string): boolean {
  const norm = (value: string) =>
    value
      .replace(/^\\\\\?\\/, "")
      .replace(/\//g, "\\")
      .replace(/[\\/]+$/, "")
      .toLowerCase();
  return norm(a) === norm(b);
}

export function systemAutostartEnabled(): boolean {
  return dash.config?.autostart?.enabled === true;
}

export function appAutostartEnabled(partId: string): boolean {
  return (dash.config?.autostartApps ?? []).some((id) => pathsEqual(id, partId));
}

const APP_AUTOSTART_DISABLED =
  "Aby aplikacje startowały automatycznie, włącz „Uruchamiaj Orbitry razem z systemem” w Ustawieniach.";

export function appAutostartTitle(): string {
  return systemAutostartEnabled()
    ? "Uruchamiaj przy starcie Orbitry"
    : APP_AUTOSTART_DISABLED;
}

export async function setAppAutostart(partId: string, enabled: boolean) {
  if (!dash.config) return;
  dash.saving = true;
  dash.error = null;
  const previous = dash.config;
  const current = dash.config.autostartApps ?? [];
  const next = enabled
    ? current.some((id) => pathsEqual(id, partId))
      ? current
      : [...current, partId]
    : current.filter((id) => !pathsEqual(id, partId));
  dash.config = { ...dash.config, autostartApps: next };
  try {
    const payload = withOverrides({
      roots: dash.config.roots,
      scriptPatterns: dash.config.scriptPatterns,
      autostartApps: next,
    });
    dash.config = await saveConfig(payload);
  } catch (err: unknown) {
    if (previous) dash.config = previous;
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  } finally {
    dash.saving = false;
  }
}
