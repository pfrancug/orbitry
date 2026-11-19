use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptKind {
    Dev,
    Preview,
    Build,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptScope {
    Fe,
    Be,
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptPattern {
    pub kind: ScriptKind,
    pub scope: ScriptScope,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortOverride {
    pub part_id: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_url: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore: bool,
}

impl PortOverride {
    pub fn matches_part(&self, part_id: &str) -> bool {
        paths_equal(&self.part_id, part_id)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigBridgeMode {
    #[default]
    Auto,
    On,
    Off,
}

impl ConfigBridgeMode {
    pub fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }

    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }
}

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Which "Open in …" actions appear in the details panel.
/// Missing fields (and missing `openIn` entirely) default to visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInTargets {
    #[serde(default = "default_true")]
    pub terminal: bool,
    #[serde(default = "default_true")]
    pub vscode: bool,
    #[serde(default = "default_true")]
    pub cursor: bool,
    #[serde(default = "default_true")]
    pub explorer: bool,
}

impl Default for OpenInTargets {
    fn default() -> Self {
        Self {
            terminal: true,
            vscode: true,
            cursor: true,
            explorer: true,
        }
    }
}

impl OpenInTargets {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn any_enabled(&self) -> bool {
        self.terminal || self.vscode || self.cursor || self.explorer
    }
}

fn default_log_limit_mb() -> u32 {
    crate::logs::DEFAULT_LOG_LIMIT_MB
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub script_patterns: Vec<ScriptPattern>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub port_overrides: Vec<PortOverride>,
    #[serde(default)]
    pub config_bridge: ConfigBridgeMode,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub auto_check_updates: bool,
    #[serde(default = "default_log_limit_mb")]
    pub log_limit_mb: u32,
    /// Stable app ids (canonical display paths) to start when Orbitry starts.
    /// Kept even if a later scan temporarily misses the app.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub autostart_apps: Vec<String>,
    /// Desktop notifications for app crashes and red health. Off by default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub desktop_notifications: bool,
    /// Project ids (canonical group ids / display paths) that should produce
    /// no notices or warnings in the dashboard or as desktop notifications.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_projects: Vec<String>,
    /// Which Open-in actions are shown in the details panel. Defaults keep
    /// every target visible so older configs and first runs look unchanged.
    #[serde(default, skip_serializing_if = "OpenInTargets::is_default")]
    pub open_in: OpenInTargets,
    /// When true, models attribution also scans documentation Markdown
    /// (`README*.md`, `docs/**/*.md`). Default false so docs mentions do not
    /// attribute models. Older configs omit the field and keep the default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub models_include_docs: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            script_patterns: Vec::new(),
            port_overrides: Vec::new(),
            config_bridge: ConfigBridgeMode::Auto,
            auto_check_updates: true,
            log_limit_mb: default_log_limit_mb(),
            autostart_apps: Vec::new(),
            desktop_notifications: false,
            ignored_projects: Vec::new(),
            open_in: OpenInTargets::default(),
            models_include_docs: false,
        }
    }
}

impl Config {
    pub fn with_root(mut self, root: impl Into<String>) -> Self {
        let root = root.into();
        if !self
            .roots
            .iter()
            .any(|existing| paths_equal(existing, &root))
        {
            self.roots.push(root);
        }
        self
    }

    pub fn normalize_autostart_apps(&mut self) {
        normalize_id_list(&mut self.autostart_apps);
    }

    pub fn normalize_ignored_projects(&mut self) {
        normalize_id_list(&mut self.ignored_projects);
    }

    pub fn ignores_project(&self, group_id: &str, path: &str, name: &str) -> bool {
        self.ignored_projects.iter().any(|id| {
            paths_equal(id, group_id) || paths_equal(id, path) || id == name
        })
    }
}

fn normalize_id_list(ids: &mut Vec<String>) {
    let mut unique: Vec<String> = Vec::new();
    for id in ids.drain(..) {
        let id = id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        if !unique.iter().any(|existing| paths_equal(existing, &id)) {
            unique.push(id);
        }
    }
    *ids = unique;
}

pub fn config_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".orbitry")
        })
        .join("orbitry");
    dir.join("config.json")
}

/// Runtime file used to tell a second Orbitry instance which port is live.
pub fn instance_path() -> PathBuf {
    config_path().with_file_name("instance.json")
}

pub fn load_config(path: &Path) -> Result<Config, String> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let data = strip_bom(&data);
    serde_json::from_slice(data).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|err| err.to_string())?;
    fs::write(path, json + "\n").map_err(|err| format!("write {}: {err}", path.display()))
}

pub fn default_kind_patterns() -> Vec<(ScriptKind, &'static str)> {
    vec![
        (ScriptKind::Dev, "dev"),
        (ScriptKind::Dev, "dev:*"),
        (ScriptKind::Preview, "preview"),
        (ScriptKind::Preview, "preview:*"),
        (ScriptKind::Preview, "start"),
        (ScriptKind::Preview, "start:*"),
        (ScriptKind::Build, "build"),
        (ScriptKind::Build, "build:*"),
    ]
}

pub fn pattern_matches(pattern: &str, name: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        if prefix.is_empty() {
            return true;
        }
        name.starts_with(prefix)
    } else {
        name.eq_ignore_ascii_case(pattern)
    }
}

pub fn display_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
}

pub fn paths_equal(a: &str, b: &str) -> bool {
    let na = normalize_path_str(a);
    let nb = normalize_path_str(b);
    if cfg!(windows) {
        na.eq_ignore_ascii_case(&nb)
    } else {
        na == nb
    }
}

pub fn normalize_path_str(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('/', r"\")
        .trim_end_matches(['\\', '/'])
        .to_string()
}

pub fn strip_bom(data: &[u8]) -> &[u8] {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..]
    } else {
        data
    }
}

pub fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
