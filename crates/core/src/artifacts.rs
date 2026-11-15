use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const BUILD_DIRS: &[&str] = &["dist", "build", ".next", "out"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub built: bool,
    pub dir: Option<String>,
    pub last_built_at: Option<i64>,
}

impl BuildInfo {
    pub fn missing() -> Self {
        Self {
            built: false,
            dir: None,
            last_built_at: None,
        }
    }
}

pub fn detect_build(project_dir: &Path) -> BuildInfo {
    let mut best: Option<(String, i64)> = None;
    for name in BUILD_DIRS {
        let dir = project_dir.join(name);
        if !dir.exists() {
            continue;
        }
        let Some(stamp) = newest_mtime(&dir, 0) else {
            continue;
        };
        match &best {
            Some((_, current)) if *current >= stamp => {}
            _ => best = Some(((*name).to_string(), stamp)),
        }
    }
    match best {
        Some((dir, last_built_at)) => BuildInfo {
            built: true,
            dir: Some(dir),
            last_built_at: Some(last_built_at),
        },
        None => BuildInfo::missing(),
    }
}

pub fn has_node_modules(project_dir: &Path) -> bool {
    for ancestor in project_dir.ancestors() {
        if ancestor.join("node_modules").is_dir() {
            return true;
        }
        if ancestor.join(".git").exists() {
            break;
        }
    }
    false
}

fn newest_mtime(path: &Path, depth: u8) -> Option<i64> {
    let meta = fs::metadata(path).ok()?;
    let mut best = to_unix(meta.modified().ok()?);
    if !meta.is_dir() || depth >= 2 {
        return Some(best);
    }
    let entries = fs::read_dir(path).ok()?;
    for entry in entries.flatten().take(80) {
        if let Some(stamp) = newest_mtime(&entry.path(), depth + 1) {
            if stamp > best {
                best = stamp;
            }
        }
    }
    Some(best)
}

fn to_unix(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
