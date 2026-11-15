import type { ProjectKind, ProjectPart, RepoGroup } from "./types";

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
    ]),
  ]
    .join(" ")
    .toLowerCase();
}