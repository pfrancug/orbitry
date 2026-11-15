import { fetchConfig, fetchProjects, saveConfig } from "./api";
import { refreshRuns, restoreSelection, runState } from "./runs.svelte";
import type { AppConfig, RepoGroup, ScriptPattern } from "./types";

export const dash = $state({
  groups: [] as RepoGroup[],
  config: null as AppConfig | null,
  query: "",
  rootDraft: "",
  error: null as string | null,
  loading: true,
  saving: false,
  refreshing: false,
});

export const allParts = () => dash.groups.flatMap((group) => group.parts);

export const partCount = () => dash.groups.reduce((sum, group) => sum + group.parts.length, 0);

export async function loadDashboard() {
  const [nextConfig, nextProjects] = await Promise.all([fetchConfig(), fetchProjects()]);
  dash.config = nextConfig;
  dash.groups = nextProjects.groups;
  restoreSelection(nextProjects.groups.flatMap((group) => group.parts));
}

export async function refreshAll() {
  dash.refreshing = true;
  dash.error = null;
  try {
    await Promise.all([loadDashboard(), refreshRuns()]);
  } catch (err: unknown) {
    dash.error = err instanceof Error ? err.message : "Could not reach Orbitry.";
  } finally {
    dash.refreshing = false;
    dash.loading = false;
  }
}

export async function persist(next: { roots: string[]; scriptPatterns: ScriptPattern[] }) {
  dash.saving = true;
  dash.error = null;
  try {
    dash.config = await saveConfig(next);
    const { groups } = await fetchProjects();
    dash.groups = groups;
  } catch (err: unknown) {
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

export function bannerMessage(): string | null {
  return dash.error || runState.error;
}
