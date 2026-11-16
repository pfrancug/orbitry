import { fetchModels, requestModelsScan } from "./api";
import type { ModelsInventory } from "./types";

export const models = $state({
  inventory: null as ModelsInventory | null,
  error: null as string | null,
  loading: true,
});

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
