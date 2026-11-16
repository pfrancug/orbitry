use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::artifacts::BuildInfo;
use crate::config::{canonical, display_path, normalize_path_str, paths_equal};
use crate::git::find_git_root;
use crate::scan::{Project, ProjectKind};
use crate::ports::PortInfo;
use crate::process::detect_package_manager;
use crate::scripts::{workspace_target, DetectedScript, ScriptSource};
use crate::GitInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartRole {
    Root,
    Fe,
    Be,
    Shared,
    Lib,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPart {
    pub id: String,
    pub path: String,
    pub name: String,
    pub version: Option<String>,
    pub kind: ProjectKind,
    pub framework: Option<String>,
    pub scripts: Vec<DetectedScript>,
    pub git: Option<GitInfo>,
    pub build: BuildInfo,
    pub node_modules: bool,
    pub parent: Option<String>,
    pub role: PartRole,
    pub relative_path: String,
    pub ports: Vec<PortInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoGroup {
    pub id: String,
    pub path: String,
    pub name: String,
    pub folder: String,
    pub version: Option<String>,
    pub kind: ProjectKind,
    pub framework: Option<String>,
    pub git: Option<GitInfo>,
    pub build: BuildInfo,
    pub node_modules: bool,
    pub parts: Vec<ProjectPart>,
}

pub fn group_projects(projects: Vec<Project>, roots: &[String]) -> Vec<RepoGroup> {
    let mut buckets: BTreeMap<String, Vec<Project>> = BTreeMap::new();
    for project in projects {
        let key = group_key(&project, roots);
        buckets.entry(key).or_default().push(project);
    }

    let mut groups: Vec<RepoGroup> = buckets
        .into_iter()
        .map(|(id, members)| build_group(id, members))
        .collect();

    groups.sort_by(|a, b| {
        let a_dirty = a.git.as_ref().is_some_and(|git| git.dirty);
        let b_dirty = b.git.as_ref().is_some_and(|git| git.dirty);
        b_dirty.cmp(&a_dirty).then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });
    groups
}

fn group_key(project: &Project, roots: &[String]) -> String {
    let path = PathBuf::from(&project.path);
    if let Some(git_root) = find_git_root(&path) {
        return display_path(&canonical(&git_root));
    }
    if let Some(under_root) = first_child_of_roots(&path, roots) {
        return under_root;
    }
    project.path.clone()
}

fn first_child_of_roots(path: &Path, roots: &[String]) -> Option<String> {
    let rendered = display_path(&canonical(path));
    let normalized = normalize_path_str(&rendered);
    for root in roots {
        let root_norm = normalize_path_str(&display_path(Path::new(root)));
        let matches = if cfg!(windows) {
            normalized.len() > root_norm.len()
                && normalized.as_bytes().get(root_norm.len()) == Some(&b'\\')
                && normalized[..root_norm.len()].eq_ignore_ascii_case(&root_norm)
        } else {
            normalized.len() > root_norm.len()
                && normalized.as_bytes().get(root_norm.len()) == Some(&b'/')
                && normalized.starts_with(&root_norm)
        };
        if !matches {
            continue;
        }
        let rest = &normalized[root_norm.len() + 1..];
        let first = rest.split(['\\', '/']).next().filter(|s| !s.is_empty())?;
        let joined = Path::new(root).join(first);
        return Some(display_path(&canonical(&joined)));
    }
    None
}

fn build_group(id: String, members: Vec<Project>) -> RepoGroup {
    let path = id.clone();
    let folder = Path::new(&path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let root = members
        .iter()
        .find(|project| paths_equal(&project.path, &path));
    let name = root
        .map(|project| project.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| folder.clone());
    let version = root
        .and_then(|project| project.version.clone())
        .or_else(|| members.iter().find_map(|project| project.version.clone()));
    let kind = unify_kind(members.iter().map(|project| project.kind));
    let framework = root
        .and_then(|project| project.framework.clone())
        .or_else(|| {
            members.iter().find_map(|project| {
                project
                    .framework
                    .clone()
                    .filter(|value| value != "rust" && value != "python" && value != "go")
            })
        })
        .or_else(|| members.iter().find_map(|project| project.framework.clone()));
    let git = root
        .and_then(|project| project.git.clone())
        .or_else(|| members.iter().find_map(|project| project.git.clone()));
    let build = aggregate_build(&members);
    let node_modules = members
        .iter()
        .filter(|project| is_js_like(project.kind))
        .all(|project| project.node_modules)
        || members.iter().all(|project| !is_js_like(project.kind));

    let mut parts: Vec<ProjectPart> = members
        .into_iter()
        .map(|project| {
            let relative_path = relative_to(&project.path, &path);
            let role = part_role(&project, &path, &relative_path);
            let package_manager = if is_js_like(project.kind) {
                Some(detect_package_manager(Path::new(&project.path)).to_string())
            } else {
                None
            };
            ProjectPart {
                id: project.id,
                path: project.path,
                name: project.name,
                version: project.version,
                kind: project.kind,
                framework: project.framework,
                scripts: project.scripts,
                git: project.git,
                build: project.build,
                node_modules: project.node_modules,
                parent: project.parent,
                role,
                relative_path,
                ports: project.ports,
                package_manager,
            }
        })
        .collect();

    parts.sort_by(|a, b| {
        role_rank(a.role).cmp(&role_rank(b.role)).then_with(|| {
            a.relative_path
                .to_ascii_lowercase()
                .cmp(&b.relative_path.to_ascii_lowercase())
        })
    });
    attach_workspace_scripts(&mut parts, &path, &name);

    RepoGroup {
        id,
        path,
        name,
        folder,
        version,
        kind,
        framework,
        git,
        build,
        node_modules,
        parts,
    }
}

fn part_role(project: &Project, group_path: &str, relative_path: &str) -> PartRole {
    if relative_path == "." || paths_equal(&project.path, group_path) {
        return PartRole::Root;
    }

    let rel = relative_path.replace('\\', "/").to_ascii_lowercase();
    let folder = Path::new(&rel)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&rel);
    let name = project.name.to_ascii_lowercase();

    if is_lib_hint(folder, &name, &rel) {
        return PartRole::Lib;
    }
    if is_fe_hint(folder, &name, &rel) {
        return PartRole::Fe;
    }
    if is_be_hint(folder, &name, &rel) {
        return PartRole::Be;
    }

    let fe = project
        .scripts
        .iter()
        .any(|script| matches!(script.scope, crate::ScriptScope::Fe));
    let be = project
        .scripts
        .iter()
        .any(|script| matches!(script.scope, crate::ScriptScope::Be));
    match (fe, be) {
        (true, false) => PartRole::Fe,
        (false, true) => PartRole::Be,
        (true, true) => PartRole::Shared,
        (false, false) => {
            if rel.starts_with("packages/") {
                PartRole::Lib
            } else {
                PartRole::Shared
            }
        }
    }
}

fn is_lib_hint(folder: &str, name: &str, rel: &str) -> bool {
    const LIBS: &[&str] = &[
        "shared",
        "core",
        "lib",
        "libs",
        "common",
        "types",
        "utils",
        "ml",
        "packages/core",
        "packages/shared",
        "packages/types",
        "packages/lib",
    ];
    LIBS.iter()
        .any(|hint| folder == *hint || name.ends_with(&format!("/{hint}")) || rel == *hint)
        || name.contains("/shared")
        || name.contains("/core")
        || name.ends_with("-shared")
        || name.ends_with("-core")
}

fn is_fe_hint(folder: &str, name: &str, rel: &str) -> bool {
    const FES: &[&str] = &[
        "web",
        "ui",
        "frontend",
        "client",
        "site",
        "extension",
        "desktop",
        "demo",
        "react-demo",
        "apps/web",
        "apps/frontend",
        "apps/ui",
        "apps/extension",
        "apps/demo",
        "apps/react-demo",
    ];
    FES.iter()
        .any(|hint| folder == *hint || rel == *hint || rel.ends_with(&format!("/{hint}")))
        || name.contains("frontend")
        || name.contains("/web")
        || name.contains("/ui")
        || name.contains("extension")
        || name.contains("demo")
}

fn is_be_hint(folder: &str, name: &str, rel: &str) -> bool {
    const BES: &[&str] = &[
        "server",
        "api",
        "backend",
        "svc",
        "service",
        "apps/api",
        "apps/server",
        "apps/backend",
    ];
    BES.iter()
        .any(|hint| folder == *hint || rel == *hint || rel.ends_with(&format!("/{hint}")))
        || name.contains("backend")
        || name.contains("/server")
        || name.contains("/api")
}

fn role_rank(role: PartRole) -> u8 {
    match role {
        PartRole::Root => 0,
        PartRole::Fe => 1,
        PartRole::Be => 2,
        PartRole::Shared => 3,
        PartRole::Lib => 4,
    }
}

fn unify_kind(kinds: impl Iterator<Item = ProjectKind>) -> ProjectKind {
    let mut unique = Vec::new();
    for kind in kinds {
        if !unique.contains(&kind) {
            unique.push(kind);
        }
    }
    match unique.as_slice() {
        [kind] => *kind,
        [] => ProjectKind::Javascript,
        _ => ProjectKind::Mixed,
    }
}


fn attach_workspace_scripts(parts: &mut [ProjectPart], group_path: &str, group_name: &str) {
    let Some(root_idx) = parts.iter().position(|part| {
        part.role == PartRole::Root || paths_equal(&part.path, group_path)
    }) else {
        return;
    };
    let root_cwd = display_path(Path::new(group_path));
    let root_name = if !group_name.is_empty() {
        group_name.to_string()
    } else {
        parts[root_idx].name.clone()
    };
    let candidates: Vec<DetectedScript> = parts[root_idx].scripts.clone();
    let mut moving: Vec<(usize, DetectedScript)> = Vec::new();
    for script in candidates {
        let Some(target) = workspace_target(&script.command) else {
            continue;
        };
        let Some(idx) = find_part_for_workspace(parts, &target) else {
            continue;
        };
        if idx == root_idx {
            continue;
        }
        let mut moved = script.clone();
        moved.cwd = Some(root_cwd.clone());
        moved.source = ScriptSource::Workspace;
        moved.source_name = Some(root_name.clone());
        moved.label = Some(format!("root: {} → {}", script.name, script.command));
        moving.push((idx, moved));
    }
    if moving.is_empty() {
        return;
    }
    parts[root_idx].scripts.retain(|script| {
        !moving.iter().any(|(_, moved)| {
            moved.name == script.name && moved.command == script.command
        })
    });
    for (idx, script) in moving {
        if !parts[idx]
            .scripts
            .iter()
            .any(|existing| existing.name == script.name && existing.source == ScriptSource::Workspace)
        {
            parts[idx].scripts.push(script);
        }
    }
}

fn find_part_for_workspace(parts: &[ProjectPart], target: &str) -> Option<usize> {
    let target_norm = target.replace('\\', "/").trim_matches('/').to_ascii_lowercase();
    let mut best: Option<(usize, usize)> = None;
    for (idx, part) in parts.iter().enumerate() {
        let rel = part
            .relative_path
            .replace('\\', "/")
            .trim_start_matches('.')
            .trim_matches('/')
            .to_ascii_lowercase();
        if rel == target_norm || rel.ends_with(&format!("/{target_norm}")) {
            let score = rel.len();
            if best.map(|(_, current)| score > current).unwrap_or(true) {
                best = Some((idx, score));
            }
            continue;
        }
        if part.name.eq_ignore_ascii_case(target) {
            if best.is_none() {
                best = Some((idx, 1));
            }
            continue;
        }
        if let Some(folder) = Path::new(&part.path)
            .file_name()
            .and_then(|name| name.to_str())
        {
            if folder.eq_ignore_ascii_case(target) && best.is_none() {
                best = Some((idx, 1));
            }
        }
    }
    best.map(|(idx, _)| idx)
}

fn aggregate_build(members: &[Project]) -> BuildInfo {
    let mut best: Option<(&Project, i64)> = None;
    for project in members {
        if let Some(stamp) = project.build.last_built_at {
            match best {
                Some((_, current)) if current >= stamp => {}
                _ => best = Some((project, stamp)),
            }
        } else if project.build.built && best.is_none() {
            best = Some((project, 0));
        }
    }
    match best {
        Some((project, _)) => project.build.clone(),
        None => BuildInfo::missing(),
    }
}

fn is_js_like(kind: ProjectKind) -> bool {
    matches!(kind, ProjectKind::Javascript | ProjectKind::Mixed)
}

fn relative_to(path: &str, root: &str) -> String {
    let path_n = normalize_path_str(path);
    let root_n = normalize_path_str(root);
    let rest = if cfg!(windows) {
        if path_n.len() >= root_n.len() && path_n[..root_n.len()].eq_ignore_ascii_case(&root_n) {
            &path_n[root_n.len()..]
        } else {
            return path.replace('\\', "/");
        }
    } else if let Some(stripped) = path_n.strip_prefix(&root_n) {
        stripped
    } else {
        return path.to_string();
    };
    let trimmed = rest.trim_start_matches(['\\', '/']);
    if trimmed.is_empty() {
        ".".to_string()
    } else {
        trimmed.replace('\\', "/")
    }
}
