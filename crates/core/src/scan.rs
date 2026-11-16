use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifacts::{detect_build, has_node_modules, BuildInfo};
use crate::config::{canonical, display_path, Config};
use crate::git::{detect_git, GitInfo};
use crate::ports::{apply_port_overrides, detect_static_ports, PortInfo};
use crate::scripts::{detect_scripts, guess_js_framework, DetectedScript};

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "target",
    ".git",
    ".next",
    "out",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
    "coverage",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Javascript,
    Rust,
    Python,
    Go,
    Docker,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
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
    pub ports: Vec<PortInfo>,
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: Option<String>,
    version: Option<String>,
    scripts: Option<BTreeMap<String, String>>,
    workspaces: Option<Workspaces>,
    dependencies: Option<BTreeMap<String, Value>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<BTreeMap<String, Value>>,
    #[serde(rename = "peerDependencies")]
    peer_dependencies: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Workspaces {
    List(Vec<String>),
    Object { packages: Option<Vec<String>> },
}

impl Workspaces {
    fn patterns(&self) -> Vec<String> {
        match self {
            Workspaces::List(list) => list.clone(),
            Workspaces::Object { packages } => packages.clone().unwrap_or_default(),
        }
    }
}

pub fn scan(config: &Config) -> Vec<Project> {
    crate::git::clear_git_cache();
    let _bridge = crate::bridge::begin(config.config_bridge, &config.roots);
    let mut seen = HashSet::new();
    let mut projects = Vec::new();
    for root in &config.roots {
        let root = PathBuf::from(root);
        if !root.exists() {
            continue;
        }
        if is_project(&root) {
            collect_project(&root, None, config, &mut seen, &mut projects);
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && !should_skip(path))
            .collect();
        children.sort();
        for child in children {
            if is_project(&child) {
                collect_project(&child, None, config, &mut seen, &mut projects);
            }
        }
    }
    projects.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    for project in &mut projects {
        apply_port_overrides(&mut project.ports, &project.id, &config.port_overrides);
    }
    projects
}

fn collect_project(
    dir: &Path,
    parent: Option<String>,
    config: &Config,
    seen: &mut HashSet<String>,
    projects: &mut Vec<Project>,
) {
    let Some(project) = inspect_project(dir, parent.clone(), config) else {
        return;
    };
    if !seen.insert(project.id.clone()) {
        return;
    }
    let id = project.id.clone();
    let package_path = dir.join("package.json");
    projects.push(project);

    if package_path.exists() {
        let members = workspace_members(dir, &package_path);
        for member in members {
            if member == dir {
                continue;
            }
            if is_project(&member) {
                collect_project(&member, Some(id.clone()), config, seen, projects);
            }
        }
    }
}

fn inspect_project(dir: &Path, parent: Option<String>, config: &Config) -> Option<Project> {
    let path = display_path(&canonical(dir));
    let package_path = dir.join("package.json");
    let has_js = package_path.exists();
    let has_rust = dir.join("Cargo.toml").exists();
    let has_python = dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists();
    let has_go = dir.join("go.mod").exists();
    let has_docker = dir.join("docker-compose.yml").exists()
        || dir.join("docker-compose.yaml").exists()
        || dir.join("compose.yml").exists()
        || dir.join("compose.yaml").exists();

    if !has_js && !has_rust && !has_python && !has_go && !has_docker {
        return None;
    }

    let mut name = folder_name(dir);
    let mut version = None;
    let mut scripts = Vec::new();
    let mut framework = None;
    let mut kind = match (has_js, has_rust, has_python, has_go, has_docker) {
        (true, false, false, false, _) => ProjectKind::Javascript,
        (false, true, false, false, _) => ProjectKind::Rust,
        (false, false, true, false, _) => ProjectKind::Python,
        (false, false, false, true, _) => ProjectKind::Go,
        (false, false, false, false, true) => ProjectKind::Docker,
        _ => ProjectKind::Mixed,
    };

    if has_js {
        if let Some(pkg) = read_package(&package_path) {
            if let Some(pkg_name) = pkg.name.filter(|s| !s.is_empty()) {
                name = pkg_name;
            }
            version = pkg.version;
            let mut dep_names = BTreeSet::new();
            for source in [
                pkg.dependencies.as_ref(),
                pkg.dev_dependencies.as_ref(),
                pkg.peer_dependencies.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                dep_names.extend(source.keys().cloned());
            }
            let script_pairs: Vec<(String, String)> =
                pkg.scripts.unwrap_or_default().into_iter().collect();
            scripts = detect_scripts(&script_pairs, &config.script_patterns, dir, &dep_names);
            framework = guess_js_framework(&dep_names);
        }
    }

    if framework.is_none() {
        if has_rust {
            if let Some((cargo_name, cargo_version)) = read_cargo_package(dir) {
                if !has_js {
                    name = cargo_name;
                    version = cargo_version.or(version);
                }
                framework = Some("rust".to_string());
            } else if !has_js {
                framework = Some("rust".to_string());
            }
        } else if has_python {
            if let Some((py_name, py_version)) = read_pyproject(dir) {
                name = py_name;
                version = py_version.or(version);
            }
            framework = Some("python".to_string());
        } else if has_go {
            if let Some(module) = read_go_module(dir) {
                name = module.rsplit('/').next().unwrap_or(&module).to_string();
            }
            framework = Some("go".to_string());
        } else if has_docker {
            framework = Some("docker".to_string());
        }
    } else if has_rust || has_python || has_go {
        kind = ProjectKind::Mixed;
    }

    let ports = detect_static_ports(dir, &scripts, None, framework.as_deref());
    Some(Project {
        id: path.clone(),
        path,
        name,
        version,
        kind,
        framework,
        scripts,
        git: detect_git(dir),
        build: detect_build(dir),
        node_modules: has_js && has_node_modules(dir),
        parent,
        ports,
    })
}

fn is_project(dir: &Path) -> bool {
    dir.join("package.json").exists()
        || dir.join("Cargo.toml").exists()
        || dir.join("pyproject.toml").exists()
        || dir.join("requirements.txt").exists()
        || dir.join("go.mod").exists()
        || dir.join("docker-compose.yml").exists()
        || dir.join("docker-compose.yaml").exists()
        || dir.join("compose.yml").exists()
        || dir.join("compose.yaml").exists()
}

fn should_skip(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };
    name.starts_with('.') || SKIP_DIRS.iter().any(|skip| *skip == name)
}

fn folder_name(dir: &Path) -> String {
    dir.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string()
}

fn read_package(path: &Path) -> Option<PackageJson> {
    let data = fs::read(path).ok()?;
    let data = crate::config::strip_bom(&data);
    serde_json::from_slice(data).ok()
}

fn workspace_members(dir: &Path, package_path: &Path) -> Vec<PathBuf> {
    let mut patterns = Vec::new();
    if let Some(pkg) = read_package(package_path) {
        if let Some(workspaces) = pkg.workspaces {
            patterns.extend(workspaces.patterns());
        }
    }
    if dir.join("apps").is_dir() && !patterns.iter().any(|p| p == "apps/*" || p == "apps/*/") {
        patterns.push("apps/*".to_string());
    }
    let mut members = Vec::new();
    for pattern in patterns {
        members.extend(expand_glob(dir, &pattern));
    }
    members.sort();
    members.dedup();
    members
}

fn expand_glob(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let pattern = pattern.replace('\\', "/").trim_end_matches('/').to_string();
    if pattern.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = pattern.split('/').collect();
    let mut current = vec![root.to_path_buf()];
    for part in parts {
        let mut next = Vec::new();
        for dir in current {
            if part == "*" {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && !should_skip(&path) {
                            next.push(path);
                        }
                    }
                }
            } else if part == "**" {
                // Keep it simple: treat as a single-level star, plus the directory itself.
                next.push(dir.clone());
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && !should_skip(&path) {
                            next.push(path);
                        }
                    }
                }
            } else {
                let child = dir.join(part);
                if child.exists() {
                    next.push(child);
                }
            }
        }
        current = next;
    }
    current
}

fn read_cargo_package(dir: &Path) -> Option<(String, Option<String>)> {
    let text = fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let value: toml::Value = text.parse().ok()?;
    // Workspace roots may not have [package]; prefer [package], else folder name.
    if let Some(package) = value.get("package") {
        let name = package.get("name")?.as_str()?.to_string();
        let version = package
            .get("version")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        return Some((name, version));
    }
    if value.get("workspace").is_some() {
        return Some((folder_name(dir), None));
    }
    None
}

fn read_pyproject(dir: &Path) -> Option<(String, Option<String>)> {
    let text = fs::read_to_string(dir.join("pyproject.toml")).ok()?;
    let value: toml::Value = text.parse().ok()?;
    if let Some(project) = value.get("project") {
        let name = project
            .get("name")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| folder_name(dir));
        let version = project
            .get("version")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        return Some((name, version));
    }
    if let Some(poetry) = value.get("tool").and_then(|t| t.get("poetry")) {
        let name = poetry
            .get("name")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| folder_name(dir));
        let version = poetry
            .get("version")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        return Some((name, version));
    }
    None
}

fn read_go_module(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join("go.mod")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(module) = line.strip_prefix("module ") {
            return Some(module.trim().to_string());
        }
    }
    None
}
