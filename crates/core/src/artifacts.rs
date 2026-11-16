use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const BUILD_DIRS: &[&str] = &[
    "dist",
    "build",
    ".next",
    "out",
    ".svelte-kit/output",
    ".output",
    ".nuxt",
];

const SOURCE_ENTRIES: &[&str] = &[
    "src",
    "lib",
    "app",
    "package.json",
    "vite.config.ts",
    "vite.config.js",
    "vite.config.mjs",
    "svelte.config.js",
    "svelte.config.ts",
    "next.config.js",
    "next.config.mjs",
    "next.config.ts",
    "nuxt.config.ts",
    "nuxt.config.js",
    "astro.config.mjs",
    "astro.config.ts",
];

const SOURCE_SKIP: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "target",
    ".next",
    ".svelte-kit",
    ".output",
    ".nuxt",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub built: bool,
    pub dir: Option<String>,
    pub last_built_at: Option<i64>,
    #[serde(default)]
    pub stale: bool,
}

impl BuildInfo {
    pub fn missing() -> Self {
        Self {
            built: false,
            dir: None,
            last_built_at: None,
            stale: false,
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
        Some((dir, last_built_at)) => {
            let sources_at = newest_source_mtime(project_dir);
            let stale = sources_at.is_some_and(|src| src > last_built_at);
            BuildInfo {
                built: true,
                dir: Some(dir),
                last_built_at: Some(last_built_at),
                stale,
            }
        }
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

fn newest_source_mtime(project_dir: &Path) -> Option<i64> {
    let mut best: Option<i64> = None;
    for name in SOURCE_ENTRIES {
        let path = project_dir.join(name);
        if !path.exists() {
            continue;
        }
        let depth_cap = if path.is_dir() { 2 } else { 0 };
        if let Some(stamp) = newest_mtime_skipping(&path, 0, depth_cap) {
            best = Some(best.map_or(stamp, |current| current.max(stamp)));
        }
    }
    best
}

fn newest_mtime(path: &Path, depth: u8) -> Option<i64> {
    newest_mtime_skipping(path, depth, 2)
}

fn newest_mtime_skipping(path: &Path, depth: u8, cap: u8) -> Option<i64> {
    let meta = fs::metadata(path).ok()?;
    let mut best = to_unix(meta.modified().ok()?);
    if !meta.is_dir() || depth >= cap {
        return Some(best);
    }
    let entries = fs::read_dir(path).ok()?;
    for entry in entries.flatten().take(80) {
        let child = entry.path();
        if let Some(name) = child.file_name().and_then(|name| name.to_str()) {
            if SOURCE_SKIP.iter().any(|skip| name.eq_ignore_ascii_case(skip)) {
                continue;
            }
        }
        if let Some(stamp) = newest_mtime_skipping(&child, depth + 1, cap) {
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
