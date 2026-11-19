use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{config_path, normalize_path_str, paths_equal, Config};
use crate::databases::DatabaseInfo;
use crate::group::{ProjectPart, RepoGroup};
use crate::ports::{PortInfo, PortSource};

/// Bump this when the scan result shape or detection rules change in a way
/// that would make a previously stored snapshot misleading.
pub const SCANNER_VERSION: u32 = 4;

pub fn scan_cache_path() -> PathBuf {
    config_path().with_file_name("scan-cache.json")
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn config_fingerprint(config: &Config) -> String {
    let mut roots: Vec<String> = config
        .roots
        .iter()
        .map(|root| normalize_path_str(root).to_ascii_lowercase())
        .collect();
    roots.sort();
    roots.dedup();
    let mut patterns: Vec<(String, String, String)> = config
        .script_patterns
        .iter()
        .map(|pattern| {
            (
                format!("{:?}", pattern.kind).to_ascii_lowercase(),
                format!("{:?}", pattern.scope).to_ascii_lowercase(),
                pattern.pattern.to_ascii_lowercase(),
            )
        })
        .collect();
    patterns.sort();
    serde_json::to_string(&serde_json::json!({
        "roots": roots,
        "patterns": patterns,
    }))
        .unwrap_or_default()
}

fn fingerprints_compatible(stored: &str, current: &str) -> bool {
    if stored == current {
        return true;
    }
    let stored: serde_json::Value = match serde_json::from_str(stored) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let current: serde_json::Value = match serde_json::from_str(current) {
        Ok(value) => value,
        Err(_) => return false,
    };
    stored.get("roots") == current.get("roots") && stored.get("patterns") == current.get("patterns")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCache {
    pub scanner_version: u32,
    pub fingerprint: String,
    pub scanned_at: i64,
    pub groups: Vec<RepoGroup>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bridge: BTreeMap<String, crate::bridge::BridgeCacheEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanState {
    Cached,
    Scanning,
    Fresh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub state: ScanState,
    pub stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scanned_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<ScanDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ScanStatus {
    pub fn empty_scanning() -> Self {
        Self {
            state: ScanState::Scanning,
            stale: false,
            scanned_at: None,
            started_at: Some(now_ms()),
            diff: None,
            error: None,
        }
    }

    pub fn from_cache(scanned_at: i64) -> Self {
        Self {
            state: ScanState::Cached,
            stale: true,
            scanned_at: Some(scanned_at),
            started_at: None,
            diff: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDiff {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<ScanChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<ScanChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<ScanChange>,
}

impl ScanDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanChange {
    pub kind: ScanChangeKind,
    pub project: String,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanChangeKind {
    Project,
    Part,
    Script,
}

pub fn load_scan_cache(path: &Path, config: &Config) -> Option<ScanCache> {
    if !path.exists() {
        return None;
    }
    let data = fs::read(path).ok()?;
    let data = crate::config::strip_bom(&data);
    let cache: ScanCache = serde_json::from_slice(data).ok()?;
    if cache.scanner_version != SCANNER_VERSION {
        return None;
    }
    if !fingerprints_compatible(&cache.fingerprint, &config_fingerprint(config)) {
        return None;
    }
    Some(cache)
}

pub fn save_scan_cache(path: &Path, cache: &ScanCache) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(cache).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json + "\n").map_err(|err| format!("write {}: {err}", tmp.display()))?;
    if path.exists() {
        fs::remove_file(path).map_err(|err| format!("replace {}: {err}", path.display()))?;
    }
    fs::rename(&tmp, path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        format!("rename {}: {err}", path.display())
    })
}

pub fn diff_groups(old: &[RepoGroup], new: &[RepoGroup]) -> ScanDiff {
    let mut diff = ScanDiff::default();

    for group in new {
        match find_group(old, &group.id) {
            None => diff.added.push(project_change(group, None)),
            Some(previous) => diff_parts(previous, group, &mut diff),
        }
    }
    for group in old {
        if find_group(new, &group.id).is_none() {
            diff.removed.push(project_change(group, None));
        }
    }
    diff
}

fn diff_parts(old_group: &RepoGroup, new_group: &RepoGroup, diff: &mut ScanDiff) {
    for part in &new_group.parts {
        match find_part(&old_group.parts, &part.id) {
            None => diff.added.push(part_change(new_group, part, None)),
            Some(previous) => diff_part(old_group, previous, part, diff),
        }
    }
    for part in &old_group.parts {
        if find_part(&new_group.parts, &part.id).is_none() {
            diff.removed.push(part_change(old_group, part, None));
        }
    }
}

fn diff_part(
    group: &RepoGroup,
    old_part: &ProjectPart,
    new_part: &ProjectPart,
    diff: &mut ScanDiff,
) {
    for script in &new_part.scripts {
        match old_part
            .scripts
            .iter()
            .find(|item| item.name == script.name)
        {
            None => diff
                .added
                .push(script_change(group, new_part, &script.name, None)),
            Some(previous) if previous.command != script.command => {
                diff.changed.push(script_change(
                    group,
                    new_part,
                    &script.name,
                    Some(format!(
                        "command changed: {} → {}",
                        previous.command, script.command
                    )),
                ));
            }
            Some(_) => {}
        }
    }
    for script in &old_part.scripts {
        if !new_part
            .scripts
            .iter()
            .any(|item| item.name == script.name)
        {
            diff.removed
                .push(script_change(group, old_part, &script.name, None));
        }
    }

    if old_part.version != new_part.version {
        diff.changed.push(part_change(
            group,
            new_part,
            Some(format!(
                "version {} → {}",
                display_opt(old_part.version.as_deref()),
                display_opt(new_part.version.as_deref())
            )),
        ));
    }
    if old_part.framework != new_part.framework {
        diff.changed.push(part_change(
            group,
            new_part,
            Some(format!(
                "framework {} → {}",
                display_opt(old_part.framework.as_deref()),
                display_opt(new_part.framework.as_deref())
            )),
        ));
    }
    if static_port_set(&old_part.ports) != static_port_set(&new_part.ports) {
        diff.changed.push(part_change(
            group,
            new_part,
            Some("configured ports changed".to_string()),
        ));
    }
    if database_set(&old_part.databases) != database_set(&new_part.databases) {
        diff.changed.push(part_change(
            group,
            new_part,
            Some("local databases changed".to_string()),
        ));
    }
}

fn database_set(databases: &[DatabaseInfo]) -> Vec<(String, Option<u16>, Option<String>, String)> {
    let mut set: Vec<(String, Option<u16>, Option<String>, String)> = databases
        .iter()
        .map(|db| {
            (
                db.engine.to_ascii_lowercase(),
                db.port,
                db.path
                    .as_ref()
                    .map(|path| path.replace('\\', "/").to_ascii_lowercase()),
                format!("{:?}", db.source).to_ascii_lowercase(),
            )
        })
        .collect();
    set.sort();
    set.dedup();
    set
}

fn static_port_set(ports: &[PortInfo]) -> Vec<(u16, &'static str, Option<&str>)> {
    let mut set: Vec<(u16, &'static str, Option<&str>)> = ports
        .iter()
        .filter(|port| {
            matches!(
                port.source,
                PortSource::Default
                    | PortSource::Config
                    | PortSource::Env
                    | PortSource::Flag
                    | PortSource::Bridge
            )
        })
        .map(|port| {
            let source = match port.source {
                PortSource::Default => "default",
                PortSource::Config => "config",
                PortSource::Env => "env",
                PortSource::Flag => "flag",
                PortSource::Bridge => "bridge",
                PortSource::Log => "log",
                PortSource::Process => "process",
            };
            (port.port, source, port.name.as_deref())
        })
        .collect();
    set.sort();
    set
}

fn find_group<'a>(groups: &'a [RepoGroup], id: &str) -> Option<&'a RepoGroup> {
    groups.iter().find(|group| paths_equal(&group.id, id))
}

fn find_part<'a>(parts: &'a [ProjectPart], id: &str) -> Option<&'a ProjectPart> {
    parts.iter().find(|part| paths_equal(&part.id, id))
}

fn project_change(group: &RepoGroup, detail: Option<String>) -> ScanChange {
    ScanChange {
        kind: ScanChangeKind::Project,
        project: group.name.clone(),
        project_id: group.id.clone(),
        part: None,
        part_id: None,
        script: None,
        detail,
    }
}

fn part_change(group: &RepoGroup, part: &ProjectPart, detail: Option<String>) -> ScanChange {
    ScanChange {
        kind: ScanChangeKind::Part,
        project: group.name.clone(),
        project_id: group.id.clone(),
        part: Some(part_label(part)),
        part_id: Some(part.id.clone()),
        script: None,
        detail,
    }
}

fn script_change(
    group: &RepoGroup,
    part: &ProjectPart,
    script: &str,
    detail: Option<String>,
) -> ScanChange {
    ScanChange {
        kind: ScanChangeKind::Script,
        project: group.name.clone(),
        project_id: group.id.clone(),
        part: Some(part_label(part)),
        part_id: Some(part.id.clone()),
        script: Some(script.to_string()),
        detail,
    }
}

fn part_label(part: &ProjectPart) -> String {
    if !part.relative_path.is_empty() && part.relative_path != "." {
        part.relative_path.clone()
    } else {
        part.name.clone()
    }
}

fn display_opt(value: Option<&str>) -> &str {
    value.filter(|item| !item.is_empty()).unwrap_or("none")
}
