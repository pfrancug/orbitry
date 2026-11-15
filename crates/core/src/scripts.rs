use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{
    default_kind_patterns, pattern_matches, ScriptKind, ScriptPattern, ScriptScope,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedScript {
    pub name: String,
    pub command: String,
    pub kind: ScriptKind,
    pub scope: ScriptScope,
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
        if let Some((kind, custom_scope)) = match_script(name, custom) {
            let scope =
                custom_scope.unwrap_or_else(|| classify_scope(name, package_dir, dep_names));
            detected.push(DetectedScript {
                name: name.clone(),
                command: command.clone(),
                kind,
                scope,
            });
        }
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
