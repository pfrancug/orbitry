//! Static and live port detection for project parts and running processes.
//!
//! An optional Node config-bridge plugin can later resolve code-based configs
//! exactly. When that plugin is missing (the default today), this module falls
//! back to environment files, CLI flags, regex on config sources, framework
//! defaults, log lines, and the live PID tree.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::config::{canonical, paths_equal, PortOverride};
use crate::ports_resolve::resolve_script_targets;
use crate::scripts::DetectedScript;
use crate::util::hide_console;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortSource {
    Default,
    Config,
    Env,
    Flag,
    Bridge,
    Log,
    Process,
}

impl PortSource {
    fn rank(self) -> u8 {
        match self {
            PortSource::Default => 0,
            PortSource::Config => 1,
            PortSource::Env => 2,
            PortSource::Flag => 3,
            PortSource::Bridge => 4,
            PortSource::Log => 5,
            PortSource::Process => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub port: u16,
    pub source: PortSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binds: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_url: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignored: bool,
}

impl PortInfo {
    pub fn new(port: u16, source: PortSource) -> Option<Self> {
        Self::named(port, source, None)
    }

    pub fn named(port: u16, source: PortSource, name: Option<String>) -> Option<Self> {
        if is_plausible_port(port) {
            Some(Self {
                port,
                source,
                name,
                binds: Vec::new(),
                pid: None,
                process: None,
                url: Some(localhost_url(port, None)),
                health_url: None,
                ignored: false,
            })
        } else {
            None
        }
    }

    pub fn is_explicit(&self) -> bool {
        matches!(
            self.source,
            PortSource::Config | PortSource::Env | PortSource::Flag | PortSource::Bridge
        )
    }

    pub fn is_live(&self) -> bool {
        self.source == PortSource::Process
    }
}

/// A TCP listener observed on the machine, including bind address and owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub address: String,
    pub pid: u32,
    pub process: Option<String>,
}

/// Snapshot of every listening TCP port on this machine.
#[derive(Debug, Clone, Default)]
pub struct ListeningSnapshot {
    pub listeners: Vec<Listener>,
}

impl ListeningSnapshot {
    pub fn collect() -> Self {
        Self {
            listeners: collect_listeners(),
        }
    }

    pub fn ports_for(&self, root_pid: u32) -> Vec<PortInfo> {
        let tree: BTreeSet<u32> = process_tree(root_pid).into_iter().collect();
        if tree.is_empty() {
            return Vec::new();
        }
        let mut by_port: BTreeMap<u16, PortInfo> = BTreeMap::new();
        for listener in &self.listeners {
            if !tree.contains(&listener.pid) || !is_plausible_port(listener.port) {
                continue;
            }
            let entry = by_port.entry(listener.port).or_insert_with(|| PortInfo {
                port: listener.port,
                source: PortSource::Process,
                name: None,
                binds: Vec::new(),
                pid: Some(listener.pid),
                process: listener.process.clone(),
                url: Some(localhost_url(listener.port, None)),
                health_url: None,
                ignored: false,
            });
            if !entry.binds.iter().any(|bind| bind == &listener.address) {
                entry.binds.push(listener.address.clone());
            }
            if entry.pid.is_none() {
                entry.pid = Some(listener.pid);
                entry.process = listener.process.clone();
            }
        }
        by_port.into_values().collect()
    }
}

pub fn localhost_url(port: u16, path: Option<&str>) -> String {
    match path {
        Some(value) if !value.is_empty() && value != "/" => {
            format!("http://localhost:{port}{value}")
        }
        _ => format!("http://localhost:{port}"),
    }
}

pub fn normalize_bind(address: &str) -> String {
    let trimmed = address.trim().trim_matches(|c| c == '[' || c == ']');
    if trimmed.is_empty() || trimmed == "*" || trimmed == "0.0.0.0" {
        return "0.0.0.0".into();
    }
    if trimmed == "::" || trimmed == "::0" || trimmed == "::ffff:0.0.0.0" {
        return "::".into();
    }
    trimmed.to_string()
}

pub fn is_wildcard_bind(address: &str) -> bool {
    matches!(normalize_bind(address).as_str(), "0.0.0.0" | "::")
}

pub fn is_loopback_bind(address: &str) -> bool {
    matches!(normalize_bind(address).as_str(), "127.0.0.1" | "::1")
}

pub fn is_ipv6_only(binds: &[String]) -> bool {
    !binds.is_empty() && binds.iter().all(|bind| normalize_bind(bind) == "::1")
}

pub fn is_all_interfaces(binds: &[String]) -> bool {
    binds.iter().any(|bind| is_wildcard_bind(bind))
}

pub fn is_lan_only(binds: &[String]) -> bool {
    !binds.is_empty()
        && !binds
            .iter()
            .any(|bind| is_wildcard_bind(bind) || is_loopback_bind(bind))
}

/// Hosts to probe for a live port, in preference order.
pub fn probe_hosts(binds: &[String]) -> Vec<String> {
    let mut hosts = Vec::new();
    let mut push = |host: &str| {
        if !hosts.iter().any(|existing| existing == host) {
            hosts.push(host.to_string());
        }
    };
    if binds.is_empty() {
        push("127.0.0.1");
        push("::1");
        return hosts;
    }
    let has_v4 = binds.iter().any(|bind| {
        let normalized = normalize_bind(bind);
        normalized == "127.0.0.1" || normalized == "0.0.0.0"
    });
    let has_v6 = binds.iter().any(|bind| {
        let normalized = normalize_bind(bind);
        normalized == "::1" || normalized == "::"
    });
    if has_v4 {
        push("127.0.0.1");
    }
    if has_v6 {
        push("::1");
    }
    for bind in binds {
        let normalized = normalize_bind(bind);
        if !matches!(normalized.as_str(), "0.0.0.0" | "::" | "127.0.0.1" | "::1") {
            push(&normalized);
        }
    }
    drop(push);
    if hosts.is_empty() {
        hosts.push("127.0.0.1".into());
        hosts.push("::1".into());
    }
    hosts
}

pub fn framework_label(framework: &str) -> String {
    match framework.to_ascii_lowercase().as_str() {
        "vite" | "vitest" => "Vite".into(),
        "svelte" => "Svelte".into(),
        "sveltekit" => "SvelteKit".into(),
        "next" | "next.js" => "Next.js".into(),
        "nuxt" => "Nuxt".into(),
        "remix" => "Remix".into(),
        "express" => "Express".into(),
        "hono" => "Hono".into(),
        "fastify" => "Fastify".into(),
        "nest" | "nestjs" => "Nest".into(),
        "koa" => "Koa".into(),
        "react" | "cra" => "React".into(),
        "webpack" => "webpack".into(),
        "angular" => "Angular".into(),
        "astro" => "Astro".into(),
        "django" => "Django".into(),
        "flask" => "Flask".into(),
        "fastapi" => "FastAPI".into(),
        "uvicorn" => "Uvicorn".into(),
        "python" => "Python".into(),
        "go" | "gin" => "Go".into(),
        "axum" | "actix" | "rocket" | "warp" | "rust" => "Rust".into(),
        other => {
            let mut label = other.to_string();
            if let Some(first) = label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}


/// Merge `incoming` into `ports`, keeping the more specific source for a port.
pub fn merge_ports(ports: &mut Vec<PortInfo>, incoming: impl IntoIterator<Item = PortInfo>) {
    for port in incoming {
        if let Some(existing) = ports.iter_mut().find(|item| item.port == port.port) {
            if port.source.rank() > existing.source.rank() {
                existing.source = port.source;
            }
            if existing.name.is_none() {
                existing.name = port.name;
            } else if port.name.is_some() && port.source.rank() >= existing.source.rank() {
                existing.name = port.name;
            }
            for bind in port.binds {
                if !existing.binds.iter().any(|item| item == &bind) {
                    existing.binds.push(bind);
                }
            }
            if port.pid.is_some()
                && (existing.pid.is_none() || port.source.rank() >= existing.source.rank())
            {
                existing.pid = port.pid;
                existing.process = port.process;
            }
            if existing.url.is_none() {
                existing.url = port.url;
            } else if port.url.is_some() && port.source.rank() >= existing.source.rank() {
                existing.url = port.url;
            }
            if port.health_url.is_some() {
                existing.health_url = port.health_url;
            }
            existing.ignored = existing.ignored || port.ignored;
        } else {
            ports.push(port);
        }
    }
    ports.sort_by_key(|item| item.port);
}



/// Apply user port overrides from Orbitry config: custom names and health URLs.
/// The `ignore` field is still accepted in config for compatibility, but it is no longer applied.
pub fn apply_port_overrides(ports: &mut Vec<PortInfo>, part_id: &str, overrides: &[PortOverride]) {
    let relevant: Vec<&PortOverride> = overrides
        .iter()
        .filter(|item| item.matches_part(part_id))
        .collect();
    if relevant.is_empty() {
        return;
    }
    for port in ports.iter_mut() {
        let Some(item) = relevant.iter().find(|item| item.port == port.port) else {
            continue;
        };
        if let Some(name) = item.name.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            port.name = Some(name.to_string());
        }
        if let Some(url) = item
            .health_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            port.health_url = Some(url.to_string());
        }
        let _ = item.ignore;
    }
}

pub fn detect_static_ports(
    dir: &Path,
    scripts: &[DetectedScript],
    script_filter: Option<&str>,
    framework: Option<&str>,
) -> Vec<PortInfo> {
    detect_ports(dir, scripts, script_filter, framework, false)
}

/// Ports a script is expected to bring up, including children started by a
/// composite orchestrator (concurrently, npm-run-all, turbo, `npm -w`).
pub fn detect_expected_ports(
    dir: &Path,
    scripts: &[DetectedScript],
    script_filter: Option<&str>,
    framework: Option<&str>,
) -> Vec<PortInfo> {
    detect_ports(dir, scripts, script_filter, framework, true)
}

fn detect_ports(
    dir: &Path,
    scripts: &[DetectedScript],
    script_filter: Option<&str>,
    framework: Option<&str>,
    include_composite_children: bool,
) -> Vec<PortInfo> {
    let filters: Vec<Option<String>> = if let Some(name) = script_filter {
        vec![Some(name.to_string())]
    } else if !scripts.is_empty() {
        scripts.iter().map(|script| Some(script.name.clone())).collect()
    } else {
        vec![None]
    };

    let own_serves = package_can_bind(dir, scripts, script_filter);
    let mut ports = Vec::new();
    for filter in filters {
        let targets = resolve_script_targets(dir, filter.as_deref());
        for target in targets {
            let foreign = !is_same_dir(&target.dir, dir);
            if foreign && !include_composite_children {
                continue;
            }
            if !foreign && !own_serves {
                continue;
            }
            if foreign && !script_starts_server(&target.dir, target.script.as_deref()) {
                continue;
            }
            let mut found = Vec::new();
            let script_name = target.script.as_deref().or(filter.as_deref());
            collect_local_ports(&target.dir, script_name, &mut found);
            if found.is_empty() {
                if script_starts_server(&target.dir, script_name) {
                    let inferred = infer_framework(&target.dir);
                    if let Some(port) = inferred.as_deref().and_then(default_port_for) {
                        merge_ports(&mut found, PortInfo::named(port, PortSource::Default, target.name.clone()));
                    }
                }
            } else if let Some(name) = &target.name {
                for item in &mut found {
                    if item.name.is_none() {
                        item.name = Some(name.clone());
                    }
                }
            }
            merge_ports(&mut ports, found);
        }
    }

    if ports.is_empty() && own_serves && !include_composite_children {
        let allow_default = match script_filter {
            Some(name) => script_starts_server(dir, Some(name)),
            None => scripts.is_empty()
                || scripts.iter().any(|script| script_starts_server(dir, Some(&script.name))),
        };
        if allow_default {
            let inferred = framework
                .map(str::to_string)
                .or_else(|| infer_framework(dir));
            if let Some(port) = inferred.as_deref().and_then(default_port_for) {
                merge_ports(&mut ports, PortInfo::new(port, PortSource::Default));
            }
        }
    }

    if own_serves || include_composite_children {
        merge_ports(&mut ports, crate::bridge::ports_for(dir, scripts));
    }
    ports
}

fn is_same_dir(a: &Path, b: &Path) -> bool {
    let a = canonical(a);
    let b = canonical(b);
    paths_equal(&a.to_string_lossy(), &b.to_string_lossy())
}

fn package_can_bind(dir: &Path, scripts: &[DetectedScript], script_filter: Option<&str>) -> bool {
    if !dir.join("package.json").exists() {
        return true;
    }
    if let Some(name) = script_filter {
        return script_starts_server(dir, Some(name));
    }
    if !scripts.is_empty() {
        return scripts
            .iter()
            .any(|script| script_starts_server(dir, Some(&script.name)));
    }
    match package_script_commands(dir, None) {
        Some(commands) => commands.iter().any(|command| command_starts_server(command)),
        None => false,
    }
}

fn collect_local_ports(dir: &Path, script_filter: Option<&str>, ports: &mut Vec<PortInfo>) {
    collect_env_ports(dir, ports);
    collect_config_ports(dir, ports, script_filter);
    collect_flag_ports(dir, &[], script_filter, ports);
    collect_source_ports(dir, ports);
}

pub fn ports_from_log(line: &str) -> Vec<PortInfo> {
    let name = concurrent_prefix(line);
    let mut found = Vec::new();
    for port in ports_from_host_urls(line) {
        merge_ports(&mut found, PortInfo::named(port, PortSource::Log, name.clone()));
    }
    if found.is_empty() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("listening")
            || lower.contains("local:")
            || lower.contains("ready in")
            || lower.contains("localhost")
            || lower.contains("--port")
        {
            for port in extract_ports(&lower, false) {
                merge_ports(&mut found, PortInfo::named(port, PortSource::Log, name.clone()));
            }
        }
    }
    found
}

fn concurrent_prefix(line: &str) -> Option<String> {
    let stripped: String = strip_ansi(line);
    let trimmed = stripped.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed.find(']')?;
    let name = trimmed[1..end].trim();
    if name.is_empty() || name.len() > 40 {
        return None;
    }
    Some(name.to_string())
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() {
                let ch = bytes[i];
                i += 1;
                if ch.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}


/// Listening TCP ports keyed by PID, then filtered to a process tree.
pub fn listening_ports_for(root_pid: u32) -> Vec<PortInfo> {
    ListeningSnapshot::collect().ports_for(root_pid)
}

fn collect_env_ports(dir: &Path, ports: &mut Vec<PortInfo>) {
    const NAMES: &[&str] = &[
        ".env",
        ".env.local",
        ".env.development",
        ".env.development.local",
        ".env.dev",
    ];
    // Only this package's env files. A repo-root `.env` must not be treated as
    // a bind port for every workspace app (libraries, frontends, extensions).
    for name in NAMES {
        let path = dir.join(name);
        if let Ok(text) = fs::read_to_string(&path) {
            for port in ports_from_env(&text) {
                merge_ports(ports, PortInfo::new(port, PortSource::Env));
            }
        }
    }
}

fn collect_config_ports(dir: &Path, ports: &mut Vec<PortInfo>, script_filter: Option<&str>) {
    const NAMES: &[&str] = &[
        "vite.config.ts",
        "vite.config.js",
        "vite.config.mjs",
        "vite.config.mts",
        "svelte.config.js",
        "svelte.config.ts",
        "next.config.js",
        "next.config.mjs",
        "next.config.ts",
        "astro.config.mjs",
        "astro.config.ts",
        "astro.config.js",
        "nuxt.config.ts",
        "nuxt.config.js",
        "webpack.config.js",
        "webpack.config.ts",
        "remix.config.js",
        "angular.json",
    ];
    for name in NAMES {
        let path = dir.join(name);
        if let Ok(raw) = fs::read_to_string(&path) {
            let text = if is_dev_script(script_filter) {
                strip_preview_block(&raw)
            } else {
                raw
            };
            for port in extract_ports(&text, true) {
                merge_ports(ports, PortInfo::new(port, PortSource::Config));
            }
        }
    }
}

fn collect_flag_ports(
    dir: &Path,
    scripts: &[DetectedScript],
    script_filter: Option<&str>,
    ports: &mut Vec<PortInfo>,
) {
    let mut commands: Vec<String> = scripts
        .iter()
        .filter(|script| script_filter.map(|name| script.name == name).unwrap_or(true))
        .map(|script| script.command.clone())
        .collect();

    if commands.is_empty() {
        if let Some(from_pkg) = package_script_commands(dir, script_filter) {
            commands = from_pkg;
        }
    }

    for command in commands {
        for port in ports_from_flags(&command) {
            merge_ports(ports, PortInfo::new(port, PortSource::Flag));
        }
    }
}


fn script_starts_server(dir: &Path, script: Option<&str>) -> bool {
    let Some(commands) = package_script_commands(dir, script) else {
        return true;
    };
    commands.iter().any(|command| command_starts_server(command))
}

fn command_starts_server(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    let has_vite_build = lower.contains("vite build");
    let has_dev_vite = lower.contains("vite") && !has_vite_build;
    let has_server = has_dev_vite
        || lower.contains("vite dev")
        || lower.contains("vite preview")
        || lower.contains("vite serve")
        || lower.contains("next dev")
        || lower.contains("nuxt dev")
        || lower.contains("nuxt start")
        || lower.contains("astro dev")
        || lower.contains("remix dev")
        || lower.contains("webpack serve")
        || lower.contains("webpack-dev-server")
        || lower.contains("tsx ")
        || lower.contains("ts-node")
        || lower.contains("nodemon")
        || lower.contains("cargo run")
        || lower.contains("cargo watch")
        || lower.contains("uvicorn")
        || lower.contains("dotnet run")
        || lower.contains("go run")
        || (lower.contains("preview") && !has_vite_build)
        || (lower.contains(" serve") && !has_vite_build);
    if has_server {
        return true;
    }
    if has_vite_build || lower.contains("tsc") || lower.contains("rimraf") {
        return false;
    }
    true
}

fn package_script_commands(dir: &Path, script_filter: Option<&str>) -> Option<Vec<String>> {
    let text = fs::read_to_string(dir.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let scripts = value.get("scripts")?.as_object()?;
    let commands = scripts
        .iter()
        .filter(|(name, _)| script_filter.map(|filter| filter == name.as_str()).unwrap_or(true))
        .filter_map(|(_, command)| command.as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    if commands.is_empty() {
        None
    } else {
        Some(commands)
    }
}

fn infer_framework(dir: &Path) -> Option<String> {
    const MARKERS: &[(&str, &str)] = &[
        ("vite.config.ts", "vite"),
        ("vite.config.js", "vite"),
        ("vite.config.mjs", "vite"),
        ("svelte.config.js", "sveltekit"),
        ("next.config.js", "next"),
        ("next.config.mjs", "next"),
        ("next.config.ts", "next"),
        ("astro.config.mjs", "astro"),
        ("astro.config.ts", "astro"),
        ("nuxt.config.ts", "nuxt"),
        ("nuxt.config.js", "nuxt"),
        ("angular.json", "angular"),
        ("manage.py", "django"),
    ];
    for (name, framework) in MARKERS {
        if dir.join(name).exists() {
            return Some((*framework).to_string());
        }
    }
    let pkg = fs::read_to_string(dir.join("package.json")).ok()?;
    let lower = pkg.to_ascii_lowercase();
    for (needle, framework) in [
        ("\"vite\"", "vite"),
        ("\"next\"", "next"),
        ("\"nuxt\"", "nuxt"),
        ("\"astro\"", "astro"),
        ("\"@sveltejs/kit\"", "sveltekit"),
        ("\"express\"", "express"),
        ("\"hono\"", "hono"),
        ("\"fastify\"", "fastify"),
        ("\"@nestjs/core\"", "nest"),
    ] {
        if lower.contains(needle) {
            return Some(framework.to_string());
        }
    }
    if dir.join("Cargo.toml").exists() {
        return Some("rust".to_string());
    }
    if dir.join("pyproject.toml").exists() || dir.join("manage.py").exists() {
        return Some("python".to_string());
    }
    if dir.join("go.mod").exists() {
        return Some("go".to_string());
    }
    None
}

pub fn default_port_for(framework: &str) -> Option<u16> {
    let name = framework.to_ascii_lowercase();
    let port = match name.as_str() {
        "vite" | "svelte" | "sveltekit" | "vitest" => 5173,
        "next" | "next.js" | "nuxt" | "remix" | "express" | "hono" | "fastify" | "nest"
        | "nestjs" | "koa" | "react" | "cra" | "webpack" | "angular" => 3000,
        "astro" => 4321,
        "django" | "flask" | "fastapi" | "uvicorn" | "python" => 8000,
        "go" | "gin" | "axum" | "actix" | "rocket" | "warp" | "rust" => 8080,
        _ => return None,
    };
    Some(port)
}

fn ports_from_env(text: &str) -> Vec<u16> {
    const KEYS: &[&str] = &[
        "port",
        "server_port",
        "app_port",
        "vite_port",
        "vite_dev_server_port",
        "host_port",
        "http_port",
        "api_port",
        "backend_port",
        "frontend_port",
        "nuxt_port",
        "astro_port",
    ];
    let mut ports = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_start_matches("export ").trim();
        if KEYS
            .iter()
            .any(|candidate| key.eq_ignore_ascii_case(candidate))
        {
            if let Some(port) = parse_port_token(value.trim().trim_matches(['\'', '"'])) {
                ports.push(port);
            }
        }
    }
    ports
}

fn ports_from_flags(command: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    let tokens: Vec<&str> = command.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        let lower = token.to_ascii_lowercase();
        if let Some(rest) = lower
            .strip_prefix("--port=")
            .or_else(|| lower.strip_prefix("--listen-port="))
            .or_else(|| lower.strip_prefix("-p="))
        {
            if let Some(port) = parse_port_token(rest) {
                ports.push(port);
            }
        } else if matches!(lower.as_str(), "--port" | "--listen-port" | "-p" | "--listen") {
            if let Some(next) = tokens.get(i + 1) {
                if let Some(port) = parse_port_token(next) {
                    ports.push(port);
                    i += 1;
                }
            }
        }
        i += 1;
    }
    ports
}

fn ports_from_host_urls(text: &str) -> Vec<u16> {
    const PREFIXES: &[&str] = &[
        "localhost:",
        "127.0.0.1:",
        "0.0.0.0:",
        "[::]:",
        "[::1]:",
        "https://localhost:",
        "http://localhost:",
        "https://127.0.0.1:",
        "http://127.0.0.1:",
        "https://0.0.0.0:",
        "http://0.0.0.0:",
    ];
    let lower = text.to_ascii_lowercase();
    let mut ports = Vec::new();
    for prefix in PREFIXES {
        let mut start = 0;
        while let Some(at) = lower[start..].find(prefix) {
            let abs = start + at + prefix.len();
            if let Some(port) = parse_port_at(&lower, abs) {
                ports.push(port);
            }
            start = abs;
        }
    }
    ports
}

fn extract_ports(text: &str, config_like: bool) -> Vec<u16> {
    let lower = text.to_ascii_lowercase();
    let mut ports = Vec::new();
    let mut search = 0;
    while let Some(at) = lower[search..].find("port") {
        let abs = search + at;
        let after = abs + 4;
        let rest = lower[after..].trim_start_matches(|c: char| {
            c.is_ascii_whitespace() || matches!(c, ':' | '=' | ',' | '(')
        });
        // Skip identifiers like portable, portion, portrait.
        let prev = if abs == 0 {
            ' '
        } else {
            lower.as_bytes()[abs - 1] as char
        };
        let next = lower.as_bytes().get(after).copied().unwrap_or(b' ') as char;
        if prev.is_ascii_alphanumeric() || next.is_ascii_alphanumeric() {
            search = after;
            continue;
        }
        // Only accept a number that immediately follows `port` / `port:`, not a
        // later digit sequence in the same file (proxy URLs, other settings).
        let immediate = rest.trim_start_matches(['\'', '"', '`']);
        if immediate.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if let Some(port) = parse_port_token(immediate.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("")) {
                if !config_like || is_app_port(port) {
                    ports.push(port);
                }
            }
        }
        search = after;
    }
    ports
}

fn parse_port_at(text: &str, start: usize) -> Option<u16> {
    let digits: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    parse_port_token(&digits)
}

fn parse_port_token(raw: &str) -> Option<u16> {
    let trimmed = raw.trim().trim_end_matches(|c: char| !c.is_ascii_digit());
    let digits: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    let port: u16 = digits.parse().ok()?;
    if is_plausible_port(port) {
        Some(port)
    } else {
        None
    }
}

fn is_plausible_port(port: u16) -> bool {
    port != 0
}

fn is_app_port(port: u16) -> bool {
    matches!(port, 80 | 443) || (1024..=65535).contains(&port)
}

fn is_dev_script(script: Option<&str>) -> bool {
    script.is_some_and(|name| {
        let lower = name.to_ascii_lowercase();
        lower == "dev" || lower.starts_with("dev:") || lower.starts_with("dev-")
    })
}

fn strip_preview_block(text: &str) -> String {
    let Some(start) = find_ident(text, "preview") else {
        return text.to_string();
    };
    let rest = &text[start + 7..];
    let Some(brace_rel) = rest.find('{') else {
        return text.to_string();
    };
    if !rest[..brace_rel].chars().all(|c| c.is_whitespace() || c == ':') {
        return text.to_string();
    }
    let abs = start + 7 + brace_rel;
    let Some(end) = matching_brace(text, abs) else {
        return text.to_string();
    };
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end + 1..]);
    out
}

fn find_ident(text: &str, ident: &str) -> Option<usize> {
    let mut search = 0;
    while let Some(at) = text[search..].find(ident) {
        let abs = search + at;
        let before = if abs == 0 { ' ' } else { text.as_bytes()[abs - 1] as char };
        let after = text.as_bytes().get(abs + ident.len()).copied().unwrap_or(b' ') as char;
        if !before.is_ascii_alphanumeric() && before != '_' && !after.is_ascii_alphanumeric() && after != '_' {
            return Some(abs);
        }
        search = abs + ident.len();
    }
    None
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'{') {
        return None;
    }
    let mut depth = 0;
    for (i, ch) in bytes.iter().enumerate().skip(open) {
        match ch {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_source_ports(dir: &Path, ports: &mut Vec<PortInfo>) {
    const FILES: &[&str] = &[
        "src/index.ts",
        "src/index.js",
        "src/server.ts",
        "src/server.js",
        "src/main.ts",
        "src/main.js",
        "src/app.ts",
        "src/constants.ts",
        "index.ts",
        "server.ts",
    ];
    let mut sources = Vec::new();
    for name in FILES {
        if let Ok(text) = fs::read_to_string(dir.join(name)) {
            sources.push(text);
        }
    }
    let combined = sources.join("
");
    for port in ports_from_port_assignments(&combined) {
        merge_ports(ports, PortInfo::new(port, PortSource::Config));
    }

    // Resolve imported PORT constants from a workspace "shared" package.
    if combined.contains("PORT") {
        for ancestor in dir.ancestors().take(4) {
            for child in ["apps/shared", "packages/shared", "shared"] {
                let shared = ancestor.join(child);
                if !shared.join("package.json").exists() {
                    continue;
                }
                for name in FILES {
                    if let Ok(text) = fs::read_to_string(shared.join(name)) {
                        for port in ports_from_port_assignments(&text) {
                            merge_ports(ports, PortInfo::new(port, PortSource::Config));
                        }
                    }
                }
            }
        }
    }
}

fn ports_from_port_assignments(text: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut search = 0;
    while let Some(at) = lower[search..].find("port") {
        let abs = search + at;
        let start = text[..abs]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let ident = &lower[start..abs + 4];
        let after = lower[abs + 4..].trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
        let after = after.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '='));
        if ident == "port" || ident.ends_with("_port") {
            if let Some(port) = parse_port_token(after.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("")) {
                ports.push(port);
            }
        }
        search = abs + 4;
    }
    ports
}


fn process_tree(root: u32) -> Vec<u32> {
    #[cfg(windows)]
    {
        return windows_process_tree(root);
    }
    #[cfg(not(windows))]
    {
        unix_process_tree(root)
    }
}

#[cfg(windows)]
fn windows_process_tree(root: u32) -> Vec<u32> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pairs: Vec<(u32, u32)> = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap == INVALID_HANDLE_VALUE {
            return vec![root];
        }
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    expand_tree(root, &pairs)
}

#[cfg(not(windows))]
fn unix_process_tree(root: u32) -> Vec<u32> {
    let mut cmd = Command::new("ps");
    cmd.args(["-axo", "pid=,ppid="]);
    hide_console(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let Ok(output) = cmd.output() else {
        return vec![root];
    };
    if !output.status.success() {
        return vec![root];
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut pairs = Vec::new();
    for line in text.lines() {
        let mut bits = line.split_whitespace();
        let Some(pid) = bits.next().and_then(|v| v.parse().ok()) else {
            continue;
        };
        let Some(ppid) = bits.next().and_then(|v| v.parse().ok()) else {
            continue;
        };
        pairs.push((pid, ppid));
    }
    expand_tree(root, &pairs)
}

fn expand_tree(root: u32, pairs: &[(u32, u32)]) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, ppid) in pairs {
        children.entry(*ppid).or_default().push(*pid);
    }
    let mut pids = vec![root];
    let mut i = 0;
    while i < pids.len() {
        if let Some(kids) = children.get(&pids[i]) {
            for kid in kids {
                if !pids.contains(kid) {
                    pids.push(*kid);
                }
            }
        }
        i += 1;
    }
    pids
}

fn collect_listeners() -> Vec<Listener> {
    let names = process_names();
    let mut listeners = raw_listeners();
    for listener in &mut listeners {
        if listener.process.is_none() {
            listener.process = names.get(&listener.pid).cloned();
        }
        listener.address = normalize_bind(&listener.address);
    }
    listeners
}

fn raw_listeners() -> Vec<Listener> {
    #[cfg(windows)]
    {
        return windows_listeners();
    }
    #[cfg(not(windows))]
    {
        unix_listeners()
    }
}

fn process_names() -> HashMap<u32, String> {
    #[cfg(windows)]
    {
        return windows_process_names();
    }
    #[cfg(not(windows))]
    {
        unix_process_names()
    }
}

#[cfg(windows)]
fn windows_process_names() -> HashMap<u32, String> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut names = HashMap::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap == INVALID_HANDLE_VALUE {
            return names;
        }
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                names.insert(entry.th32ProcessID, wide_to_string(&entry.szExeFile));
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    names
}

#[cfg(windows)]
fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|unit| *unit == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(windows)]
fn windows_listeners() -> Vec<Listener> {
    let mut cmd = Command::new("netstat");
    cmd.args(["-ano"]);
    hide_console(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let Ok(output) = cmd.output() else {
        return Vec::new();
    };
    parse_netstat(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(windows)]
fn parse_netstat(text: &str) -> Vec<Listener> {
    let mut listeners = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_uppercase().starts_with("TCP") {
            continue;
        }
        if !trimmed.to_ascii_uppercase().contains("LISTENING") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let local = parts[1];
        let pid = parts
            .last()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        if pid == 0 {
            continue;
        }
        if let Some((address, port)) = split_local_addr(local) {
            listeners.push(Listener {
                port,
                address,
                pid,
                process: None,
            });
        }
    }
    listeners
}

fn split_local_addr(local: &str) -> Option<(String, u16)> {
    let local = local.trim();
    if let Some(rest) = local.strip_prefix('[') {
        let end = rest.find("]:")?;
        let host = rest[..end].to_string();
        let port = parse_port_token(&rest[end + 2..])?;
        return Some((host, port));
    }
    let (host, port) = local.rsplit_once(':')?;
    Some((host.to_string(), parse_port_token(port)?))
}

#[allow(dead_code)]
fn port_from_local_addr(local: &str) -> Option<u16> {
    split_local_addr(local).map(|(_, port)| port)
}

#[cfg(not(windows))]
fn unix_process_names() -> HashMap<u32, String> {
    let mut cmd = Command::new("ps");
    cmd.args(["-axo", "pid=,comm="]);
    hide_console(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let Ok(output) = cmd.output() else {
        return HashMap::new();
    };
    let mut names = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        let Some((pid, name)) = trimmed.split_once(char::is_whitespace) else {
            continue;
        };
        if let Ok(pid) = pid.parse::<u32>() {
            names.insert(pid, name.trim().to_string());
        }
    }
    names
}

#[cfg(not(windows))]
fn unix_listeners() -> Vec<Listener> {
    if let Some(listeners) =
        run_hidden_listeners("lsof", &["-nP", "-iTCP", "-sTCP:LISTEN"], parse_lsof)
    {
        if !listeners.is_empty() {
            return listeners;
        }
    }
    run_hidden_listeners("ss", &["-lptn"], parse_ss).unwrap_or_default()
}

#[cfg(not(windows))]
fn run_hidden_listeners(
    bin: &str,
    args: &[&str],
    parse: fn(&str) -> Vec<Listener>,
) -> Option<Vec<Listener>> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    hide_console(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let output = cmd.output().ok()?;
    if !output.status.success() && output.stdout.is_empty() {
        return None;
    }
    Some(parse(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(not(windows))]
fn parse_lsof(text: &str) -> Vec<Listener> {
    let mut listeners = Vec::new();
    for line in text.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }
        let Ok(pid) = parts[1].parse::<u32>() else {
            continue;
        };
        let name = parts[8];
        let parsed = split_local_addr(name).or_else(|| {
            name.rsplit(':')
                .next()
                .and_then(parse_port_token)
                .map(|port| (guess_lsof_host(name), port))
        });
        let Some((address, port)) = parsed else {
            continue;
        };
        listeners.push(Listener {
            port,
            address,
            pid,
            process: Some(parts[0].to_string()),
        });
    }
    listeners
}

#[cfg(not(windows))]
fn guess_lsof_host(name: &str) -> String {
    if name.starts_with('*') {
        return "0.0.0.0".into();
    }
    name.rsplit_once(':')
        .map(|(host, _)| {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .to_string()
        })
        .unwrap_or_else(|| "0.0.0.0".into())
}

#[cfg(not(windows))]
fn parse_ss(text: &str) -> Vec<Listener> {
    let mut listeners = Vec::new();
    for line in text.lines() {
        if !line.to_ascii_uppercase().contains("LISTEN") {
            continue;
        }
        let pid = line.split("pid=").nth(1).and_then(|rest| {
            rest.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        });
        let Some(pid) = pid else {
            continue;
        };
        let process = line.split("users:((").nth(1).and_then(|rest| {
            rest.trim_start_matches(|c| c == '"' || c == '\'')
                .split(|c| c == '"' || c == '\'' || c == ',')
                .next()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
        for token in line.split_whitespace() {
            if let Some((address, port)) = split_local_addr(token) {
                listeners.push(Listener {
                    port,
                    address,
                    pid,
                    process: process.clone(),
                });
                break;
            }
        }
    }
    listeners
}
