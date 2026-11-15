import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { fetchConfig, fetchProjects, saveConfig } from "./api";
import type { AppConfig, Project, ScriptKind, ScriptPattern, ScriptScope } from "./types";

export function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [query, setQuery] = useState("");
  const [rootDraft, setRootDraft] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const refresh = useCallback(async () => {
    const [nextConfig, nextProjects] = await Promise.all([fetchConfig(), fetchProjects()]);
    setConfig(nextConfig);
    setProjects(nextProjects.projects);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    refresh()
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : "Could not reach Orbitry.");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  const persist = async (next: { roots: string[]; scriptPatterns: ScriptPattern[] }) => {
    setSaving(true);
    setError(null);
    try {
      const saved = await saveConfig(next);
      setConfig(saved);
      const { projects: nextProjects } = await fetchProjects();
      setProjects(nextProjects);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Could not save settings.");
    } finally {
      setSaving(false);
    }
  };

  const addRoot = async (event?: FormEvent) => {
    event?.preventDefault();
    const root = rootDraft.trim();
    if (!root || !config) return;
    if (config.roots.some((existing) => existing.toLowerCase() === root.toLowerCase())) {
      setRootDraft("");
      return;
    }
    await persist({ roots: [...config.roots, root], scriptPatterns: config.scriptPatterns });
    setRootDraft("");
  };

  const removeRoot = async (root: string) => {
    if (!config) return;
    await persist({
      roots: config.roots.filter((item) => item !== root),
      scriptPatterns: config.scriptPatterns,
    });
  };

  const addPattern = async (pattern: ScriptPattern) => {
    if (!config) return;
    const exists = config.scriptPatterns.some(
      (item) =>
        item.kind === pattern.kind &&
        item.scope === pattern.scope &&
        item.pattern.toLowerCase() === pattern.pattern.toLowerCase(),
    );
    if (exists) return;
    await persist({
      roots: config.roots,
      scriptPatterns: [...config.scriptPatterns, pattern],
    });
  };

  const removePattern = async (index: number) => {
    if (!config) return;
    await persist({
      roots: config.roots,
      scriptPatterns: config.scriptPatterns.filter((_, i) => i !== index),
    });
  };

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return projects;
    return projects.filter((project) => {
      const haystack = [
        project.name,
        project.path,
        project.version ?? "",
        project.framework ?? "",
        project.kind,
        project.git?.branch ?? "",
        ...project.scripts.map((script) => script.name),
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(needle);
    });
  }, [projects, query]);

  return (
    <div className="shell">
      <div className="glow glow-a" />
      <div className="glow glow-b" />

      <header className="topbar">
        <div className="brand">
          <span className="mark" aria-hidden="true">
            ✦
          </span>
          <div>
            <p className="logo">Orbitry</p>
            <p className="tagline">All your apps. One orbit.</p>
          </div>
        </div>

        <label className="search">
          <span className="sr-only">Search projects</span>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search projects, branches, scripts…"
          />
        </label>

        <button className="icon-btn" type="button" onClick={() => setSettingsOpen(true)}>
          Settings
        </button>
      </header>

      <section className="roots">
        <form className="root-form" onSubmit={addRoot}>
          <label className="sr-only" htmlFor="root-path">
            Project root folder
          </label>
          <input
            id="root-path"
            value={rootDraft}
            onChange={(event) => setRootDraft(event.target.value)}
            placeholder="Add a project folder…  C:\Projects\private"
            spellCheck={false}
          />
          <button type="submit" disabled={saving || !rootDraft.trim()}>
            Add folder
          </button>
        </form>
        <div className="root-list">
          {(config?.roots ?? []).map((root) => (
            <button key={root} className="root-chip" type="button" onClick={() => removeRoot(root)} title="Remove folder">
              <span>{root}</span>
              <i aria-hidden="true">×</i>
            </button>
          ))}
          {config && config.roots.length === 0 && (
            <p className="hint">Point Orbitry at a folder of projects to begin.</p>
          )}
        </div>
      </section>

      {error && <div className="banner">{error}</div>}

      <div className="meta">
        <p>
          {loading
            ? "Scanning…"
            : `${visible.length}${visible.length === projects.length ? "" : ` of ${projects.length}`} in orbit`}
        </p>
      </div>

      {loading ? (
        <div className="grid">
          {Array.from({ length: 6 }).map((_, index) => (
            <div key={index} className="card skeleton" />
          ))}
        </div>
      ) : visible.length === 0 ? (
        <div className="empty">
          <strong>{projects.length === 0 ? "Nothing in orbit yet." : "No matches."}</strong>
          <p>
            {projects.length === 0
              ? "Add a folder above and Orbitry will map every app it finds."
              : "Try a different name, branch, or script."}
          </p>
        </div>
      ) : (
        <div className="grid">
          {visible.map((project, index) => (
            <ProjectCard key={project.id} project={project} index={index} />
          ))}
        </div>
      )}

      {settingsOpen && config && (
        <SettingsPanel
          config={config}
          saving={saving}
          onClose={() => setSettingsOpen(false)}
          onAdd={addPattern}
          onRemove={removePattern}
        />
      )}
    </div>
  );
}

function ProjectCard({ project, index }: { project: Project; index: number }) {
  const shortPath = shortenPath(project.path);
  const needsInstall = isJsLike(project) && !project.nodeModules;

  return (
    <article className="card" style={{ animationDelay: `${Math.min(index, 16) * 40}ms` }}>
      <div className="card-head">
        <div>
          <h2 title={project.name}>{project.name}</h2>
          <p className="path" title={project.path}>
            {shortPath}
          </p>
        </div>
        <div className="badges">
          {project.version && <span className="badge quiet">v{project.version}</span>}
          {project.framework && <span className="badge framework">{project.framework}</span>}
        </div>
      </div>

      <div className="scripts">
        {project.scripts.length === 0 ? (
          <span className="muted">No dev or preview scripts</span>
        ) : (
          project.scripts.map((script) => (
            <span key={`${script.kind}-${script.name}`} className={`chip scope-${script.scope}`}>
              <em>{script.scope}</em>
              {script.name}
            </span>
          ))
        )}
      </div>

      <footer className="card-foot">
        <span className="git">
          {project.git ? (
            <>
              <span className={`dot ${project.git.dirty ? "dirty" : "clean"}`} title={project.git.dirty ? "Uncommitted changes" : "Clean working tree"} />
              <span title={project.git.commit}>{project.git.branch}</span>
            </>
          ) : (
            <span className="muted">No git</span>
          )}
        </span>
        <span className={`build ${project.build.built ? "ok" : ""}`}>
          {project.build.built
            ? `Built ${timeAgo(project.build.lastBuiltAt)}`
            : "Not built"}
        </span>
      </footer>
      {needsInstall && <p className="warn">Dependencies not installed</p>}
    </article>
  );
}

function SettingsPanel({
  config,
  saving,
  onClose,
  onAdd,
  onRemove,
}: {
  config: AppConfig;
  saving: boolean;
  onClose: () => void;
  onAdd: (pattern: ScriptPattern) => Promise<void>;
  onRemove: (index: number) => Promise<void>;
}) {
  const [kind, setKind] = useState<ScriptKind>("dev");
  const [scope, setScope] = useState<ScriptScope>("shared");
  const [pattern, setPattern] = useState("");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const value = pattern.trim();
    if (!value) return;
    await onAdd({ kind, scope, pattern: value });
    setPattern("");
  };

  return (
    <div className="overlay" onClick={onClose} role="presentation">
      <aside className="sheet" onClick={(event) => event.stopPropagation()} role="dialog" aria-labelledby="settings-title">
        <header>
          <div>
            <h2 id="settings-title">Script patterns</h2>
            <p>Orbitry already looks for dev and preview commands. Add your own, and they join the scan.</p>
          </div>
          <button className="icon-btn" type="button" onClick={onClose}>
            Close
          </button>
        </header>

        <div className="defaults">
          <p className="label">Built in</p>
          <div className="chips">
            {config.defaultPatterns.map((item) => (
              <span key={`${item.kind}-${item.pattern}`} className="chip quiet">
                <em>{item.kind}</em>
                {item.pattern}
              </span>
            ))}
          </div>
        </div>

        <form className="pattern-form" onSubmit={submit}>
          <select value={kind} onChange={(event) => setKind(event.target.value as ScriptKind)}>
            <option value="dev">dev</option>
            <option value="preview">preview / start</option>
          </select>
          <select value={scope} onChange={(event) => setScope(event.target.value as ScriptScope)}>
            <option value="shared">shared</option>
            <option value="fe">frontend</option>
            <option value="be">backend</option>
          </select>
          <input
            value={pattern}
            onChange={(event) => setPattern(event.target.value)}
            placeholder="e.g. dev:mobile or serve*"
            spellCheck={false}
          />
          <button type="submit" disabled={saving || !pattern.trim()}>
            Add
          </button>
        </form>

        <div className="custom-list">
          {config.scriptPatterns.length === 0 ? (
            <p className="hint">No custom patterns yet. They live here, not in a config file you have to edit.</p>
          ) : (
            config.scriptPatterns.map((item, index) => (
              <button
                key={`${item.kind}-${item.scope}-${item.pattern}-${index}`}
                className="root-chip"
                type="button"
                onClick={() => onRemove(index)}
                title="Remove pattern"
              >
                <span>
                  {item.kind} · {item.scope} · {item.pattern}
                </span>
                <i aria-hidden="true">×</i>
              </button>
            ))
          )}
        </div>
      </aside>
    </div>
  );
}

function isJsLike(project: Project): boolean {
  return project.kind === "javascript" || project.kind === "mixed";
}

function shortenPath(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 3) return path;
  return parts.slice(-3).join(" / ");
}

function timeAgo(unix: number | null): string {
  if (!unix) return "recently";
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - unix));
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 14) return `${days}d ago`;
  const date = new Date(unix * 1000);
  return date.toLocaleDateString();
}