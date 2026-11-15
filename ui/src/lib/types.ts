export type ScriptKind = "dev" | "preview";
export type ScriptScope = "fe" | "be" | "shared";
export type ProjectKind = "javascript" | "rust" | "python" | "go" | "docker" | "mixed";
export type PartRole = "root" | "fe" | "be" | "shared" | "lib";
export type PortSource = "default" | "config" | "env" | "flag" | "log" | "process";
export type HealthStatus = "unknown" | "ok" | "fail" | "skipped";

export type DetectedScript = {
  name: string;
  command: string;
  kind: ScriptKind;
  scope: ScriptScope;
};

export type GitInfo = {
  branch: string;
  commit: string;
  dirty: boolean;
};

export type BuildInfo = {
  built: boolean;
  dir: string | null;
  lastBuiltAt: number | null;
};

export type WarningSeverity = "info" | "warning" | "error";

export type AppWarning = {
  code: string;
  severity: WarningSeverity;
  message: string;
  hint?: string | null;
  port?: number | null;
  runId?: string | null;
  partId?: string | null;
  partName?: string | null;
  project?: string | null;
};

export type PortInfo = {
  port: number;
  source: PortSource;
  name?: string | null;
  binds?: string[];
  pid?: number | null;
  process?: string | null;
  url?: string | null;
};

export type PortHealth = {
  port: number;
  status: HealthStatus;
  url: string | null;
  name?: string | null;
};

export type HealthInfo = {
  status: HealthStatus;
  url: string | null;
  checkedAt: number | null;
  ports?: PortHealth[];
};

export type ProjectPart = {
  id: string;
  path: string;
  name: string;
  version: string | null;
  kind: ProjectKind;
  framework: string | null;
  scripts: DetectedScript[];
  git: GitInfo | null;
  build: BuildInfo;
  nodeModules: boolean;
  parent: string | null;
  role: PartRole;
  relativePath: string;
  ports: PortInfo[];
};

export type RepoGroup = {
  id: string;
  path: string;
  name: string;
  folder: string;
  version: string | null;
  kind: ProjectKind;
  framework: string | null;
  git: GitInfo | null;
  build: BuildInfo;
  nodeModules: boolean;
  parts: ProjectPart[];
};

export type ScriptPattern = {
  kind: ScriptKind;
  scope: ScriptScope;
  pattern: string;
};

export type AppConfig = {
  roots: string[];
  scriptPatterns: ScriptPattern[];
  defaultPatterns: { kind: ScriptKind; pattern: string }[];
};

export type RunStatus = "running" | "exited" | "failed";
export type LogStream = "stdout" | "stderr" | "system";

export type Run = {
  id: string;
  partId: string;
  partName: string;
  partPath: string;
  script: string;
  command: string;
  cwd: string;
  pid: number | null;
  status: RunStatus;
  exitCode: number | null;
  startedAt: number;
  endedAt: number | null;
  ports: PortInfo[];
  expectedPorts?: PortInfo[];
  health: HealthInfo;
  warnings?: AppWarning[];
};

export type LogLine = {
  seq: number;
  stream: LogStream;
  text: string;
  ts: number;
};
