import { fetchDockerLogs, runDockerAction } from "./api";
import { dash } from "./dashboard.svelte";
import { t } from "./i18n.svelte";

export const dockerUi = $state({
  busy: null as string | null,
  logs: null as { projectId: string; service?: string | null; text: string } | null,
  logError: null as string | null,
});

export async function dockerAction(
  action: "up" | "down" | "restart",
  projectId: string,
  service?: string | null,
) {
  dockerUi.busy = `${action}:${projectId}:${service ?? ""}`;
  dash.error = null;
  try {
    dash.compose = await runDockerAction(action, projectId, service);
  } catch (err: unknown) {
    dash.error = err instanceof Error ? err.message : t("docker.error", { error: String(err) });
  } finally {
    dockerUi.busy = null;
  }
}

export async function loadDockerLogs(projectId: string, service?: string | null) {
  dockerUi.busy = `logs:${projectId}:${service ?? ""}`;
  dockerUi.logError = null;
  try {
    const { text } = await fetchDockerLogs(projectId, service);
    dockerUi.logs = { projectId, service, text };
  } catch (err: unknown) {
    dockerUi.logError = err instanceof Error ? err.message : String(err);
    dockerUi.logs = { projectId, service, text: "" };
  } finally {
    dockerUi.busy = null;
  }
}

export function closeDockerLogs() {
  dockerUi.logs = null;
  dockerUi.logError = null;
}
