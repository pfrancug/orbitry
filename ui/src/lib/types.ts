export type ScriptKind = "dev" | "preview" | "build" | "other";
export type ScriptScope = "fe" | "be" | "shared";
export type ProjectKind = "javascript" | "rust" | "python" | "go" | "docker" | "mixed";
export type PartRole = "root" | "fe" | "be" | "shared" | "lib";
export type PortSource = "default" | "config" | "env" | "flag" | "bridge" | "log" | "process";
export type DatabaseSource = "env" | "prisma" | "compose" | "dependency" | "file";
export type ConfigBridgeMode = "auto" | "on" | "off";

export type BridgeState = "available" | "node-missing" | "plugin-missing" | "disabled" | "error";

export type BridgeStatus = {
  mode: ConfigBridgeMode;
  available: boolean;
  node: boolean;
  plugin: boolean;
  label: string;
  state?: BridgeState | string;
  nodeVersion?: string | null;
  nodePath?: string | null;
  error?: string | null;
};
export type HealthStatus = "unknown" | "ok" | "fail" | "skipped";

export type ScriptSource = "package" | "workspace";

export type DetectedScript = {
  name: string;
  command: string;
  kind: ScriptKind;
  scope: ScriptScope;
  cwd?: string | null;
  source?: ScriptSource;
  sourceName?: string | null;
  label?: string | null;
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
  stale?: boolean;
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
  healthUrl?: string | null;
  ignored?: boolean;
};

/** Local database detected during project scan. Optional on older payloads. */
export type DatabaseInfo = {
  engine: string;
  source: DatabaseSource | string;
  detail?: string | null;
  host?: string | null;
  port?: number | null;
  path?: string | null;
};

export type PortOverride = {
  partId: string;
  port: number;
  name?: string | null;
  healthUrl?: string | null;
  ignore?: boolean;
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
  packageManager?: string | null;
  databases?: DatabaseInfo[];
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

export type AutostartStatus = {
  enabled: boolean;
  command?: string | null;
  stale: boolean;
};

/** Which Open-in actions appear in the details panel. Missing fields default to true. */
export type OpenInTargets = {
  terminal?: boolean;
  vscode?: boolean;
  cursor?: boolean;
  explorer?: boolean;
};

export type OpenInKind = "terminal" | "vscode" | "cursor" | "explorer";

export type AppConfig = {
  roots: string[];
  scriptPatterns: ScriptPattern[];
  defaultPatterns: { kind: ScriptKind; pattern: string }[];
  portOverrides?: PortOverride[];
  configBridge?: ConfigBridgeMode;
  autoCheckUpdates?: boolean;
  logLimitMb?: number;
  autostartApps?: string[];
  desktopNotifications?: boolean;
  ignoredProjects?: string[];
  /** Present from 0.11.0. Older configs omit it; every target stays visible. */
  openIn?: OpenInTargets;
  bridgeStatus?: BridgeStatus;
  autostart?: AutostartStatus;
};

export type AppAutostartOutcome = "started" | "skipped" | "blocked" | "failed";
export type AppAutostartState = "idle" | "running" | "done";

export type AppAutostartEvent = {
  partId: string;
  partName: string;
  script?: string | null;
  outcome: AppAutostartOutcome;
  reason?: string | null;
  port?: number | null;
  process?: string | null;
};

export type AppAutostartReport = {
  state: AppAutostartState;
  started?: AppAutostartEvent[];
  skipped?: AppAutostartEvent[];
  blocked?: AppAutostartEvent[];
  failed?: AppAutostartEvent[];
  completedAt?: number | null;
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
  processStart?: number | null;
  logPath?: string | null;
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

export type ScanState = "cached" | "scanning" | "fresh";
export type ScanChangeKind = "project" | "part" | "script";

export type ScanChange = {
  kind: ScanChangeKind;
  project: string;
  projectId: string;
  part?: string | null;
  partId?: string | null;
  script?: string | null;
  detail?: string | null;
};

export type ScanDiff = {
  added: ScanChange[];
  removed: ScanChange[];
  changed: ScanChange[];
};

export type ScanStatus = {
  state: ScanState;
  stale: boolean;
  scannedAt?: number | null;
  startedAt?: number | null;
  diff?: ScanDiff | null;
  error?: string | null;
};

export type ComposeService = {
  name: string;
  state?: string;
  status?: string | null;
  ports?: string[];
  container?: string | null;
};

export type ComposeProject = {
  projectId: string;
  projectName: string;
  workdir: string;
  file: string;
  services?: ComposeService[];
  dockerAvailable: boolean;
  dockerCommand?: string | null;
  error?: string | null;
};

export type ProjectsResponse = {
  groups: RepoGroup[];
  warnings?: AppWarning[];
  scan?: ScanStatus;
  appAutostart?: AppAutostartReport;
  compose?: ComposeProject[];
};

export type UpdatePhase = "idle" | "downloading" | "verifying" | "installing" | "restarting" | "error";

export type VersionInfo = {
  current: string;
  latest?: string | null;
  updateAvailable: boolean;
  notesUrl?: string | null;
  notes?: string | null;
  checking: boolean;
  installing: boolean;
  error?: string | null;
  autoCheck: boolean;
  dev: boolean;
  phase?: UpdatePhase;
  progress?: number | null;
  message?: string | null;
  rolledBack?: boolean;
  checkedAt?: number | null;
};

export type SystemMetrics = {
  cpuPct: number;
  memUsed: number;
  memTotal: number;
};

export type RunMetrics = {
  cpuPct: number;
  memBytes: number;
  pidCount: number;
};

export type MetricsSnapshot = {
  sampledAt: number;
  system: SystemMetrics;
  runs: Record<string, RunMetrics>;
  history: { cpu: number[]; memPct: number[] };
};

export type ToolsInfo = {
  vscodeInstalled: boolean;
  /** Present from 0.11.0; older servers omit it. */
  cursorInstalled?: boolean;
};

export type UpdateResponse = VersionInfo & {
  started: boolean;
  needsConfirm: boolean;
  running: number;
};

export type ModelsScanState = "idle" | "scanning" | "fresh" | "cached";

export type ModelsScanStatus = {
  state: ModelsScanState;
  scannedAt?: number | null;
  startedAt?: number | null;
  error?: string | null;
  stale?: boolean;
};

export type ModelUsage = {
  projectId: string;
  projectName: string;
  projectPath: string;
  files?: string[];
};

export type OllamaModel = {
  name: string;
  id?: string | null;
  sizeBytes: number;
  modifiedAt?: number | null;
  usedBy?: ModelUsage[];
};

export type OllamaInventory = {
  installed: boolean;
  version?: string | null;
  installPath?: string | null;
  serviceUrl: string;
  serviceReachable: boolean;
  modelsDir: string;
  modelsDirBytes: number;
  source: string;
  models?: OllamaModel[];
};

export type CachedModel = {
  name: string;
  path: string;
  sizeBytes: number;
  usedBy?: ModelUsage[];
};

export type CacheSource = {
  id: string;
  label: string;
  path: string;
  present: boolean;
  totalBytes: number;
  models?: CachedModel[];
};

export type WeightFile = {
  name: string;
  path: string;
  sizeBytes: number;
  usedBy?: ModelUsage[];
};

export type ProjectWeights = {
  projectId: string;
  projectName: string;
  projectPath: string;
  totalBytes: number;
  files?: WeightFile[];
};

export type SourceTotal = {
  id: string;
  label: string;
  path?: string | null;
  bytes: number;
};

export type ModelsInventory = {
  scan: ModelsScanStatus;
  ollama: OllamaInventory;
  caches?: CacheSource[];
  projectFiles?: ProjectWeights[];
  totals?: SourceTotal[];
  grandTotalBytes: number;
  fingerprint?: string | null;
  scannerVersion?: number;
};
