import { fetchMetrics } from "./api";
import type { MetricsSnapshot } from "./types";

export const metrics = $state({
  snapshot: null as MetricsSnapshot | null,
});

export async function refreshMetrics() {
  try {
    metrics.snapshot = await fetchMetrics();
  } catch {
    // Keep the last sample if a poll fails.
  }
}

export function runMetrics(runId: string | null | undefined) {
  if (!runId || !metrics.snapshot) return null;
  return metrics.snapshot.runs[runId] ?? null;
}
