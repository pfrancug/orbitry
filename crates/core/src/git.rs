use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub commit: String,
    pub dirty: bool,
}

pub fn detect_git(path: &Path) -> Option<GitInfo> {
    let repo = find_git_root(path)?;
    let branch = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let commit = git(&repo, &["rev-parse", "--short", "HEAD"])?;
    let status = git(&repo, &["status", "--porcelain"])?;
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
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() && args.first().copied() != Some("status") {
        return None;
    }
    Some(text)
}