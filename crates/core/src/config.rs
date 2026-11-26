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

/// Node.js plugin toggle. Plain on/off from 0.14.1; older `"auto"` is migrated
/// on load (on when Node + plugin were effectively available, otherwise off).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigBridgeMode {
    #[default]
    On,
    Off,
}

impl ConfigBridgeMode {
    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }
}

impl<'de> Deserialize<'de> for ConfigBridgeMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            // "auto" is accepted so older configs parse; load_config rewrites it.
            "on" | "auto" | "" => Ok(Self::On),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["on", "off"],
            )),
        }
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

/// Per-app autostart entry. `script` is the package/AHK script name to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartApp {
    pub part_id: String,
    pub script: String,
}

impl<'de> Deserialize<'de> for AutostartApp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Legacy(String),
            Entry {
                #[serde(alias = "part_id")]
                part_id: String,
                #[serde(default)]
                script: String,
            },
        }
        match Raw::deserialize(deserializer)? {
            Raw::Legacy(part_id) => Ok(Self {
                part_id,
                script: String::new(),
            }),
            Raw::Entry { part_id, script } => Ok(Self { part_id, script }),
        }
    }
}

impl AutostartApp {
    pub fn new(part_id: impl Into<String>, script: impl Into<String>) -> Self {
        Self {
            part_id: part_id.into(),
            script: script.into(),
        }
    }
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
    /// Apps to start when Orbitry starts. Present from 0.15.1 as
    /// `{ partId, script }` objects. Legacy bare id strings still deserialize
    /// (script empty) and are resolved or pruned against the inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub autostart_apps: Vec<AutostartApp>,
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
    /// AutoHotkey plugin (Windows). Default on; older configs omit it and keep
    /// the default. On non-Windows hosts the plugin stays unavailable.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub ahk_plugin: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            script_patterns: Vec::new(),
            port_overrides: Vec::new(),
            config_bridge: ConfigBridgeMode::On,
            auto_check_updates: true,
            log_limit_mb: default_log_limit_mb(),
            autostart_apps: Vec::new(),
            desktop_notifications: false,
            ignored_projects: Vec::new(),
            open_in: OpenInTargets::default(),
            models_include_docs: false,
            ahk_plugin: true,
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
        let mut unique: Vec<AutostartApp> = Vec::new();
        for mut entry in self.autostart_apps.drain(..) {
            entry.part_id = entry.part_id.trim().to_string();
            entry.script = entry.script.trim().to_string();
            if entry.part_id.is_empty() {
                continue;
            }
            if let Some(existing) = unique
                .iter_mut()
                .find(|item| paths_equal(&item.part_id, &entry.part_id))
            {
                // Last write wins for the script when the same part appears twice.
                if !entry.script.is_empty() {
                    existing.script = entry.script;
                }
                continue;
            }
            unique.push(entry);
        }
        self.autostart_apps = unique;
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
    let mut value: serde_json::Value =
        serde_json::from_slice(data).map_err(|err| format!("parse {}: {err}", path.display()))?;
    let migrated = migrate_config_value(&mut value);
    let config: Config =
        serde_json::from_value(value).map_err(|err| format!("parse {}: {err}", path.display()))?;
    if migrated {
        // Persist on/off so older "auto" does not linger on disk.
        let _ = save_config(path, &config);
    }
    Ok(config)
}

/// Rewrite legacy plugin values in a parsed config JSON object.
/// Returns true when the value was changed and should be saved.
fn migrate_config_value(value: &mut serde_json::Value) -> bool {
    let Some(obj) = value.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    let bridge = obj
        .get("configBridge")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if bridge.eq_ignore_ascii_case("auto") {
        let on = crate::bridge::deps_available();
        obj.insert(
            "configBridge".to_string(),
            serde_json::Value::String(if on { "on" } else { "off" }.to_string()),
        );
        changed = true;
    }
    changed
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
