import { fetchRuns, startRun, stopAllRuns, stopRun } from "./api";
import { groupNameFor } from "./format";
import { rememberRecent } from "./recent";
import type { AppWarning, ProjectPart, RepoGroup, Run } from "./types";

const SELECTED_PART_KEY = "orbitry.selectedPart";
const SELECTED_SCRIPT_KEY = "orbitry.selectedScript";
const PANEL_KEY = "orbitry.panelOpen";

export const runState = $state({
  runs: [] as Run[],
  warnings: [] as AppWarning[],
  selectedId: null as string | null,
  selectedPartId: null as string | null,
  selectedScript: null as string | null,
  panelOpen: false,
  error: null as string | null,
  pending: {} as Record<string, boolean>,
  stoppingAll: false,
});

function keyOf(partId: string, script: string): string {
  return `${partId}::${script}`;
}

function persistSelection() {
  try {
    if (runState.selectedPartId) localStorage.setItem(SELECTED_PART_KEY, runState.selectedPartId);
    if (runState.selectedScript) localStorage.setItem(SELECTED_SCRIPT_KEY, runState.selectedScript);
    localStorage.setItem(PANEL_KEY, runState.panelOpen ? "1" : "0");
  } catch {
    // Private mode can block storage.
  }
}

export function upsertRun(run: Run) {
  const index = runState.runs.findIndex((item) => item.id === run.id);
  if (index >= 0) runState.runs[index] = run;
  else runState.runs = [run, ...runState.runs];
}

export async function refreshRuns() {
  const { runs, warnings } = await fetchRuns();
  runState.runs = runs;
  runState.warnings = warnings ?? [];
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

export function setPanelOpen(open: boolean) {
  runState.panelOpen = open;
  persistSelection();
}

export function closePanel() {
  setPanelOpen(false);
}

export function selectPart(partId: string, script?: string, openPanel = true) {
  runState.selectedPartId = partId;
  if (script) {
    runState.selectedScript = script;
  } else if (!runState.selectedScript) {
    const live = runningFor(partId);
    runState.selectedScript = live?.script ?? null;
  }
  const run =
    runningFor(partId, runState.selectedScript) ?? latestRun(partId, runState.selectedScript);
  runState.selectedId = run?.id ?? null;
  if (openPanel) runState.panelOpen = true;
  persistSelection();
}

/** Select an app, or close the panel if this app is already selected and open. */
export function togglePart(partId: string, script?: string) {
  const samePart = runState.selectedPartId === partId;
  const sameScript = !script || script === runState.selectedScript;
  if (samePart && sameScript && runState.panelOpen) {
    closePanel();
    return;
  }
  selectPart(partId, script, true);
}

export function restoreSelection(parts: ProjectPart[]) {
  try {
    const partId = localStorage.getItem(SELECTED_PART_KEY);
    const script = localStorage.getItem(SELECTED_SCRIPT_KEY);
    const panel = localStorage.getItem(PANEL_KEY) === "1";
    if (partId && parts.some((part) => part.id === partId)) {
      selectPart(partId, script ?? undefined, panel);
    } else {
      runState.panelOpen = false;
    }
  } catch {
    // Ignore storage errors on first load.
  }
}

export function openRun(id: string, openPanel = true) {
  const run = runState.runs.find((item) => item.id === id);
  runState.selectedId = id;
  if (run) {
    runState.selectedPartId = run.partId;
    runState.selectedScript = run.script;
  }
  if (openPanel) runState.panelOpen = true;
  persistSelection();
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

function remember(partId: string, script: string, groups: RepoGroup[]) {
  const partName =
    groups.flatMap((group) => group.parts).find((part) => part.id === partId)?.name ?? partId;
  rememberRecent({
    partId,
    partName,
    groupName: groupNameFor(partId, groups),
    script,
  });
}

export async function toggleScript(partId: string, script: string, groups: RepoGroup[] = []) {
  const existing = runningFor(partId, script);
  await withPending(partId, script, async () => {
    if (existing) {
      const run = await stopRun(existing.id);
      upsertRun(run);
      selectPart(partId, script, runState.panelOpen);
    } else {
      const run = await startRun(partId, script);
      upsertRun(run);
      remember(partId, script, groups);
      selectPart(partId, script, true);
      openRun(run.id);
    }
  });
}

export async function startScript(partId: string, script: string, groups: RepoGroup[] = []) {
  if (runningFor(partId, script) || isPending(partId, script)) return;
  await withPending(partId, script, async () => {
    const run = await startRun(partId, script);
    upsertRun(run);
    remember(partId, script, groups);
    selectPart(partId, script, true);
    openRun(run.id);
  });
}

export async function stopScript(partId: string, script: string) {
  const existing = runningFor(partId, script);
  if (!existing) return;
  await withPending(partId, script, async () => {
    const run = await stopRun(existing.id);
    upsertRun(run);
    selectPart(partId, script, runState.panelOpen);
  });
}

export async function restartScript(partId: string, script: string, groups: RepoGroup[] = []) {
  await withPending(partId, script, async () => {
    const existing = runningFor(partId, script);
    if (existing) {
      const stopped = await stopRun(existing.id);
      upsertRun(stopped);
    }
    const run = await startRun(partId, script);
    upsertRun(run);
    remember(partId, script, groups);
    selectPart(partId, script, runState.panelOpen || true);
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

export async function stopAll() {
  if (runState.stoppingAll) return;
  runState.stoppingAll = true;
  runState.error = null;
  try {
    const { runs, warnings } = await stopAllRuns();
    runState.runs = runs;
    runState.warnings = warnings ?? [];
  } catch (err: unknown) {
    runState.error = err instanceof Error ? err.message : "Could not stop running apps.";
  } finally {
    runState.stoppingAll = false;
  }
}
