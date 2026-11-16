//! Log-file layout, size limits, rotation, and cleanup for detached runs.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::config::config_path;
use crate::process::now_ms;

/// Default total log budget per app (live file + one rotation), in megabytes.
pub const DEFAULT_LOG_LIMIT_MB: u32 = 10;
pub const MIN_LOG_LIMIT_MB: u32 = 2;
pub const MAX_LOG_LIMIT_MB: u32 = 100;

/// Keep at most this many finished-run logs per app (part + script).
pub const FINISHED_LOGS_PER_APP: usize = 8;
/// Drop finished-run logs older than this.
pub const FINISHED_LOG_MAX_AGE: Duration = Duration::from_secs(3 * 24 * 60 * 60);

const MB: u64 = 1024 * 1024;
const BASE_GLOBAL_CAP: u64 = 200 * MB;

pub fn clamp_log_limit_mb(value: u32) -> u32 {
    value.clamp(MIN_LOG_LIMIT_MB, MAX_LOG_LIMIT_MB)
}

pub fn per_app_limit_bytes(limit_mb: u32) -> u64 {
    u64::from(clamp_log_limit_mb(limit_mb)) * MB
}

/// Half of the per-app budget goes to the live file; the other half is the rotated copy.
pub fn file_cap_bytes(limit_mb: u32) -> u64 {
    per_app_limit_bytes(limit_mb) / 2
}

pub fn global_cap_bytes(limit_mb: u32) -> u64 {
    BASE_GLOBAL_CAP.max(20 * per_app_limit_bytes(limit_mb))
}

pub fn logs_dir() -> PathBuf {
    config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("logs")
}

pub fn runs_path() -> PathBuf {
    config_path().with_file_name("runs.json")
}

pub fn log_limit_path() -> PathBuf {
    config_path().with_file_name("log-limit")
}

pub fn ensure_logs_dir() -> io::Result<PathBuf> {
    let dir = logs_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn run_log_path(run_id: &str) -> PathBuf {
    logs_dir().join(format!("{run_id}.log"))
}

pub fn run_log_rotated_path(run_id: &str) -> PathBuf {
    logs_dir().join(format!("{run_id}.log.1"))
}

pub fn run_system_log_path(run_id: &str) -> PathBuf {
    logs_dir().join(format!("{run_id}.system.log"))
}

pub fn write_log_limit(limit_mb: u32) -> io::Result<()> {
    let path = log_limit_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = file_cap_bytes(limit_mb);
    fs::write(path, format!("{bytes}\n"))
}

pub fn read_file_cap_bytes(fallback: u64) -> u64 {
    let Ok(data) = fs::read_to_string(log_limit_path()) else {
        return fallback;
    };
    data.trim()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub fn open_append(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

pub fn append_line(path: &Path, line: &str) -> io::Result<()> {
    let mut file = open_append(path)?;
    file.write_all(line.as_bytes())?;
    if !line.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    Ok(())
}

pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(path);
        fs::rename(&tmp, path).map_err(|_| err)?;
    }
    Ok(())
}

/// Rotate `path` to `path.1`, replacing any previous rotation.
pub fn rotate_log(path: &Path) -> io::Result<()> {
    let rotated = rotated_sibling(path);
    if rotated.exists() {
        let _ = fs::remove_file(&rotated);
    }
    if path.exists() {
        fs::rename(path, &rotated)?;
    }
    Ok(())
}

fn rotated_sibling(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("log");
    if let Some(parent) = path.parent() {
        parent.join(format!("{name}.1"))
    } else {
        PathBuf::from(format!("{name}.1"))
    }
}

pub fn read_last_lines(paths: &[&Path], cap: usize) -> Vec<String> {
    // bool is_system is decided by the caller; here we just return text lines in order.
    let mut lines = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
    }
    if lines.len() > cap {
        lines = lines.split_off(lines.len() - cap);
    }
    lines
}

#[derive(Debug, Clone)]
pub struct LogFileInfo {
    pub run_id: String,
    pub path: PathBuf,
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub running: bool,
}

/// All log files that belong to a run id (live, rotation, system).
pub fn collect_log_files(running_ids: &[String]) -> Vec<LogFileInfo> {
    let Ok(entries) = fs::read_dir(logs_dir()) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(run_id) = run_id_from_log_name(name) else {
            continue;
        };
        let meta = entry.metadata().ok();
        files.push(LogFileInfo {
            running: running_ids.iter().any(|id| id == &run_id),
            run_id,
            path,
            len: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            modified: meta.and_then(|m| m.modified().ok()),
        });
    }
    files
}

fn run_id_from_log_name(name: &str) -> Option<String> {
    let stem = name
        .strip_suffix(".system.log")
        .or_else(|| name.strip_suffix(".log.1"))
        .or_else(|| name.strip_suffix(".log"))?;
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

#[derive(Debug, Clone)]
pub struct FinishedLog {
    pub run_id: String,
    pub part_key: String,
    pub ended_at_ms: i64,
}

/// Delete logs for finished runs that are too old, exceed the per-app count, or
/// push the directory over the global cap. Never touches logs of a running app.
pub fn cleanup_logs(
    limit_mb: u32,
    running_ids: &[String],
    finished: &[FinishedLog],
) {
    let _ = ensure_logs_dir();
    let now = now_ms();
    let max_age_ms = FINISHED_LOG_MAX_AGE.as_millis() as i64;

    let mut delete_ids: Vec<String> = Vec::new();

    for item in finished {
        if running_ids.iter().any(|id| id == &item.run_id) {
            continue;
        }
        if item.ended_at_ms > 0 && now.saturating_sub(item.ended_at_ms) > max_age_ms {
            delete_ids.push(item.run_id.clone());
        }
    }

    let mut by_app: std::collections::HashMap<String, Vec<&FinishedLog>> =
        std::collections::HashMap::new();
    for item in finished {
        if running_ids.iter().any(|id| id == &item.run_id) {
            continue;
        }
        if delete_ids.iter().any(|id| id == &item.run_id) {
            continue;
        }
        by_app.entry(item.part_key.clone()).or_default().push(item);
    }
    for group in by_app.values_mut() {
        group.sort_by_key(|item| item.ended_at_ms);
        if group.len() > FINISHED_LOGS_PER_APP {
            let extra = group.len() - FINISHED_LOGS_PER_APP;
            for item in group.iter().take(extra) {
                delete_ids.push(item.run_id.clone());
            }
        }
    }

    for id in &delete_ids {
        delete_run_logs(id);
    }

    enforce_global_cap(limit_mb, running_ids, finished);
}

fn enforce_global_cap(limit_mb: u32, running_ids: &[String], finished: &[FinishedLog]) {
    let cap = global_cap_bytes(limit_mb);
    let files = collect_log_files(running_ids);
    let total: u64 = files.iter().map(|f| f.len).sum();
    if total <= cap {
        return;
    }

    let mut finished_ids: Vec<(i64, String)> = finished
        .iter()
        .filter(|item| running_ids.iter().all(|id| id != &item.run_id))
        .map(|item| (item.ended_at_ms, item.run_id.clone()))
        .collect();

    // Also consider finished log files not present in the in-memory list.
    for file in &files {
        if file.running {
            continue;
        }
        if finished_ids.iter().any(|(_, id)| id == &file.run_id) {
            continue;
        }
        let ended = file
            .modified
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        finished_ids.push((ended, file.run_id.clone()));
    }
    finished_ids.sort_by_key(|(ended, _)| *ended);

    let mut remaining = total;
    let mut seen = std::collections::HashSet::new();
    for (_, id) in finished_ids {
        if remaining <= cap {
            break;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        let size: u64 = files
            .iter()
            .filter(|f| f.run_id == id)
            .map(|f| f.len)
            .sum();
        delete_run_logs(&id);
        remaining = remaining.saturating_sub(size);
    }
}

pub fn delete_run_logs(run_id: &str) {
    for path in [
        run_log_path(run_id),
        run_log_rotated_path(run_id),
        run_system_log_path(run_id),
    ] {
        let _ = fs::remove_file(path);
    }
}
