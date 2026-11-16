//! Optional Node config-bridge: resolve ports from Vite/Next configs.
//!
//! Orbitry never requires Node. When the plugin and Node are available, a
//! short-lived, hidden process evaluates the project's own config and exits.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{config_path, display_path, ConfigBridgeMode};
use crate::ports::{merge_ports, PortInfo, PortSource};
use crate::scripts::DetectedScript;
use crate::ScriptKind;

const PLUGIN_JS: &str = include_str!("../../../plugins/config-bridge/index.mjs");
const PLUGIN_PKG: &str = include_str!("../../../plugins/config-bridge/package.json");
const TIMEOUT: Duration = Duration::from_secs(8);
const CONCURRENCY: usize = 4;
const MAX_WALK_DEPTH: usize = 6;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const CONFIG_FILES: &[&str] = &[
    "vite.config.ts",
    "vite.config.mts",
    "vite.config.js",
    "vite.config.mjs",
    "vite.config.cjs",
    "next.config.ts",
    "next.config.mts",
    "next.config.js",
    "next.config.mjs",
    "next.config.cjs",
    "astro.config.ts",
    "astro.config.mjs",
    "astro.config.js",
];

static CURRENT: Mutex<Option<Arc<Session>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub mode: ConfigBridgeMode,
    pub available: bool,
    pub node: bool,
    pub plugin: bool,
    pub label: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub path: PathBuf,
    pub version: Option<String>,
}

pub fn bridge_status(mode: ConfigBridgeMode) -> BridgeStatus {
    let plugin_error = match ensure_plugin() {
        Some(_) => None,
        None => Some("The config-bridge plugin could not be written.".to_string()),
    };
    let info = probe_node();
    let node = info.is_some();
    let plugin = locate_plugin().is_some();
    let node_version = info.as_ref().and_then(|item| item.version.clone());
    let node_path = info.as_ref().map(|item| display_path(&item.path));
    let (state, label, error) = if matches!(mode, ConfigBridgeMode::Off) {
        ("disabled", "disabled".to_string(), None)
    } else if let Some(err) = plugin_error.clone().filter(|_| !plugin) {
        ("error", format!("error: {err}"), Some(err))
    } else if !node {
        ("node-missing", "Node missing".to_string(), None)
    } else if !plugin {
        ("plugin-missing", "plugin missing".to_string(), None)
    } else if let Some(version) = &node_version {
        ("available", format!("available · Node {version}"), None)
    } else {
        ("available", "available".to_string(), None)
    };
    BridgeStatus {
        mode,
        available: node && plugin && !matches!(mode, ConfigBridgeMode::Off),
        node,
        plugin,
        label,
        state: state.to_string(),
        node_version,
        node_path,
        error,
    }
}

pub fn plugin_dir() -> PathBuf {
    config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("plugins")
        .join("config-bridge")
}

pub fn cache_path() -> PathBuf {
    config_path().with_file_name("bridge-cache.json")
}

pub fn ensure_plugin() -> Option<PathBuf> {
    let dir = plugin_dir();
    if let Err(err) = fs::create_dir_all(&dir) {
        eprintln!("orbitry: config-bridge: could not create {}: {err}", dir.display());
        return locate_plugin();
    }
    let script = dir.join("index.mjs");
    let pkg = dir.join("package.json");
    if let Err(err) = write_if_changed(&script, PLUGIN_JS) {
        eprintln!("orbitry: config-bridge: could not write plugin: {err}");
        return locate_plugin();
    }
    let _ = write_if_changed(&pkg, PLUGIN_PKG);
    if script.is_file() {
        Some(script)
    } else {
        locate_plugin()
    }
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    let existing = fs::read_to_string(path).ok();
    if existing.as_deref() == Some(contents) {
        return Ok(());
    }
    fs::write(path, contents).map_err(|err| format!("write {}: {err}", path.display()))
}

pub fn locate_plugin() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(plugin_dir().join("index.mjs"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("plugins/config-bridge/index.mjs"));
            push_ancestor_plugins(dir, &mut candidates);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_ancestor_plugins(&cwd, &mut candidates);
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn push_ancestor_plugins(start: &Path, candidates: &mut Vec<PathBuf>) {
    let mut current = Some(start);
    for _ in 0..8 {
        let Some(dir) = current else { break };
        candidates.push(dir.join("plugins/config-bridge/index.mjs"));
        current = dir.parent();
    }
}

static NODE_CACHE: Mutex<Option<(Instant, Option<NodeInfo>)>> = Mutex::new(None);
const NODE_TTL: Duration = Duration::from_secs(60);

pub fn find_node() -> Option<PathBuf> {
    probe_node().map(|info| info.path)
}

pub fn probe_node() -> Option<NodeInfo> {
    if let Ok(cache) = NODE_CACHE.lock() {
        if let Some((at, info)) = cache.as_ref() {
            if at.elapsed() < NODE_TTL {
                return info.clone();
            }
        }
    }
    let info = look_up_node();
    if let Ok(mut cache) = NODE_CACHE.lock() {
        *cache = Some((Instant::now(), info.clone()));
    }
    info
}

fn look_up_node() -> Option<NodeInfo> {
    let path = locate_node_binary()?;
    let version = read_node_version(&path);
    Some(NodeInfo { path, version })
}

fn locate_node_binary() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(exe);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(path) = which_on_path("node") {
        return Some(path);
    }
    let mut extras = Vec::new();
    if cfg!(windows) {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            extras.push(PathBuf::from(program_files).join("nodejs").join(exe));
        }
        if let Some(symlink) = std::env::var_os("NVM_SYMLINK") {
            extras.push(PathBuf::from(symlink).join(exe));
        }
        if let Some(home) = std::env::var_os("NVM_HOME") {
            extras.push(PathBuf::from(home).join(exe));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            extras.push(PathBuf::from(local).join("Programs").join("nodejs").join(exe));
        }
    }
    extras.into_iter().find(|path| path.is_file())
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let (bin, arg) = if cfg!(windows) {
        ("where.exe", name)
    } else {
        ("which", name)
    };
    let mut command = Command::new(bin);
    command.arg(arg).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let first = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .to_string();
    let path = PathBuf::from(first);
    path.is_file().then_some(path)
}

fn read_node_version(path: &Path) -> Option<String> {
    let mut command = Command::new(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCacheEntry {
    pub fingerprint: String,
    pub result: BridgeResult,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub config_path: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub strict_port: bool,
    #[serde(default)]
    pub preview: Option<BridgePreview>,
    #[serde(default)]
    pub proxy: Vec<BridgeProxy>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePreview {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub host: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeProxy {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub target: String,
}

struct Session {
    enabled: bool,
    node: Option<PathBuf>,
    plugin: Option<PathBuf>,
    cache: Mutex<BTreeMap<String, BridgeCacheEntry>>,
    slots: Semaphore,
}

struct Semaphore {
    inner: Mutex<usize>,
    cvar: Condvar,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Self {
            inner: Mutex::new(permits),
            cvar: Condvar::new(),
        }
    }

    fn acquire(&self) -> SemaphoreGuard<'_> {
        let mut left = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        while *left == 0 {
            left = self.cvar.wait(left).unwrap_or_else(|err| err.into_inner());
        }
        *left -= 1;
        SemaphoreGuard(self)
    }
}

struct SemaphoreGuard<'a>(&'a Semaphore);

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut left) = self.0.inner.lock() {
            *left += 1;
            self.0.cvar.notify_one();
        }
    }
}

pub struct SessionGuard {
    session: Arc<Session>,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.session.flush();
        if let Ok(mut current) = CURRENT.lock() {
            *current = None;
        }
    }
}

impl Session {
    fn new(mode: ConfigBridgeMode) -> Self {
        let plugin = ensure_plugin().or_else(locate_plugin);
        let node = find_node();
        let enabled = match mode {
            ConfigBridgeMode::Off => false,
            ConfigBridgeMode::Auto => node.is_some() && plugin.is_some(),
            ConfigBridgeMode::On => {
                if node.is_none() {
                    eprintln!("orbitry: config-bridge: Node was not found on PATH");
                }
                if plugin.is_none() {
                    eprintln!("orbitry: config-bridge: plugin script was not found");
                }
                node.is_some() && plugin.is_some()
            }
        };
        Self {
            enabled,
            node,
            plugin,
            cache: Mutex::new(load_cache()),
            slots: Semaphore::new(CONCURRENCY),
        }
    }

    fn ports_for(&self, dir: &Path, scripts: &[DetectedScript]) -> Vec<PortInfo> {
        let Some(result) = self.resolve(dir) else {
            return Vec::new();
        };
        ports_from_result(&result, scripts)
    }

    fn resolve(&self, dir: &Path) -> Option<BridgeResult> {
        if !self.enabled {
            return None;
        }
        let Some(fingerprint) = dir_fingerprint(dir) else {
            return None;
        };
        let key = cache_key(dir);
        if let Ok(cache) = self.cache.lock() {
            if let Some(entry) = cache.get(&key) {
                if entry.fingerprint == fingerprint {
                    return if entry.result.ok {
                        Some(entry.result.clone())
                    } else {
                        None
                    };
                }
            }
        }
        let result = self.run_plugin(dir);
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                key,
                BridgeCacheEntry {
                    fingerprint,
                    result: result.clone().unwrap_or_default(),
                },
            );
        }
        result
    }

    fn run_plugin(&self, dir: &Path) -> Option<BridgeResult> {
        let node = self.node.as_ref()?;
        let plugin = self.plugin.as_ref()?;
        let _slot = self.slots.acquire();
        match invoke_plugin(node, plugin, dir) {
            Ok(result) if result.ok => Some(result),
            Ok(result) => {
                if let Some(error) = result.error.as_deref() {
                    if error != "no supported config file" {
                        eprintln!(
                            "orbitry: config-bridge: {}: {error}",
                            display_path(dir)
                        );
                    }
                }
                None
            }
            Err(err) => {
                eprintln!("orbitry: config-bridge: {}: {err}", display_path(dir));
                None
            }
        }
    }

    fn prefetch(&self, dirs: Vec<PathBuf>) {
        if !self.enabled || dirs.is_empty() {
            return;
        }
        let session = {
            let current = CURRENT.lock().ok();
            current.and_then(|guard| guard.clone())
        };
        let Some(session) = session else { return };
        let mut handles = Vec::new();
        for dir in dirs {
            let session = Arc::clone(&session);
            handles.push(thread::spawn(move || {
                let _ = session.resolve(&dir);
            }));
        }
        for handle in handles {
            let _ = handle.join();
        }
    }

    fn flush(&self) {
        let Ok(cache) = self.cache.lock() else { return };
        let path = cache_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(json) = serde_json::to_string_pretty(&*cache) else {
            return;
        };
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, json + "\n").is_err() {
            return;
        }
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        if fs::rename(&tmp, &path).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }

    fn export(&self) -> BTreeMap<String, BridgeCacheEntry> {
        self.cache
            .lock()
            .map(|cache| cache.clone())
            .unwrap_or_default()
    }
}

pub fn begin(mode: ConfigBridgeMode, roots: &[String]) -> SessionGuard {
    let session = Arc::new(Session::new(mode));
    if let Ok(mut current) = CURRENT.lock() {
        *current = Some(Arc::clone(&session));
    }
    if session.enabled {
        session.prefetch(find_candidate_dirs(roots));
    }
    SessionGuard { session }
}

pub fn ports_for(dir: &Path, scripts: &[DetectedScript]) -> Vec<PortInfo> {
    let session = CURRENT.lock().ok().and_then(|guard| guard.clone());
    session
        .map(|session| session.ports_for(dir, scripts))
        .unwrap_or_default()
}

pub fn export_cache() -> BTreeMap<String, BridgeCacheEntry> {
    CURRENT
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|session| session.export()))
        .unwrap_or_else(load_cache)
}

fn load_cache() -> BTreeMap<String, BridgeCacheEntry> {
    let path = cache_path();
    let Ok(data) = fs::read(&path) else {
        return BTreeMap::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

fn cache_key(dir: &Path) -> String {
    display_path(&crate::config::canonical(dir)).to_ascii_lowercase()
}

fn dir_fingerprint(dir: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for name in CONFIG_FILES {
        let path = dir.join(name);
        if let Some(mark) = file_fingerprint(&path) {
            parts.push(format!("{name}:{mark}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

fn file_fingerprint(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let bytes = fs::read(path).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in &bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{mtime}:{}:{hash:016x}", bytes.len()))
}

fn find_candidate_dirs(roots: &[String]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let root = PathBuf::from(root);
        if !root.exists() {
            continue;
        }
        walk_candidates(&root, 0, &mut found, &mut seen);
    }
    found
}

fn walk_candidates(
    dir: &Path,
    depth: usize,
    found: &mut Vec<PathBuf>,
    seen: &mut std::collections::HashSet<String>,
) {
    if depth > MAX_WALK_DEPTH || should_skip(dir) {
        return;
    }
    if dir_fingerprint(dir).is_some() {
        let key = cache_key(dir);
        if seen.insert(key) {
            found.push(dir.to_path_buf());
        }
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    children.sort();
    for child in children {
        walk_candidates(&child, depth + 1, found, seen);
    }
}

fn should_skip(path: &Path) -> bool {
    const SKIP: &[&str] = &[
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
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SKIP.iter().any(|skip| name.eq_ignore_ascii_case(skip)))
}

fn invoke_plugin(node: &Path, plugin: &Path, cwd: &Path) -> Result<BridgeResult, String> {
    let mut command = Command::new(node);
    command
        .arg(plugin)
        .arg("--cwd")
        .arg(cwd)
        .arg("--kind")
        .arg("auto")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("ORBITRY_CONFIG_BRIDGE", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("spawn node: {err}"))?;
    let stdout = child.stdout.take();
    let reader = thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        if let Some(mut stream) = stdout {
            let _ = stream.read_to_string(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("wait node: {err}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let pid = child.id();
            let _ = child.kill();
            let _ = child.wait();
            kill_tree(pid);
            return Err("timed out".into());
        }
        thread::sleep(Duration::from_millis(20));
    };
    let output = reader.join().unwrap_or_default();
    let line = output
        .lines()
        .rev()
        .find(|line| line.trim().starts_with('{'))
        .unwrap_or(output.trim());
    if line.is_empty() {
        return Err(if status.success() {
            "empty plugin output".into()
        } else {
            format!("plugin exited with status {status}")
        });
    }
    serde_json::from_str(line).map_err(|err| format!("plugin json: {err}"))
}

fn kill_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn ports_from_result(result: &BridgeResult, scripts: &[DetectedScript]) -> Vec<PortInfo> {
    let mut ports = Vec::new();
    if let Some(port) = result.port {
        let name = result.framework.clone();
        merge_ports(
            &mut ports,
            PortInfo::named(port, PortSource::Bridge, name).map(|mut info| {
                if let Some(host) = result.host.as_deref() {
                    info.binds = vec![host.to_string()];
                }
                info
            }),
        );
    }
    let wants_preview = scripts
        .iter()
        .any(|script| matches!(script.kind, ScriptKind::Preview));
    if wants_preview {
        if let Some(port) = result
            .preview
            .as_ref()
            .and_then(|preview| preview.port)
            .filter(|port| result.port != Some(*port))
        {
            merge_ports(
                &mut ports,
                PortInfo::named(port, PortSource::Bridge, Some("preview".into())),
            );
        }
    }
    for proxy in &result.proxy {
        if let Some(port) = port_from_target(&proxy.target) {
            if result.port == Some(port) {
                continue;
            }
            let name = if proxy.path.is_empty() {
                "proxy".to_string()
            } else {
                format!("proxy {}", proxy.path)
            };
            merge_ports(
                &mut ports,
                PortInfo::named(port, PortSource::Bridge, Some(name)),
            );
        }
    }
    ports
}

fn port_from_target(target: &str) -> Option<u16> {
    let trimmed = target.trim();
    let after = trimmed
        .split("://")
        .nth(1)
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .unwrap_or("");
    let port = after.rsplit(':').next()?;
    let parsed: u16 = port.parse().ok()?;
    if (1..=65535).contains(&parsed) && after.contains(':') {
        Some(parsed)
    } else {
        None
    }
}
