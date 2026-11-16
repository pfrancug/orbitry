import type { AppWarning, DetectedScript, HealthInfo, PartRole, PortInfo, ProjectKind, ProjectPart, RepoGroup, Run, ScanChange, ScanDiff } from "./types";

export function formatAgeMs(ms: number | null | undefined): string {
  if (!ms) return "just now";
  const seconds = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 14) return `${days}d ago`;
  return new Date(ms).toLocaleDateString();
}

export function normalizeScanDiff(diff?: Partial<ScanDiff> | null): ScanDiff {
  return {
    added: Array.isArray(diff?.added) ? diff.added : [],
    removed: Array.isArray(diff?.removed) ? diff.removed : [],
    changed: Array.isArray(diff?.changed) ? diff.changed : [],
  };
}

export function changeLabel(item: ScanChange): string {
  if (item.kind === "script") return `${item.part ?? item.project} · ${item.script}`;
  if (item.kind === "part") return item.part ? `${item.project} / ${item.part}` : item.project;
  return item.project;
}

export function changeSummary(diff?: Partial<ScanDiff> | null): string {
  const normalized = normalizeScanDiff(diff);
  const parts: string[] = [];
  if (normalized.added.length) parts.push(`${normalized.added.length} new`);
  if (normalized.changed.length) parts.push(`${normalized.changed.length} changed`);
  if (normalized.removed.length) parts.push(`${normalized.removed.length} removed`);
  const names = [...normalized.added, ...normalized.changed, ...normalized.removed].slice(0, 5).map(changeLabel);
  const extra = normalized.added.length + normalized.changed.length + normalized.removed.length - names.length;
  const list = names.join(", ") + (extra > 0 ? ` +${extra}` : "");
  return `${parts.join(", ")}${list ? ` — ${list}` : ""}`;
}

export function timeAgo(unix: number | null): string {
  if (!unix) return "recently";
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - unix));
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 14) return `${days}d ago`;
  return new Date(unix * 1000).toLocaleDateString();
}

export function formatUptime(startedAt: number | null | undefined, now = Date.now()): string {
  if (!startedAt) return "—";
  const seconds = Math.max(0, Math.floor((now - startedAt) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${rest}s`;
  return `${rest}s`;
}

export function shortPath(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) return path;
  return parts.slice(-3).join(" / ");
}

export function isJsLike(kind: ProjectKind): boolean {
  return kind === "javascript" || kind === "mixed";
}

export function partTitle(part: ProjectPart): string {
  if (part.relativePath && part.relativePath !== ".") return part.relativePath;
  return part.name;
}

export function roleLabel(role: PartRole): string {
  if (role === "fe") return "FE";
  if (role === "be") return "BE";
  if (role === "shared") return "shared";
  if (role === "lib") return "lib";
  return "app";
}

export function formatPorts(ports: PortInfo[] | undefined): string {
  if (!ports?.length) return "";
  const unique = [...new Set(ports.map((item) => item.port))];
  return unique.map((port) => `:${port}`).join("  ");
}

export function preferredPort(ports: PortInfo[] | undefined): number | null {
  if (!ports?.length) return null;
  const rank = (source: PortInfo["source"]) =>
    source === "process" ? 6 : source === "log" ? 5 : source === "bridge" ? 4 : source === "flag" ? 3 : source === "env" ? 2 : source === "config" ? 1 : 0;
  const sorted = [...ports].sort((a, b) => rank(b.source) - rank(a.source) || a.port - b.port);
  return sorted[0]?.port ?? null;
}

export function portUrl(port: number, url?: string | null): string {
  return url || `http://localhost:${port}`;
}

export function isLivePort(port: PortInfo): boolean {
  return port.source === "process";
}

export function isExpectedSource(source: PortInfo["source"] | undefined): boolean {
  return source === "default" || source === "config" || source === "env" || source === "flag" || source === "bridge";
}

export function portsFor(part: ProjectPart, runs: Run[]): PortInfo[] {
  const live = runs.find((run) => run.status === "running" && run.partId === part.id);
  if (live?.ports?.length) return live.ports;
  const latest = runs.find((run) => run.partId === part.id && run.ports?.length);
  if (latest) return latest.ports;
  return part.ports ?? [];
}


function withPortNames(ports: PortInfo[], known?: PortInfo[] | null): PortInfo[] {
  if (!known?.length) return ports;
  return ports.map((item) => {
    if (item.name?.trim()) return item;
    const named = known.find((port) => port.port === item.port && Boolean(port.name?.trim()));
    return named ? { ...item, name: named.name } : item;
  });
}

export function pathsEqual(a: string, b: string): boolean {
  const normalize = (value: string) =>
    value.replaceAll("/", "\\").replace(/^\\\\\?\\/, "").replace(/\\+$/, "").toLowerCase();
  return normalize(a) === normalize(b);
}

/** Visible label for a port: its name, the script/role it belongs to, or its source. Never empty. */
export function portLabel(item: PortInfo, part?: ProjectPart | null, script?: string | null): string {
  const named = item.name?.trim();
  if (named) return named;
  const scriptName = script?.trim();
  if (scriptName) {
    const lower = scriptName.toLowerCase();
    if (lower === "dev" || lower.startsWith("dev:") || lower.startsWith("dev-") || lower.startsWith("dev_")) {
      return "dev";
    }
    if (lower.startsWith("preview")) return "preview";
    if (lower === "start" || lower.startsWith("start:") || lower.startsWith("start-")) return "start";
    if (lower.includes("api")) return "api";
    return scriptName;
  }
  if (part?.role === "be") return "api";
  if (part?.role === "fe") return "dev";
  switch (item.source) {
    case "default":
      return "default";
    case "config":
      return "config";
    case "env":
      return "env";
    case "flag":
      return "flag";
    case "bridge":
      return "config";
    case "log":
      return "log";
    case "process":
      return "process";
    default:
      return item.source || "port";
  }
}

export function displayPorts(part: ProjectPart | null | undefined, runs: Run[], runningOnly = false): PortInfo[] {
  const live = part ? runs.find((run) => run.status === "running" && run.partId === part.id) : undefined;
  const known = [...(part?.ports ?? []), ...(live?.expectedPorts ?? []), ...(live?.ports ?? [])];
  if (live) {
    const process = (live.ports ?? []).filter((item) => item.source === "process");
    return withPortNames(dedupePorts(process), known);
  }
  if (runningOnly) return [];
  return withPortNames(dedupePorts(part?.ports ?? []), known);
}

export function displayRunPorts(run: Run | null | undefined, part?: ProjectPart | null): PortInfo[] {
  if (!run?.ports?.length) return [];
  const process = (run.ports ?? []).filter((item) => item.source === "process");
  const known = [...(part?.ports ?? []), ...(run.expectedPorts ?? []), ...(run.ports ?? [])];
  return withPortNames(dedupePorts(process), known);
}

export function dedupeWarnings(warnings: AppWarning[]): AppWarning[] {
  const seen = new Set<string>();
  const out: AppWarning[] = [];
  for (const warning of warnings) {
    const key = [warning.code, warning.port ?? "", warning.partId ?? "", warning.runId ?? "", warning.message].join("|");
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(warning);
  }
  return out;
}

export function warningsForPart(partId: string, runs: Run[], global: AppWarning[] = []): AppWarning[] {
  const fromRuns = runs.flatMap((run) => (run.partId === partId ? (run.warnings ?? []) : []));
  const fromGlobal = global.filter((warning) => warning.partId === partId);
  return dedupeWarnings([...fromRuns, ...fromGlobal]);
}

export function warningsForRun(run: Run | null | undefined, global: AppWarning[] = []): AppWarning[] {
  if (!run) return [];
  const fromRun = run.warnings ?? [];
  const fromGlobal = global.filter(
    (warning) => warning.runId === run.id || (!warning.runId && warning.partId === run.partId),
  );
  return dedupeWarnings([...fromRun, ...fromGlobal]);
}

export function issueWarnings(warnings: AppWarning[]): AppWarning[] {
  return warnings.filter((warning) => warning.severity === "error" || warning.severity === "warning");
}

export function warningCountLabel(warnings: AppWarning[]): string | null {
  const issues = issueWarnings(warnings);
  const infos = warnings.filter((warning) => warning.severity === "info");
  const parts: string[] = [];
  if (issues.length) parts.push(String(issues.length));
  else if (infos.length) parts.push(String(infos.length));
  return parts[0] ?? null;
}

const CONFLICT_CODES = new Set(["port_conflict", "port_occupied_foreign"]);

export type WarningGroup = {
  code: string;
  severity: AppWarning["severity"];
  count: number;
  items: AppWarning[];
};

export function groupWarnings(warnings: AppWarning[]): WarningGroup[] {
  const order: string[] = [];
  const map = new Map<string, AppWarning[]>();
  for (const warning of warnings) {
    const list = map.get(warning.code);
    if (list) {
      list.push(warning);
    } else {
      order.push(warning.code);
      map.set(warning.code, [warning]);
    }
  }
  return order.map((code) => {
    const items = map.get(code) ?? [];
    const severity = items.some((item) => item.severity === "error")
      ? "error"
      : items.some((item) => item.severity === "warning")
        ? "warning"
        : "info";
    return { code, severity, count: items.length, items };
  });
}

export function polishCount(n: number, one: string, few: string, many: string): string {
  const abs = Math.abs(n);
  const mod10 = abs % 10;
  const mod100 = abs % 100;
  if (abs === 1) return `${n} ${one}`;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return `${n} ${few}`;
  return `${n} ${many}`;
}

export function warningSummaryLabel(warnings: AppWarning[]): string {
  const issues = issueWarnings(warnings);
  const infos = warnings.filter((warning) => warning.severity === "info");
  const conflicts = issues.filter((warning) => CONFLICT_CODES.has(warning.code)).length;
  const other = issues.length - conflicts;
  const parts: string[] = [];
  if (conflicts) parts.push(polishCount(conflicts, "konflikt", "konflikty", "konfliktów"));
  if (other) parts.push(polishCount(other, "ostrzeżenie", "ostrzeżenia", "ostrzeżeń"));
  if (infos.length) parts.push(polishCount(infos.length, "komunikat", "komunikaty", "komunikatów"));
  return parts.join(" · ");
}

function dedupePorts(ports: PortInfo[]): PortInfo[] {
  const seen = new Set<number>();
  const out: PortInfo[] = [];
  for (const item of ports) {
    if (seen.has(item.port)) continue;
    seen.add(item.port);
    out.push(item);
  }
  return out;
}

export function healthLabel(health: HealthInfo | undefined): string {
  if (!health || health.status === "unknown") return "idle";
  if (health.status === "ok") return "healthy";
  if (health.status === "fail") return "down";
  return "no port";
}

export function groupNameFor(partId: string, groups: RepoGroup[]): string {
  const group = groups.find((item) => (item.parts ?? []).some((part) => part.id === partId));
  return group?.name ?? "";
}

export function findPart(partId: string, groups: RepoGroup[]): ProjectPart | undefined {
  for (const group of groups) {
    const part = (group.parts ?? []).find((item) => item.id === partId);
    if (part) return part;
  }
  return undefined;
}

export function groupSearchText(group: RepoGroup): string {
  return [
    group.name,
    group.folder,
    group.path,
    group.framework ?? "",
    group.kind,
    group.git?.branch ?? "",
    ...(group.parts ?? []).flatMap((part) => [
      part.name,
      part.path,
      part.relativePath,
      part.framework ?? "",
      part.role,
      ...(part.scripts ?? []).map((script) => script.name),
      ...(part.ports ?? []).map((item) => String(item.port)),
    ]),
  ]
    .join(" ")
    .toLowerCase();
}

export function fmtPct(value: number | null | undefined, digits = 0): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${value.toFixed(digits)}%`;
}

export function fmtBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes)) return "—";
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${Math.round(bytes / 1024 ** 2)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${Math.round(bytes)} B`;
}

export function formatCheckedAt(ts: number | null | undefined): string {
  if (!ts) return "";
  const date = new Date(ts);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString("pl-PL", { hour: "2-digit", minute: "2-digit", day: "2-digit", month: "2-digit" });
}

export function runScripts(part: ProjectPart | null | undefined): DetectedScript[] {
  return (part?.scripts ?? []).filter((script) => script.kind !== "build");
}

export function buildScripts(part: ProjectPart | null | undefined): DetectedScript[] {
  return (part?.scripts ?? []).filter((script) => script.kind === "build");
}

/** One button per distinct build command on an app (package + workspace duplicates collapse). */
export function uniqueBuildScripts(part: ProjectPart | null | undefined): DetectedScript[] {
  const seen = new Map<string, DetectedScript>();
  for (const script of buildScripts(part)) {
    const command = (script.command ?? "").trim().toLowerCase();
    const cwd = (script.cwd ?? "").trim().toLowerCase();
    const key = command
      ? `${command}|${cwd}`
      : `name:${script.name}|${cwd}|${script.source ?? ""}`;
    if (!seen.has(key)) seen.set(key, script);
  }
  return [...seen.values()];
}

/** Distinct label when an app has more than one build script. */
export function buildButtonName(script: DetectedScript): string {
  const label = scriptLabel(script).trim();
  if (label && label.toLowerCase() !== "build") return label;
  const extra = (script.sourceName ?? "").trim() || script.name.replace(/^build:?/i, "").trim();
  return extra || script.name;
}

export function scriptLabel(script: DetectedScript): string {
  return script.label?.trim() || script.name;
}

/** Stable unique key for script lists. Package and workspace scripts can share a name and kind. */
export function scriptEachKey(script: DetectedScript, prefix = ""): string {
  return JSON.stringify([
    prefix,
    script.source ?? "",
    script.scope ?? "",
    script.kind,
    script.name,
    script.command ?? "",
    script.cwd ?? "",
  ]);
}

export function installCommand(part: ProjectPart | null | undefined): string {
  return `${part?.packageManager || "npm"} install`;
}
