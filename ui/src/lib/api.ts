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

export function openPath(kind: "terminal" | "vscode", path: string): Promise<void> {
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

export function stopRun(runId: string): Promise<Run> {
  return request("/api/stop", {
    method: "POST",
    body: JSON.stringify({ runId }),
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
