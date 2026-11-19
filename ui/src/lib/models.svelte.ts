import { fetchModels, requestModelsScan } from "./api";
import { pathsEqual } from "./format";
import type { ModelUsage, ModelsInventory } from "./types";

export const models = $state({
  inventory: null as ModelsInventory | null,
  error: null as string | null,
  loading: true,
});

/** Model names attributed to a project (or one of its parts) via usedBy. */
export function modelsForProject(opts: {
  projectId?: string | null;
  projectPath?: string | null;
  partIds?: string[] | null;
  partPaths?: string[] | null;
}): string[] {
  const inv = models.inventory;
  if (!inv) return [];
  const partIds = new Set((opts.partIds ?? []).filter(Boolean));
  const partPaths = (opts.partPaths ?? []).filter(Boolean);
  const hits: string[] = [];

  function matches(usage: ModelUsage): boolean {
    if (opts.projectId && (usage.projectId === opts.projectId || pathsEqual(usage.projectId, opts.projectId))) {
      return true;
    }
    if (opts.projectPath && pathsEqual(usage.projectPath, opts.projectPath)) return true;
    if (partIds.has(usage.projectId)) return true;
    if (partPaths.some((path) => pathsEqual(usage.projectPath, path) || pathsEqual(usage.projectId, path))) {
      return true;
    }
    return false;
  }

  function consider(name: string | undefined, usages: ModelUsage[] | undefined) {
    const label = (name ?? "").trim();
    if (!label) return;
    for (const usage of usages ?? []) {
      if (!matches(usage)) continue;
      if (!hits.includes(label)) hits.push(label);
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
  for (const project of inv.projectFiles ?? []) {
    for (const file of project.files ?? []) {
      consider(file.name, file.usedBy);
    }
  }
  return hits;
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
