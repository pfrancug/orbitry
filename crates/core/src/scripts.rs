use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{
    default_kind_patterns, display_path, pattern_matches, ScriptKind, ScriptPattern, ScriptScope,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptSource {
    #[default]
    Package,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedScript {
    pub name: String,
    pub command: String,
    pub kind: ScriptKind,
    pub scope: ScriptScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub source: ScriptSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl DetectedScript {
    pub fn display_label(&self) -> String {
        if let Some(label) = &self.label {
            if !label.is_empty() {
                return label.clone();
            }
        }
        self.name.clone()
    }
}

const FE_DEPS: &[&str] = &[
    "vite",
    "next",
    "nuxt",
    "react",
    "react-dom",
    "vue",
    "svelte",
    "sveltekit",
    "@remix-run/react",
    "@sveltejs/kit",
    "@angular/core",
    "gatsby",
    "astro",
    "preact",
    "solid-js",
    "@vitejs/plugin-react",
];

const BE_DEPS: &[&str] = &[
    "hono",
    "express",
    "fastify",
    "koa",
    "@nestjs/core",
    "fastapi",
    "elysia",
    "oak",
    "polka",
    "restify",
    "socket.io",
];

const FE_NAME_HINTS: &[&str] = &["web", "ui", "frontend", "client", "site", "app-web"];
const BE_NAME_HINTS: &[&str] = &["api", "server", "backend", "svc", "service", "worker"];
const SHARED_NAME_HINTS: &[&str] = &["all", "full", "stack", "monorepo"];
const FE_FOLDERS: &[&str] = &[
    "web",
    "frontend",
    "ui",
    "client",
    "site",
    "app",
    "apps/web",
    "apps/frontend",
    "apps/ui",
];
const BE_FOLDERS: &[&str] = &[
    "server",
    "api",
    "backend",
    "svc",
    "service",
    "apps/api",
    "apps/server",
    "apps/backend",
];

pub fn detect_scripts(
    scripts: &[(String, String)],
    custom: &[ScriptPattern],
    package_dir: &Path,
    dep_names: &BTreeSet<String>,
) -> Vec<DetectedScript> {
    let mut detected = Vec::new();
    for (name, command) in scripts {
        let (kind, custom_scope) = match_script(name, custom).unwrap_or((ScriptKind::Other, None));
        let scope =
            custom_scope.unwrap_or_else(|| classify_scope(name, package_dir, dep_names));
        detected.push(DetectedScript {
            name: name.clone(),
            command: command.clone(),
            kind,
            scope,
            cwd: Some(display_path(package_dir)),
            source: ScriptSource::Package,
            source_name: None,
            label: None,
        });
    }
    detected
}

fn match_script(name: &str, custom: &[ScriptPattern]) -> Option<(ScriptKind, Option<ScriptScope>)> {
    for pattern in custom {
        if pattern_matches(&pattern.pattern, name) {
            return Some((pattern.kind, Some(pattern.scope)));
        }
    }
    for (kind, pattern) in default_kind_patterns() {
        if pattern_matches(pattern, name) {
            return Some((kind, None));
        }
    }
    None
}

pub fn classify_scope(
    script_name: &str,
    package_dir: &Path,
    dep_names: &BTreeSet<String>,
) -> ScriptScope {
    let lower = script_name.to_ascii_lowercase();
    let token_source = lower.replace(':', " ").replace(['_', '-', '/'], " ");
    let tokens: Vec<&str> = token_source.split_whitespace().collect();

    if tokens.iter().any(|token| SHARED_NAME_HINTS.contains(token)) {
        return ScriptScope::Shared;
    }
    if tokens.iter().any(|token| FE_NAME_HINTS.contains(token)) {
        return ScriptScope::Fe;
    }
    if tokens.iter().any(|token| BE_NAME_HINTS.contains(token)) {
        return ScriptScope::Be;
    }

    if let Some(folder) = folder_scope(package_dir) {
        return folder;
    }

    let has_fe = has_any_dep(dep_names, FE_DEPS);
    let has_be = has_any_dep(dep_names, BE_DEPS);
    match (has_fe, has_be) {
        (true, true) => ScriptScope::Shared,
        (true, false) => ScriptScope::Fe,
        (false, true) => ScriptScope::Be,
        (false, false) => ScriptScope::Shared,
    }
}

fn folder_scope(package_dir: &Path) -> Option<ScriptScope> {
    let name = package_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if FE_FOLDERS.iter().any(|folder| *folder == name) {
        return Some(ScriptScope::Fe);
    }
    if BE_FOLDERS.iter().any(|folder| *folder == name) {
        return Some(ScriptScope::Be);
    }

    let rendered = package_dir
        .iter()
        .rev()
        .take(3)
        .filter_map(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let suffix = rendered.iter().rev().cloned().collect::<Vec<_>>().join("/");
    if FE_FOLDERS.iter().any(|folder| suffix.ends_with(folder)) {
        return Some(ScriptScope::Fe);
    }
    if BE_FOLDERS.iter().any(|folder| suffix.ends_with(folder)) {
        return Some(ScriptScope::Be);
    }
    None
}

pub fn has_any_dep(dep_names: &BTreeSet<String>, candidates: &[&str]) -> bool {
    candidates.iter().any(|dep| dep_names.contains(*dep))
}

pub fn guess_js_framework(dep_names: &BTreeSet<String>) -> Option<String> {
    const ORDER: &[(&str, &str)] = &[
        ("next", "next"),
        ("nuxt", "nuxt"),
        ("@remix-run/react", "remix"),
        ("@sveltejs/kit", "sveltekit"),
        ("astro", "astro"),
        ("gatsby", "gatsby"),
        ("vite", "vite"),
        ("electron", "electron"),
        ("@tauri-apps/api", "tauri"),
        ("@nestjs/core", "nestjs"),
        ("hono", "hono"),
        ("fastify", "fastify"),
        ("express", "express"),
        ("@angular/core", "angular"),
        ("vue", "vue"),
        ("svelte", "svelte"),
        ("react", "react"),
        ("preact", "preact"),
        ("solid-js", "solid"),
    ];
    ORDER
        .iter()
        .find(|(dep, _)| dep_names.contains(*dep))
        .map(|(_, name)| (*name).to_string())
}


/// If `command` targets a workspace package (`-w`, `--workspace`, `--filter`,
/// or `yarn workspace`), return that package name or relative path.
pub fn workspace_target(command: &str) -> Option<String> {
    let tokens = split_command(command);
    if tokens.is_empty() {
        return None;
    }
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        if token == "-w" || token == "--workspace" || token == "--filter" {
            if let Some(value) = tokens.get(i + 1) {
                return Some(clean_workspace_name(value));
            }
        }
        if let Some(value) = token
            .strip_prefix("--workspace=")
            .or_else(|| token.strip_prefix("--filter="))
            .or_else(|| token.strip_prefix("-w="))
        {
            return Some(clean_workspace_name(value));
        }
        if token == "workspace" && i > 0 {
            let prev = tokens[i - 1].as_str();
            let prev_name = prev.rsplit(['/', '\\']).next().unwrap_or(prev).to_ascii_lowercase();
            if prev_name == "yarn" || prev_name == "yarn.cmd" || prev_name == "yarn.exe" {
                if let Some(value) = tokens.get(i + 1) {
                    return Some(clean_workspace_name(value));
                }
            }
        }
        i += 1;
    }
    None
}

fn clean_workspace_name(value: &str) -> String {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'');
    let trimmed = trimmed.strip_prefix("./").unwrap_or(trimmed);
    trimmed.replace('\\', "/").trim_matches('/').to_string()
}

fn split_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in command.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
