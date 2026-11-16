use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::config::normalize_path_str;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub commit: String,
    pub dirty: bool,
}

static CACHE: Mutex<Option<HashMap<String, Option<GitInfo>>>> = Mutex::new(None);

/// Drop cached repo info so the next scan reads git fresh.
pub fn clear_git_cache() {
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
}

fn cache_key(repo: &Path) -> String {
    let raw = normalize_path_str(&repo.to_string_lossy());
    if cfg!(windows) {
        raw.to_ascii_lowercase()
    } else {
        raw
    }
}

pub fn detect_git(path: &Path) -> Option<GitInfo> {
    let repo = find_git_root(path)?;
    let key = cache_key(&repo);
    if let Ok(cache) = CACHE.lock() {
        if let Some(map) = cache.as_ref() {
            if let Some(info) = map.get(&key) {
                return info.clone();
            }
        }
    }
    let info = detect_git_fresh(&repo);
    if let Ok(mut cache) = CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(key, info.clone());
    }
    info
}

fn detect_git_fresh(repo: &Path) -> Option<GitInfo> {
    let branch = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let commit = git(repo, &["rev-parse", "--short", "HEAD"])?;
    let status = git(repo, &["status", "--porcelain"])?;
    Some(GitInfo {
        branch,
        commit,
        dirty: !status.is_empty(),
    })
}

pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    // A GUI-subsystem exe has no console, so every git call would otherwise
    // pop up its own console window. Keep them hidden.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() && args.first().copied() != Some("status") {
        return None;
    }
    Some(text)
}
