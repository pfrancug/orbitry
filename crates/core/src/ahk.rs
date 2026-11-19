//! Optional AutoHotkey plugin: detect the Windows interpreter and top-level `.ahk` scripts.
//!
//! On non-Windows hosts the plugin is always unavailable ("Windows only") and
//! no AHK files are scanned or spawned.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{display_path, Config};
use crate::scripts::{DetectedScript, ScriptSource};
use crate::ScriptKind;
use crate::ScriptScope;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AhkStatus {
    /// Whether the plugin toggle is on in config.
    pub enabled: bool,
    /// Whether AHK can actually be used (Windows + interpreter found + enabled).
    pub available: bool,
    pub windows: bool,
    pub installed: bool,
    pub label: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Always includes v1 and v2 entries with installed/path status (from 0.14.1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interpreters: Vec<AhkInterpreter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AhkInterpreter {
    pub version: String,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhkVersion {
    V1,
    V2,
}

impl AhkVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::V1 => "AutoHotkey v1",
            Self::V2 => "AutoHotkey v2",
        }
    }
}

pub fn ahk_status(config: &Config) -> AhkStatus {
    let enabled = config.ahk_plugin;
    #[cfg(windows)]
    {
        let found = detect_interpreters();
        let interpreters = version_entries(&found);
        let installed = interpreters.iter().any(|item| item.installed);
        let available = enabled && installed;
        let (state, label, reason) = if !enabled {
            (
                "disabled",
                "disabled".to_string(),
                Some("Turned off in Settings.".to_string()),
            )
        } else if !installed {
            (
                "missing",
                "not installed".to_string(),
                Some("AutoHotkey was not found (registry or Program Files).".to_string()),
            )
        } else {
            let summary = interpreters
                .iter()
                .filter(|item| item.installed)
                .map(|item| item.version.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            ("available", format!("available · {summary}"), None)
        };
        AhkStatus {
            enabled,
            available,
            windows: true,
            installed,
            label,
            state: state.to_string(),
            reason,
            interpreters,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        AhkStatus {
            enabled,
            available: false,
            windows: false,
            installed: false,
            label: "Windows only".to_string(),
            state: "windows-only".to_string(),
            reason: Some("AutoHotkey runs on Windows only.".to_string()),
            interpreters: version_entries(&[]),
        }
    }
}

/// True when the AHK plugin should contribute scripts during a scan.
pub fn ahk_active(config: &Config) -> bool {
    ahk_status(config).available
}

pub fn dir_has_ahk(dir: &Path) -> bool {
    list_ahk_files(dir).next().is_some()
}

pub fn detect_ahk_scripts(dir: &Path, existing_names: &[String]) -> Vec<DetectedScript> {
    let found = detect_interpreters();
    let mut taken: std::collections::BTreeSet<String> = existing_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut out = Vec::new();
    for path in list_ahk_files(dir) {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let key = stem.to_ascii_lowercase();
        if taken.contains(&key) {
            continue;
        }
        taken.insert(key);
        let version = parse_requires(&path).unwrap_or(AhkVersion::V2);
        let version_ok = found.iter().any(|(ver, _)| *ver == version);
        let blocked_reason = if version_ok {
            None
        } else {
            Some(format!(
                "{} is not installed. Install it to run this script.",
                version.label()
            ))
        };
        out.push(DetectedScript {
            name: stem.to_string(),
            command: display_path(&path),
            kind: ScriptKind::Other,
            scope: ScriptScope::Shared,
            cwd: Some(display_path(dir)),
            source: ScriptSource::Ahk,
            source_name: Some(version.label().to_string()),
            label: None,
            ahk_version: Some(version.as_str().to_string()),
            blocked_reason,
        });
    }
    out.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    out
}

/// Resolve the interpreter for an exact major version. No cross-major fallback.
pub fn resolve_interpreter(version: AhkVersion) -> Option<PathBuf> {
    detect_interpreters()
        .into_iter()
        .find(|(ver, _)| *ver == version)
        .map(|(_, path)| path)
}

pub fn missing_version_message(version: AhkVersion) -> String {
    format!(
        "{} is not installed. Install it to run this script, or turn the AutoHotkey plugin off.",
        version.label()
    )
}

pub fn parse_requires(path: &Path) -> Option<AhkVersion> {
    let raw = fs::read_to_string(path).ok()?;
    for line in raw.lines().take(40) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with(';') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if !lower.starts_with("#requires") {
            continue;
        }
        if lower.contains("autohotkey") {
            if lower.contains("v1") || lower.contains("1.") {
                return Some(AhkVersion::V1);
            }
            if lower.contains("v2") || lower.contains("2.") {
                return Some(AhkVersion::V2);
            }
        }
    }
    None
}

fn version_entries(found: &[(AhkVersion, PathBuf)]) -> Vec<AhkInterpreter> {
    [AhkVersion::V1, AhkVersion::V2]
        .into_iter()
        .map(|version| match found.iter().find(|(ver, _)| *ver == version) {
            Some((_, path)) => AhkInterpreter {
                version: version.as_str().to_string(),
                installed: true,
                path: Some(display_path(path)),
            },
            None => AhkInterpreter {
                version: version.as_str().to_string(),
                installed: false,
                path: None,
            },
        })
        .collect()
}

fn list_ahk_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files.into_iter();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_ahk = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ahk"));
        if is_ahk {
            files.push(path);
        }
    }
    files.sort();
    files.into_iter()
}

#[cfg(windows)]
fn detect_interpreters() -> Vec<(AhkVersion, PathBuf)> {
    let mut found: Vec<(AhkVersion, PathBuf)> = Vec::new();
    let mut push = |version: AhkVersion, path: PathBuf| {
        if !path.is_file() {
            return;
        }
        if found
            .iter()
            .any(|(ver, existing)| *ver == version && existing == &path)
        {
            return;
        }
        if found.iter().any(|(ver, _)| *ver == version) {
            return;
        }
        found.push((version, path));
    };

    for (version, path) in registry_installs() {
        push(version, path);
    }

    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));

    for root in [&program_files, &program_files_x86] {
        push(
            AhkVersion::V2,
            root.join("AutoHotkey").join("v2").join("AutoHotkey64.exe"),
        );
        push(
            AhkVersion::V2,
            root.join("AutoHotkey").join("v2").join("AutoHotkey.exe"),
        );
        push(
            AhkVersion::V1,
            root.join("AutoHotkey").join("AutoHotkeyU64.exe"),
        );
        push(
            AhkVersion::V1,
            root.join("AutoHotkey").join("AutoHotkey.exe"),
        );
    }

    found.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    found
}

#[cfg(not(windows))]
fn detect_interpreters() -> Vec<(AhkVersion, PathBuf)> {
    Vec::new()
}

#[cfg(windows)]
fn registry_installs() -> Vec<(AhkVersion, PathBuf)> {
    use std::ptr;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, KEY_WOW64_64KEY, REG_EXPAND_SZ, REG_SZ,
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn read_install_dir(root: HKEY, subkey: &str) -> Option<PathBuf> {
        let subkey_w = wide(subkey);
        let mut hkey: HKEY = ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                root,
                subkey_w.as_ptr(),
                0,
                KEY_READ | KEY_WOW64_64KEY,
                &mut hkey,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let name = wide("InstallDir");
        let mut ty = 0u32;
        let mut size = 0u32;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                name.as_ptr(),
                ptr::null_mut(),
                &mut ty,
                ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS || size == 0 {
            unsafe { RegCloseKey(hkey) };
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let mut size = size;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                name.as_ptr(),
                ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr(),
                &mut size,
            )
        };
        unsafe { RegCloseKey(hkey) };
        if status != ERROR_SUCCESS {
            return None;
        }
        if ty != REG_SZ && ty != REG_EXPAND_SZ {
            return None;
        }
        let units = (size as usize) / 2;
        let chars = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u16>(), units) };
        let end = chars.iter().position(|&unit| unit == 0).unwrap_or(chars.len());
        let value = String::from_utf16_lossy(&chars[..end]);
        let trimmed = value.trim().trim_matches('"');
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    }

    let mut out = Vec::new();
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if let Some(dir) = read_install_dir(root, r"Software\AutoHotkey") {
            let v2 = dir.join("v2").join("AutoHotkey64.exe");
            if v2.is_file() {
                out.push((AhkVersion::V2, v2));
            } else {
                let v2b = dir.join("v2").join("AutoHotkey.exe");
                if v2b.is_file() {
                    out.push((AhkVersion::V2, v2b));
                }
            }
            let v1 = dir.join("AutoHotkeyU64.exe");
            if v1.is_file() {
                out.push((AhkVersion::V1, v1));
            } else {
                let v1b = dir.join("AutoHotkey.exe");
                if v1b.is_file() {
                    out.push((AhkVersion::V1, v1b));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_defaults_to_v2_when_absent() {
        assert!(parse_requires(Path::new("does-not-exist.ahk")).is_none());
    }

    #[test]
    fn version_entries_always_list_v1_and_v2() {
        let entries = version_entries(&[]);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, "v1");
        assert!(!entries[0].installed);
        assert_eq!(entries[1].version, "v2");
        assert!(!entries[1].installed);
    }
}
