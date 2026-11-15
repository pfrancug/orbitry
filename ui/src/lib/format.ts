import type { AppWarning, HealthInfo, PartRole, PortInfo, ProjectKind, ProjectPart, RepoGroup, Run } from "./types";

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
    source === "process" ? 5 : source === "log" ? 4 : source === "flag" ? 3 : source === "env" ? 2 : source === "config" ? 1 : 0;
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
  return source === "default" || source === "config" || source === "env" || source === "flag";
}

export function portsFor(part: ProjectPart, runs: Run[]): PortInfo[] {
  const live = runs.find((run) => run.status === "running" && run.partId === part.id);
  if (live?.ports?.length) return live.ports;
  const latest = runs.find((run) => run.partId === part.id && run.ports?.length);
  if (latest) return latest.ports;
  return part.ports ?? [];
}


export function displayPorts(part: ProjectPart | null | undefined, runs: Run[], runningOnly = false): PortInfo[] {
  const live = part ? runs.find((run) => run.status === "running" && run.partId === part.id) : undefined;
  if (live) {
    const process = (live.ports ?? []).filter((item) => item.source === "process");
    return dedupePorts(process);
  }
  if (runningOnly) return [];
  return dedupePorts(part?.ports ?? []);
}

export function displayRunPorts(run: Run | null | undefined): PortInfo[] {
  if (!run?.ports?.length) return [];
  return dedupePorts((run.ports ?? []).filter((item) => item.source === "process"));
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
