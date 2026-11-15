export type ScriptKind = "dev" | "preview";
export type ScriptScope = "fe" | "be" | "shared";
export type ProjectKind = "javascript" | "rust" | "python" | "go" | "docker" | "mixed";

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

export type Project = {
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