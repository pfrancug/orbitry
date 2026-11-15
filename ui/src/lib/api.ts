import type { AppConfig, RepoGroup, ScriptPattern } from "./types";

async function request<T>(input: string, init?: RequestInit): Promise<T> {
  const response = await fetch(input, {
    ...init,
    headers: {
      Accept: "application/json",
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...init?.headers,
    },
  });
  if (!response.ok) {
    const detail = await response.text();
    throw new Error(detail || `${response.status} ${response.statusText}`);
  }
  return response.json() as Promise<T>;
}

export function fetchProjects(): Promise<{ groups: RepoGroup[] }> {
  return request("/api/projects");
}

export function fetchConfig(): Promise<AppConfig> {
  return request("/api/config");
}

export function saveConfig(config: {
  roots: string[];
  scriptPatterns: ScriptPattern[];
}): Promise<AppConfig> {
  return request("/api/config", {
    method: "PUT",
    body: JSON.stringify(config),
  });
}