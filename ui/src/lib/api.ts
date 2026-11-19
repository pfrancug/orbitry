import type { AppConfig, AppWarning, MetricsSnapshot, ModelsInventory, PortOverride, ProjectsResponse, Run, ScriptPattern, ToolsInfo, UpdateResponse, VersionInfo } from "./types";

async function request<T>(input: string, init?: RequestInit): Promise<T> {
  const response = await fetch(input, {
    ...init,
    headers: {
      Accept: "application/json",
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...init?.headers,
    },
  });
  const detail = await response.text();
  if (!response.ok) {
    throw new Error(detail || `${response.status} ${response.statusText}`);
  }
  if (!detail) {
    return undefined as T;
  }
  try {
    return JSON.parse(detail) as T;
  } catch {
    throw new Error(`Invalid JSON from ${input}`);
  }
}

export function fetchProjects(): Promise<ProjectsResponse> {
  return request("/api/projects");
}

export function requestScan(): Promise<ProjectsResponse> {
  return request("/api/scan", { method: "POST" });
}

export function fetchConfig(): Promise<AppConfig> {
  return request("/api/config");
}

export function saveConfig(config: {
  roots: string[];
  scriptPatterns: ScriptPattern[];
  portOverrides?: PortOverride[];
  configBridge?: import("./types").ConfigBridgeMode;
  autoCheckUpdates?: boolean;
  logLimitMb?: number;
  startWithSystem?: boolean;
  autostartApps?: string[];
  desktopNotifications?: boolean;
  ignoredProjects?: string[];
  openIn?: import("./types").OpenInTargets;
  modelsIncludeDocs?: boolean;
  ahkPlugin?: boolean;
}): Promise<AppConfig> {
  return request("/api/config", {
    method: "PUT",
    body: JSON.stringify(config),
  });
}

export function pickFolder(): Promise<{ path: string } | null> {
  return fetch("/api/pick-folder", {
    method: "POST",
    headers: { Accept: "application/json" },
  }).then(async (response) => {
    if (response.status === 204) return null;
    if (!response.ok) {
      const detail = await response.text();
      throw new Error(detail || `${response.status} ${response.statusText}`);
    }
    return response.json() as Promise<{ path: string }>;
  });
}

export function openPath(kind: "terminal" | "vscode" | "cursor" | "explorer", path: string): Promise<void> {
  return fetch("/api/open", {
    method: "POST",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ kind, path }),
  }).then(async (response) => {
    if (!response.ok) {
      const detail = await response.text();
      throw new Error(detail || `${response.status} ${response.statusText}`);
    }
  });
}

export function fetchRuns(): Promise<{
  runs: Run[];
  warnings?: AppWarning[];
  appAutostart?: import("./types").AppAutostartReport;
}> {
  return request("/api/runs");
}

export function startRun(partId: string, script: string): Promise<Run> {
  return request("/api/run", {
    method: "POST",
    body: JSON.stringify({ partId, script }),
  });
}

export function startInstall(partId: string): Promise<Run> {
  return request("/api/install", {
    method: "POST",
    body: JSON.stringify({ partId }),
  });
}

export function stopRun(
  runId: string | null | undefined,
  opts?: { partId?: string; script?: string },
): Promise<Run> {
  const body: { runId?: string; partId?: string; script?: string } = {};
  if (runId) body.runId = runId;
  if (opts?.partId) body.partId = opts.partId;
  if (opts?.script) body.script = opts.script;
  return request("/api/stop", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function stopAllRuns(): Promise<{ runs: Run[]; warnings?: AppWarning[] }> {
  return request("/api/stop-all", { method: "POST" });
}

export function logsSocket(runId: string): WebSocket {
  const protocol = location.protocol === "https:" ? "wss" : "ws";
  return new WebSocket(`${protocol}://${location.host}/api/runs/${runId}/logs`);
}

export function fetchVersion(): Promise<VersionInfo> {
  return request("/api/version");
}

export function checkForUpdates(): Promise<VersionInfo> {
  return request("/api/update/check", { method: "POST" });
}

export function startUpdate(confirm = false): Promise<UpdateResponse> {
  return request("/api/update", {
    method: "POST",
    body: JSON.stringify({ confirm }),
  });
}

export function fetchUpdateStatus(): Promise<VersionInfo> {
  return request("/api/update/status");
}

export function fetchTools(): Promise<ToolsInfo> {
  return request("/api/tools");
}

export function fetchMetrics(): Promise<MetricsSnapshot> {
  return request("/api/metrics");
}

export function fetchModels(): Promise<ModelsInventory> {
  return request("/api/models");
}

export function requestModelsScan(): Promise<ModelsInventory> {
  return request("/api/models", { method: "POST" });
}

export function fetchDocker(): Promise<import("./types").ComposeProject[]> {
  return request("/api/docker");
}

export function runDockerAction(
  action: "up" | "down" | "restart",
  projectId: string,
  service?: string | null,
): Promise<import("./types").ComposeProject[]> {
  return request("/api/docker", {
    method: "POST",
    body: JSON.stringify({ action, projectId, service: service || undefined }),
  });
}

export function fetchDockerLogs(projectId: string, service?: string | null): Promise<{ text: string }> {
  const params = new URLSearchParams({ projectId });
  if (service) params.set("service", service);
  return request(`/api/docker/logs?${params}`);
}
