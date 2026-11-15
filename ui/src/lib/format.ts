import type { HealthInfo, PartRole, PortInfo, ProjectKind, ProjectPart, RepoGroup, Run } from "./types";

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

export function portUrl(port: number): string {
  return `http://127.0.0.1:${port}`;
}

export function portsFor(part: ProjectPart, runs: Run[]): PortInfo[] {
  const live = runs.find((run) => run.status === "running" && run.partId === part.id);
  if (live?.ports?.length) return live.ports;
  const latest = runs.find((run) => run.partId === part.id && run.ports?.length);
  if (latest) return latest.ports;
  return part.ports ?? [];
}

export function healthLabel(health: HealthInfo | undefined): string {
  if (!health || health.status === "unknown") return "idle";
  if (health.status === "ok") return "healthy";
  if (health.status === "fail") return "down";
  return "no port";
}

export function groupNameFor(partId: string, groups: RepoGroup[]): string {
  const group = groups.find((item) => item.parts.some((part) => part.id === partId));
  return group?.name ?? "";
}

export function findPart(partId: string, groups: RepoGroup[]): ProjectPart | undefined {
  for (const group of groups) {
    const part = group.parts.find((item) => item.id === partId);
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
    ...group.parts.flatMap((part) => [
      part.name,
      part.path,
      part.relativePath,
      part.framework ?? "",
      part.role,
      ...part.scripts.map((script) => script.name),
      ...part.ports.map((item) => String(item.port)),
    ]),
  ]
    .join(" ")
    .toLowerCase();
}
