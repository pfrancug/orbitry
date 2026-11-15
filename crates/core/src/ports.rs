//! Static and live port detection for project parts and running processes.
//!
//! An optional Node config-bridge plugin can later resolve code-based configs
//! exactly. When that plugin is missing (the default today), this module falls
//! back to environment files, CLI flags, regex on config sources, framework
//! defaults, log lines, and the live PID tree.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::scripts::DetectedScript;
use crate::util::hide_console;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortSource {
    Default,
    Config,
    Env,
    Flag,
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
            PortSource::Log => 4,
            PortSource::Process => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub port: u16,
    pub source: PortSource,
}

impl PortInfo {
    pub fn new(port: u16, source: PortSource) -> Option<Self> {
        if is_plausible_port(port) {
            Some(Self { port, source })
        } else {
            None
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
        } else {
            ports.push(port);
        }
    }
    ports.sort_by_key(|item| item.port);
}

/// Hook for the optional Node config-bridge plugin. Returns `None` when the
/// plugin is not installed, which is the usual case today.
pub fn config_bridge_ports(_dir: &Path) -> Option<Vec<PortInfo>> {
    None
}

pub fn detect_static_ports(
    dir: &Path,
    scripts: &[DetectedScript],
    script_filter: Option<&str>,
    framework: Option<&str>,
) -> Vec<PortInfo> {
    if let Some(ports) = config_bridge_ports(dir) {
        if !ports.is_empty() {
            return ports;
        }
    }

    let mut ports = Vec::new();
    collect_env_ports(dir, &mut ports);
    collect_config_ports(dir, &mut ports);
    collect_flag_ports(dir, scripts, script_filter, &mut ports);

    if ports.is_empty() {
        let inferred = framework
            .map(str::to_string)
            .or_else(|| infer_framework(dir));
        if let Some(port) = inferred.as_deref().and_then(default_port_for) {
            merge_ports(&mut ports, PortInfo::new(port, PortSource::Default));
        }
    }

    ports
}

pub fn ports_from_log(line: &str) -> Vec<PortInfo> {
    let mut found = Vec::new();
    for port in ports_from_host_urls(line) {
        merge_ports(&mut found, PortInfo::new(port, PortSource::Log));
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
                merge_ports(&mut found, PortInfo::new(port, PortSource::Log));
            }
        }
    }
    found
}

/// Listening TCP ports keyed by PID, then filtered to a process tree.
pub fn listening_ports_for(root_pid: u32) -> Vec<PortInfo> {
    let tree: BTreeSet<u32> = process_tree(root_pid).into_iter().collect();
    if tree.is_empty() {
        return Vec::new();
    }
    let table = listening_table();
    let mut ports = Vec::new();
    for pid in tree {
        if let Some(bound) = table.get(&pid) {
            for port in bound {
                merge_ports(&mut ports, PortInfo::new(*port, PortSource::Process));
            }
        }
    }
    ports
}

fn collect_env_ports(dir: &Path, ports: &mut Vec<PortInfo>) {
    const NAMES: &[&str] = &[
        ".env",
        ".env.local",
        ".env.development",
        ".env.development.local",
        ".env.dev",
    ];
    for folder in dir.ancestors().take(5) {
        for name in NAMES {
            let path = folder.join(name);
            if let Ok(text) = fs::read_to_string(&path) {
                for port in ports_from_env(&text) {
                    merge_ports(ports, PortInfo::new(port, PortSource::Env));
                }
            }
        }
        if folder.join(".git").exists() {
            break;
        }
    }
}

fn collect_config_ports(dir: &Path, ports: &mut Vec<PortInfo>) {
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
        if let Ok(text) = fs::read_to_string(&path) {
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

fn default_port_for(framework: &str) -> Option<u16> {
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
        if let Some(port) = parse_port_token(rest.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("")) {
            if !config_like || is_app_port(port) {
                ports.push(port);
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

fn listening_table() -> HashMap<u32, BTreeSet<u16>> {
    #[cfg(windows)]
    {
        return windows_listening_table();
    }
    #[cfg(not(windows))]
    {
        unix_listening_table()
    }
}

#[cfg(windows)]
fn windows_listening_table() -> HashMap<u32, BTreeSet<u16>> {
    let mut cmd = Command::new("netstat");
    cmd.args(["-ano"]);
    hide_console(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let Ok(output) = cmd.output() else {
        return HashMap::new();
    };
    parse_netstat(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(windows)]
fn parse_netstat(text: &str) -> HashMap<u32, BTreeSet<u16>> {
    let mut table: HashMap<u32, BTreeSet<u16>> = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_uppercase().starts_with("TCP") {
            continue;
        }
        if !trimmed.to_ascii_uppercase().contains("LISTENING") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        // TCP local foreign [state] pid  -- or with LISTENING before pid
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
        if let Some(port) = port_from_local_addr(local) {
            table.entry(pid).or_default().insert(port);
        }
    }
    table
}

fn port_from_local_addr(local: &str) -> Option<u16> {
    if let Some(rest) = local.strip_prefix('[') {
        let end = rest.find("]:")?;
        return parse_port_token(&rest[end + 2..]);
    }
    local.rsplit_once(':').and_then(|(_, port)| parse_port_token(port))
}

#[cfg(not(windows))]
fn unix_listening_table() -> HashMap<u32, BTreeSet<u16>> {
    if let Some(table) = run_hidden_table(
        "lsof",
        &["-nP", "-iTCP", "-sTCP:LISTEN"],
        parse_lsof,
    ) {
        if !table.is_empty() {
            return table;
        }
    }
    run_hidden_table("ss", &["-lptn"], parse_ss).unwrap_or_default()
}

#[cfg(not(windows))]
fn run_hidden_table(
    bin: &str,
    args: &[&str],
    parse: fn(&str) -> HashMap<u32, BTreeSet<u16>>,
) -> Option<HashMap<u32, BTreeSet<u16>>> {
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
fn parse_lsof(text: &str) -> HashMap<u32, BTreeSet<u16>> {
    let mut table: HashMap<u32, BTreeSet<u16>> = HashMap::new();
    for line in text.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }
        let Ok(pid) = parts[1].parse::<u32>() else {
            continue;
        };
        if let Some(port) = port_from_local_addr(parts[8])
            .or_else(|| parts[8].rsplit(':').next().and_then(parse_port_token))
        {
            table.entry(pid).or_default().insert(port);
        }
    }
    table
}

#[cfg(not(windows))]
fn parse_ss(text: &str) -> HashMap<u32, BTreeSet<u16>> {
    let mut table: HashMap<u32, BTreeSet<u16>> = HashMap::new();
    for line in text.lines() {
        if !line.to_ascii_uppercase().contains("LISTEN") {
            continue;
        }
        let pid = line
            .split("pid=")
            .nth(1)
            .and_then(|rest| {
                rest.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            });
        let Some(pid) = pid else {
            continue;
        };
        for token in line.split_whitespace() {
            if let Some(port) = port_from_local_addr(token) {
                table.entry(pid).or_default().insert(port);
                break;
            }
        }
    }
    table
}
