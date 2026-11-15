import { fetchRuns, startRun, stopRun } from "./api";
import type { ProjectPart, Run } from "./types";

const SELECTED_PART_KEY = "orbitry.selectedPart";
const SELECTED_SCRIPT_KEY = "orbitry.selectedScript";

export const runState = $state({
  runs: [] as Run[],
  selectedId: null as string | null,
  selectedPartId: null as string | null,
  selectedScript: null as string | null,
  panelOpen: false,
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

export function runningFor(partId: string, script?: string | null): Run | undefined {
  return runState.runs.find(
    (run) =>
      run.status === "running" &&
      run.partId === partId &&
      (script == null || run.script === script),
  );
}

export function latestRun(partId: string, script?: string | null): Run | undefined {
  return runState.runs.find(
    (run) => run.partId === partId && (script == null || run.script === script),
  );
}

export function selectPart(partId: string, script?: string) {
  runState.selectedPartId = partId;
  if (script) {
    runState.selectedScript = script;
  } else if (runState.selectedScript) {
    // Keep the current script if it still belongs to a later caller.
  } else {
    const live = runningFor(partId);
    runState.selectedScript = live?.script ?? null;
  }
  const run =
    runningFor(partId, runState.selectedScript) ?? latestRun(partId, runState.selectedScript);
  runState.selectedId = run?.id ?? null;
  runState.panelOpen = true;
  try {
    localStorage.setItem(SELECTED_PART_KEY, partId);
    if (runState.selectedScript) {
      localStorage.setItem(SELECTED_SCRIPT_KEY, runState.selectedScript);
    }
  } catch {
    // Private mode can block storage.
  }
}

export function restoreSelection(parts: ProjectPart[]) {
  try {
    const partId = localStorage.getItem(SELECTED_PART_KEY);
    const script = localStorage.getItem(SELECTED_SCRIPT_KEY);
    if (partId && parts.some((part) => part.id === partId)) {
      selectPart(partId, script ?? undefined);
    }
  } catch {
    // Ignore storage errors on first load.
  }
}

export function closePanel() {
  runState.panelOpen = false;
}

export function openRun(id: string) {
  const run = runState.runs.find((item) => item.id === id);
  runState.selectedId = id;
  runState.panelOpen = true;
  if (run) {
    runState.selectedPartId = run.partId;
    runState.selectedScript = run.script;
  }
}

export function isPending(partId: string, script: string): boolean {
  return Boolean(runState.pending[keyOf(partId, script)]);
}

async function withPending(partId: string, script: string, work: () => Promise<void>) {
  const key = keyOf(partId, script);
  if (runState.pending[key]) return;
  runState.pending[key] = true;
  runState.error = null;
  try {
    await work();
  } catch (err: unknown) {
    runState.error = err instanceof Error ? err.message : "Could not update the process.";
  } finally {
    delete runState.pending[key];
  }
}

export async function toggleScript(partId: string, script: string) {
  const existing = runningFor(partId, script);
  await withPending(partId, script, async () => {
    if (existing) {
      const run = await stopRun(existing.id);
      upsertRun(run);
      selectPart(partId, script);
    } else {
      const run = await startRun(partId, script);
      upsertRun(run);
      selectPart(partId, script);
      openRun(run.id);
    }
  });
}

export async function startScript(partId: string, script: string) {
  if (runningFor(partId, script) || isPending(partId, script)) return;
  await withPending(partId, script, async () => {
    const run = await startRun(partId, script);
    upsertRun(run);
    selectPart(partId, script);
    openRun(run.id);
  });
}

export async function stopScript(partId: string, script: string) {
  const existing = runningFor(partId, script);
  if (!existing) return;
  await withPending(partId, script, async () => {
    const run = await stopRun(existing.id);
    upsertRun(run);
    selectPart(partId, script);
  });
}

export async function restartScript(partId: string, script: string) {
  await withPending(partId, script, async () => {
    const existing = runningFor(partId, script);
    if (existing) {
      const stopped = await stopRun(existing.id);
      upsertRun(stopped);
    }
    const run = await startRun(partId, script);
    upsertRun(run);
    selectPart(partId, script);
    openRun(run.id);
  });
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
