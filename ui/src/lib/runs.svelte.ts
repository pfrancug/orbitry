import { fetchRuns, startRun, stopRun } from "./api";
import type { Run } from "./types";

export const runState = $state({
  runs: [] as Run[],
  selectedId: null as string | null,
  drawerOpen: false,
  error: null as string | null,
  pending: {} as Record<string, boolean>,
});

function keyOf(partId: string, script: string): string {
  return `${partId}::${script}`;
}

export function upsertRun(run: Run) {
  const index = runState.runs.findIndex((item) => item.id === run.id);
  if (index >= 0) runState.runs[index] = run;
  else runState.runs = [run, ...runState.runs];
}

export async function refreshRuns() {
  const { runs } = await fetchRuns();
  runState.runs = runs;
}

export function openRun(id: string) {
  runState.selectedId = id;
  runState.drawerOpen = true;
}

export function closeDrawer() {
  runState.drawerOpen = false;
}

export function isPending(partId: string, script: string): boolean {
  return Boolean(runState.pending[keyOf(partId, script)]);
}

export async function toggleScript(partId: string, script: string) {
  const key = keyOf(partId, script);
  if (runState.pending[key]) return;
  const existing = runState.runs.find(
    (run) => run.status === "running" && run.partId === partId && run.script === script,
  );
  runState.pending[key] = true;
  runState.error = null;
  try {
    if (existing) {
      const run = await stopRun(existing.id);
      upsertRun(run);
    } else {
      const run = await startRun(partId, script);
      upsertRun(run);
      openRun(run.id);
    }
  } catch (err: unknown) {
    runState.error = err instanceof Error ? err.message : "Could not update the process.";
  } finally {
    delete runState.pending[key];
  }
}

export async function stopSelected() {
  const id = runState.selectedId;
  if (!id) return;
  const current = runState.runs.find((run) => run.id === id);
  if (!current || current.status !== "running") return;
  runState.error = null;
  try {
    const run = await stopRun(id);
    upsertRun(run);
  } catch (err: unknown) {
    runState.error = err instanceof Error ? err.message : "Could not stop the process.";
  }
}
