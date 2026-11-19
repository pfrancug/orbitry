import { fetchConfig, fetchProjects, pickFolder, requestScan, saveConfig } from "./api";
import { refreshRuns, restoreSelection, runState } from "./runs.svelte";
import { findPart, normalizeScanDiff, pathsEqual } from "./format";
import { t } from "./i18n.svelte";
import type { AppAutostartReport, AppConfig, AppWarning, ComposeProject, ConfigBridgeMode, PortOverride, ProjectsResponse, RepoGroup, ScanChange, ScanDiff, ScanStatus, ScriptPattern } from "./types";

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

export function groupForPart(partId: string | null | undefined): RepoGroup | null {
  if (!partId) return null;
  return dash.groups.find((group) => (group.parts ?? []).some((part) => part.id === partId)) ?? null;
}

export function isIgnoredProject(ref: string | null | undefined): boolean {
  if (!ref) return false;
  const ignored = dash.config?.ignoredProjects ?? [];
  if (!ignored.length) return false;
  if (ignored.some((id) => pathsEqual(id, ref) || id === ref)) return true;
  const group =
    dash.groups.find((item) => pathsEqual(item.id, ref) || pathsEqual(item.path, ref) || item.name === ref) ??
    groupForPart(ref);
  if (!group) return false;
  return ignored.some((id) => pathsEqual(id, group.id) || pathsEqual(id, group.path) || id === group.name);
}

export function isIgnoredPart(partId: string | null | undefined): boolean {
  const group = groupForPart(partId);
  return group ? isIgnoredProject(group.id) : isIgnoredProject(partId);
}

export function isIgnoredChange(item: ScanChange): boolean {
  return isIgnoredProject(item.projectId) || isIgnoredProject(item.project);
}

export function isIgnoredWarning(warning: AppWarning): boolean {
  if (warning.project && isIgnoredProject(warning.project)) return true;
  if (warning.partId && isIgnoredPart(warning.partId)) return true;
  return false;
}

export function visibleWarnings(warnings: AppWarning[] | null | undefined): AppWarning[] {
  return (warnings ?? []).filter((item) => !isIgnoredWarning(item));
}

export function visibleScanDiff(diff?: Partial<ScanDiff> | null): ScanDiff {
  const normalized = normalizeScanDiff(diff);
  return {
    added: normalized.added.filter((item) => !isIgnoredChange(item)),
    removed: normalized.removed.filter((item) => !isIgnoredChange(item)),
    changed: normalized.changed.filter((item) => !isIgnoredChange(item)),
  };
}

export function changesVisible(): boolean {
  const scan = dash.scan;
  if (!scan || scan.state !== "fresh" || isDiffEmpty(visibleScanDiff(scan.diff)) || scan.scannedAt == null) {
    return false;
  }
  return dash.dismissedScannedAt !== scan.scannedAt;
}

export async function addIgnoredProject(projectId: string) {
  if (!dash.config) return;
  const id = projectId.trim();
  if (!id) return;
  const current = dash.config.ignoredProjects ?? [];
  if (current.some((item) => pathsEqual(item, id))) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    ignoredProjects: [...current, id],
  });
}

export async function removeIgnoredProject(projectId: string) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    ignoredProjects: (dash.config.ignoredProjects ?? []).filter((item) => !pathsEqual(item, projectId)),
  });
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
    if (selected?.source === "ahk") {
      const version = (selected.ahkVersion || "v2").toLowerCase();
      const entry = dash.config?.ahkStatus?.interpreters?.find(
        (item) => item.version.toLowerCase() === version,
      );
      const installed =
        entry?.installed === true ||
        (entry?.installed == null && Boolean(entry?.path));
      if (!installed) {
        return selected.blockedReason?.trim() || t("script.ahkMissing", { version });
      }
    }
    if (selected?.blockedReason?.trim()) {
      return selected.blockedReason.trim();
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
  if (!changesVisible() || isIgnoredProject(projectId)) return null;
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
  if (!changesVisible() || isIgnoredPart(partId) || isIgnoredProject(projectId)) return null;
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
  ignoredProjects?: string[];
  openIn?: import("./types").OpenInTargets;
  modelsIncludeDocs?: boolean;
  ahkPlugin?: boolean;
}) {
  return {
    roots: next.roots,
    scriptPatterns: next.scriptPatterns,
    portOverrides: next.portOverrides ?? dash.config?.portOverrides ?? [],
    configBridge: next.configBridge ?? dash.config?.configBridge ?? "on",
    autoCheckUpdates: next.autoCheckUpdates ?? dash.config?.autoCheckUpdates ?? true,
    logLimitMb: next.logLimitMb ?? dash.config?.logLimitMb ?? 10,
    autostartApps: next.autostartApps ?? dash.config?.autostartApps ?? [],
    desktopNotifications: next.desktopNotifications ?? dash.config?.desktopNotifications ?? false,
    ignoredProjects: next.ignoredProjects ?? dash.config?.ignoredProjects ?? [],
    openIn: next.openIn ?? dash.config?.openIn ?? {
      terminal: true,
      vscode: true,
      cursor: true,
      explorer: true,
    },
    modelsIncludeDocs: next.modelsIncludeDocs ?? dash.config?.modelsIncludeDocs ?? false,
    ahkPlugin: next.ahkPlugin ?? dash.config?.ahkPlugin ?? true,
  };
}

function patchConfig(next: AppConfig | Record<string, unknown>) {
  if (!dash.config) {
    dash.config = next as AppConfig;
    return;
  }
  Object.assign(dash.config, next);
}

function cloneConfig(): AppConfig | null {
  // $state proxies cannot be cloned as-is; a snapshot is a plain object.
  return dash.config ? $state.snapshot(dash.config) : null;
}

function applySavedConfig(
  payload: {
    ignoredProjects?: string[];
    openIn?: import("./types").OpenInTargets;
    modelsIncludeDocs?: boolean;
    ahkPlugin?: boolean;
  },
  saved: AppConfig,
): AppConfig {
  let next = saved;
  // The API omits an empty ignoredProjects list. Keep the list we just saved
  // so a successful clear is not treated as "field missing, leave as-is".
  if (!Object.prototype.hasOwnProperty.call(saved, "ignoredProjects") || saved.ignoredProjects == null) {
    next = { ...next, ignoredProjects: payload.ignoredProjects ?? [] };
  }
  // Default Open-in prefs are omitted from the saved file. Keep the values we
  // just wrote so the UI does not flicker back to "missing = all on" incorrectly
  // when the user has an all-default object in memory.
  if (payload.openIn && (!Object.prototype.hasOwnProperty.call(saved, "openIn") || saved.openIn == null)) {
    next = { ...next, openIn: payload.openIn };
  }
  // modelsIncludeDocs=false is omitted from the saved file; keep the value we wrote.
  if (
    payload.modelsIncludeDocs != null &&
    (!Object.prototype.hasOwnProperty.call(saved, "modelsIncludeDocs") || saved.modelsIncludeDocs == null)
  ) {
    next = { ...next, modelsIncludeDocs: payload.modelsIncludeDocs };
  }
  // ahkPlugin=true is omitted from the saved file; keep the value we wrote.
  if (
    payload.ahkPlugin != null &&
    (!Object.prototype.hasOwnProperty.call(saved, "ahkPlugin") || saved.ahkPlugin == null)
  ) {
    next = { ...next, ahkPlugin: payload.ahkPlugin };
  }
  return next;
}

function shouldRefreshProjects(next: { roots: string[]; scriptPatterns: ScriptPattern[] }): boolean {
  const current = dash.config;
  if (!current) return true;
  const rootsChanged =
    next.roots.length !== current.roots.length ||
    next.roots.some((root, index) => root !== current.roots[index]);
  const patternsChanged = JSON.stringify(next.scriptPatterns) !== JSON.stringify(current.scriptPatterns);
  return rootsChanged || patternsChanged;
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
  ignoredProjects?: string[];
  openIn?: import("./types").OpenInTargets;
  modelsIncludeDocs?: boolean;
  ahkPlugin?: boolean;
}) {
  dash.error = null;
  const refreshProjects = shouldRefreshProjects(next);
  const payload = withOverrides(next);
  let previous: AppConfig | null = null;
  try {
    previous = cloneConfig();
  } catch {
    previous = null;
  }
  if (dash.config) patchConfig(payload);
  try {
    patchConfig(applySavedConfig(payload, await saveConfig(payload)));
    if (refreshProjects) applyProjectSnapshot(await fetchProjects());
  } catch (err: unknown) {
    if (previous) patchConfig(previous);
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
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

export async function setModelsIncludeDocs(enabled: boolean) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    modelsIncludeDocs: enabled,
  });
}

export async function setAhkPlugin(enabled: boolean) {
  if (!dash.config) return;
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    ahkPlugin: enabled,
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

export async function setOpenInTarget(kind: import("./types").OpenInKind, enabled: boolean) {
  if (!dash.config) return;
  const current = {
    terminal: dash.config.openIn?.terminal !== false,
    vscode: dash.config.openIn?.vscode !== false,
    cursor: dash.config.openIn?.cursor !== false,
    explorer: dash.config.openIn?.explorer !== false,
  };
  await persist({
    roots: dash.config.roots,
    scriptPatterns: dash.config.scriptPatterns,
    openIn: { ...current, [kind]: enabled },
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
  dash.error = null;
  const previous = cloneConfig();
  patchConfig({ logLimitMb: value });
  try {
    const payload = withOverrides({
      roots: dash.config.roots,
      scriptPatterns: dash.config.scriptPatterns,
      logLimitMb: value,
    });
    patchConfig(await saveConfig(payload));
  } catch (err: unknown) {
    if (previous) patchConfig(previous);
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  }
}

export async function setStartWithSystem(enabled: boolean) {
  if (!dash.config) return;
  dash.error = null;
  const previous = cloneConfig();
  patchConfig({
    autostart: {
      enabled,
      command: dash.config.autostart?.command,
      stale: false,
    },
  });
  try {
    const payload = {
      ...withOverrides({
        roots: dash.config.roots,
        scriptPatterns: dash.config.scriptPatterns,
      }),
      startWithSystem: enabled,
    };
    patchConfig(await saveConfig(payload));
  } catch (err: unknown) {
    if (previous) patchConfig(previous);
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  }
}

export function applyAppAutostartReport(report?: AppAutostartReport | null) {
  if (!report) return;
  dash.appAutostart = report;
}

export function dismissAppAutostartToast() {
  dash.appAutostartDismissedAt = dash.appAutostart?.completedAt ?? Date.now();
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
  dash.error = null;
  const previous = cloneConfig();
  const current = dash.config.autostartApps ?? [];
  const next = enabled
    ? current.some((id) => pathsEqual(id, partId))
      ? current
      : [...current, partId]
    : current.filter((id) => !pathsEqual(id, partId));
  patchConfig({ autostartApps: next });
  try {
    const payload = withOverrides({
      roots: dash.config.roots,
      scriptPatterns: dash.config.scriptPatterns,
      autostartApps: next,
    });
    patchConfig(await saveConfig(payload));
  } catch (err: unknown) {
    if (previous) patchConfig(previous);
    dash.error = err instanceof Error ? err.message : "Could not save settings.";
  }
}
