import { fetchModels, requestModelsScan } from "./api";
import { normalizePathKey, pathsEqual } from "./format";
import type { ModelUsage, ModelsInventory, ProjectPart, RepoGroup } from "./types";

export const models = $state({
  inventory: null as ModelsInventory | null,
  error: null as string | null,
  loading: true,
});

export type AiSignal = {
  kind: "model" | "dependency";
  label: string;
};

function pathWithin(child: string, parent: string): boolean {
  const c = normalizePathKey(child);
  const p = normalizePathKey(parent);
  return c === p || c.startsWith(`${p}/`);
}

function usageMatchesProject(
  usage: ModelUsage,
  opts: {
    projectId?: string | null;
    projectPath?: string | null;
    partIds?: string[] | null;
    partPaths?: string[] | null;
  },
): boolean {
  const partIds = new Set((opts.partIds ?? []).filter(Boolean));
  const partPaths = (opts.partPaths ?? []).filter(Boolean);
  const roots = [opts.projectId, opts.projectPath, ...partIds, ...partPaths]
    .map((item) => (item ?? "").trim())
    .filter(Boolean);

  if (opts.projectId && (usage.projectId === opts.projectId || pathsEqual(usage.projectId, opts.projectId))) {
    return true;
  }
  if (opts.projectPath && pathsEqual(usage.projectPath, opts.projectPath)) return true;
  if (partIds.has(usage.projectId) || partIds.has(usage.projectPath)) return true;
  if (partPaths.some((path) => pathsEqual(usage.projectPath, path) || pathsEqual(usage.projectId, path))) {
    return true;
  }
  // File hits under the project / part tree count even when projectId differs.
  for (const file of usage.files ?? []) {
    if (roots.some((root) => pathWithin(file, root))) return true;
  }
  if (usage.projectPath && roots.some((root) => pathWithin(usage.projectPath, root) || pathWithin(root, usage.projectPath))) {
    // Prefer containment of usage inside project, not the reverse for short roots.
    if (opts.projectPath && pathWithin(usage.projectPath, opts.projectPath)) return true;
    if (partPaths.some((path) => pathWithin(usage.projectPath, path))) return true;
  }
  return false;
}

/** Model names attributed to a project (or one of its parts) via usedBy / project weights. */
export function modelsForProject(opts: {
  projectId?: string | null;
  projectPath?: string | null;
  partIds?: string[] | null;
  partPaths?: string[] | null;
}): string[] {
  const inv = models.inventory;
  if (!inv) return [];
  const hits: string[] = [];

  function push(label: string | undefined) {
    const name = (label ?? "").trim();
    if (name && !hits.includes(name)) hits.push(name);
  }

  function consider(name: string | undefined, usages: ModelUsage[] | undefined) {
    for (const usage of usages ?? []) {
      if (!usageMatchesProject(usage, opts)) continue;
      push(name);
      return;
    }
  }

  for (const model of inv.ollama?.models ?? []) {
    consider(model.name, model.usedBy);
  }
  for (const cache of inv.caches ?? []) {
    for (const model of cache.models ?? []) {
      consider(model.name, model.usedBy);
    }
  }
  // Weight files owned by this scanned project are AI even without usedBy.
  for (const project of inv.projectFiles ?? []) {
    const owned =
      (opts.projectId &&
        (project.projectId === opts.projectId || pathsEqual(project.projectId, opts.projectId))) ||
      (opts.projectPath && pathsEqual(project.projectPath, opts.projectPath)) ||
      (opts.partIds ?? []).some((id) => id && (project.projectId === id || pathsEqual(project.projectId, id))) ||
      (opts.partPaths ?? []).some(
        (path) => path && (pathsEqual(project.projectPath, path) || pathWithin(project.projectPath, path)),
      ) ||
      (opts.projectPath && pathWithin(project.projectPath, opts.projectPath));
    if (owned) {
      for (const file of project.files ?? []) push(file.name);
    }
    for (const file of project.files ?? []) {
      consider(file.name, file.usedBy);
    }
  }
  return hits;
}

export function aiDependenciesFor(
  group?: RepoGroup | null,
  part?: ProjectPart | null,
): string[] {
  const names: string[] = [];
  const pushAll = (list: string[] | null | undefined) => {
    for (const item of list ?? []) {
      const label = item.trim();
      if (label && !names.includes(label)) names.push(label);
    }
  };
  if (part) pushAll(part.aiDependencies);
  if (group) {
    for (const item of group.parts ?? []) pushAll(item.aiDependencies);
  }
  return names;
}

/** Combined AI badge signals: local/attributed models + manifest dependencies. */
export function aiSignalsForProject(opts: {
  group?: RepoGroup | null;
  part?: ProjectPart | null;
}): AiSignal[] {
  const group = opts.group ?? null;
  const part = opts.part ?? null;
  const signals: AiSignal[] = [];
  const seen = new Set<string>();

  const modelOpts = part
    ? {
        projectId: part.id,
        projectPath: part.path,
        partIds: [part.id],
        partPaths: [part.path],
      }
    : {
        projectId: group?.id,
        projectPath: group?.path,
        partIds: (group?.parts ?? []).map((item) => item.id),
        partPaths: (group?.parts ?? []).map((item) => item.path),
      };

  for (const name of modelsForProject(modelOpts)) {
    const key = `model:${name}`;
    if (seen.has(key)) continue;
    seen.add(key);
    signals.push({ kind: "model", label: name });
  }
  for (const name of aiDependenciesFor(group, part)) {
    const key = `dep:${name}`;
    if (seen.has(key)) continue;
    seen.add(key);
    signals.push({ kind: "dependency", label: name });
  }
  return signals;
}

let poll: ReturnType<typeof setInterval> | null = null;

export function applyModelsSnapshot(payload: ModelsInventory) {
  models.inventory = payload;
  models.loading = false;
  if (payload.scan?.error) {
    models.error = payload.scan.error;
  } else {
    models.error = null;
  }
  watchModelsScan();
}

export async function refreshModels() {
  try {
    applyModelsSnapshot(await fetchModels());
  } catch (err: unknown) {
    // Keep projects usable: leave inventory as-is (or null) and only set error.
    models.loading = false;
    models.error = err instanceof Error ? err.message : "Could not load the models inventory.";
  }
}

export async function rescanModels() {
  models.error = null;
  try {
    applyModelsSnapshot(await requestModelsScan());
  } catch (err: unknown) {
    models.error = err instanceof Error ? err.message : "Could not start a models scan.";
  }
}

export function modelsScanning(): boolean {
  return models.inventory?.scan?.state === "scanning";
}

function watchModelsScan() {
  const scanning = modelsScanning();
  if (!scanning) {
    if (poll) {
      clearInterval(poll);
      poll = null;
    }
    return;
  }
  if (poll) return;
  poll = setInterval(() => {
    refreshModels().catch(() => {
      // Keep the last snapshot if a poll fails.
    });
  }, 1000);
}
