//! GitHub Releases updater. Checks and installs only in release builds,
//! and never installs without an explicit click.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::shutdown::Shutdown;

const GITHUB_LATEST: &str = "https://api.github.com/repos/pfrancug/orbitry/releases/latest";
const USER_AGENT: &str = concat!("orbitry/", env!("CARGO_PKG_VERSION"));
const CHECKSUMS_NAME: &str = "SHA256SUMS.txt";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const API_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub notes_url: Option<String>,
    pub notes: Option<String>,
    pub checking: bool,
    pub installing: bool,
    pub error: Option<String>,
    pub auto_check: bool,
    pub dev: bool,
}

impl VersionInfo {
    fn fresh(auto_check: bool) -> Self {
        Self {
            current: current_version_label(),
            latest: None,
            update_available: false,
            notes_url: None,
            notes: None,
            checking: false,
            installing: false,
            error: None,
            auto_check,
            dev: is_dev_build(),
        }
    }
}

#[derive(Clone)]
pub struct UpdateService {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    info: VersionInfo,
    asset_url: Option<String>,
    asset_name: Option<String>,
    checksums_url: Option<String>,
}

impl UpdateService {
    pub fn new(auto_check: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                info: VersionInfo::fresh(auto_check),
                asset_url: None,
                asset_name: None,
                checksums_url: None,
            })),
        }
    }

    pub fn snapshot(&self, auto_check: bool) -> VersionInfo {
        let mut info = self
            .inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .info
            .clone();
        info.auto_check = auto_check;
        info.dev = is_dev_build();
        if info.current.is_empty() {
            info.current = current_version_label();
        }
        info
    }

    pub async fn check(&self) -> Result<VersionInfo, String> {
        if is_dev_build() {
            return Ok(self.snapshot(true));
        }

        {
            let mut inner = self.lock();
            inner.info.checking = true;
            inner.info.error = None;
        }

        let result = fetch_latest().await;

        let mut inner = self.lock();
        inner.info.checking = false;
        match result {
            Ok(release) => {
                let latest = normalize_tag(&release.tag_name);
                let available = is_newer(&latest, env!("CARGO_PKG_VERSION"));
                let asset_name = platform_asset().map(str::to_string);
                let asset = asset_name
                    .as_deref()
                    .and_then(|name| release.assets.iter().find(|asset| asset.name == name));
                let checksums = release
                    .assets
                    .iter()
                    .find(|asset| asset.name == CHECKSUMS_NAME);

                inner.info.latest = Some(format!("v{latest}"));
                inner.info.update_available = available;
                inner.info.notes_url = Some(release.html_url);
                inner.info.notes = release.body.map(truncate_notes);
                inner.info.error = None;
                if available {
                    if let Some(asset) = asset {
                        inner.asset_url = Some(asset.browser_download_url.clone());
                        inner.asset_name = Some(asset.name.clone());
                    } else {
                        inner.asset_url = None;
                        inner.asset_name = None;
                        inner.info.update_available = false;
                        inner.info.error = Some(format!(
                            "A newer version is available, but no build for this system was attached (wanted {}).",
                            asset_name.unwrap_or_else(|| "this platform".into())
                        ));
                    }
                    inner.checksums_url = checksums.map(|asset| asset.browser_download_url.clone());
                    if inner.info.update_available && inner.checksums_url.is_none() {
                        inner.asset_url = None;
                        inner.info.update_available = false;
                        inner.info.error = Some(
                            "A newer version is available, but the release is missing checksums."
                                .into(),
                        );
                    }
                } else {
                    inner.asset_url = None;
                    inner.asset_name = None;
                    inner.checksums_url = None;
                }
                Ok(inner.info.clone())
            }
            Err(err) if err == "not-found" => {
                inner.info.latest = None;
                inner.info.update_available = false;
                inner.info.error = None;
                inner.asset_url = None;
                inner.asset_name = None;
                inner.checksums_url = None;
                Ok(inner.info.clone())
            }
            Err(err) => {
                inner.info.error = Some(err.clone());
                Err(err)
            }
        }
    }

    pub async fn install(&self, shutdown: Shutdown, port: u16) -> Result<(), String> {
        if is_dev_build() {
            return Err("Updates are disabled in development builds.".into());
        }
        {
            let mut inner = self.lock();
            if inner.info.installing {
                return Err("An update is already in progress.".into());
            }
            inner.info.installing = true;
            inner.info.error = None;
        }

        let (mut asset_url, mut asset_name, mut checksums_url) = {
            let inner = self.lock();
            (
                inner.asset_url.clone(),
                inner.asset_name.clone(),
                inner.checksums_url.clone(),
            )
        };

        let result = async {
            if asset_url.is_none() || asset_name.is_none() || checksums_url.is_none() {
                self.check().await?;
                let (next_url, next_name, next_sums, missing) = {
                    let inner = self.lock();
                    (
                        inner.asset_url.clone(),
                        inner.asset_name.clone(),
                        inner.checksums_url.clone(),
                        inner.info.error.clone(),
                    )
                };
                asset_url = next_url;
                asset_name = next_name;
                checksums_url = next_sums;
                if asset_url.is_none() || asset_name.is_none() || checksums_url.is_none() {
                    return Err(missing.unwrap_or_else(|| {
                        "No update is available for this system.".into()
                    }));
                }
            }
            download_and_replace(
                asset_url.as_deref().unwrap(),
                asset_name.as_deref().unwrap(),
                checksums_url.as_deref().unwrap(),
                port,
            )
            .await
        }
        .await;

        match result {
            Ok(()) => {
                shutdown.request();
                Ok(())
            }
            Err(err) => {
                let mut inner = self.lock();
                inner.info.installing = false;
                inner.info.error = Some(err.clone());
                Err(err)
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

pub fn is_dev_build() -> bool {
    cfg!(debug_assertions)
}

pub fn current_version_label() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION").trim_start_matches('v'))
}

pub fn cleanup_stale_exe() {
    if let Some(path) = stale_exe_path() {
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
}

fn stale_exe_path() -> Option<PathBuf> {
    let exe = current_exe().ok()?;
    Some(stale_path_for(&exe))
}

fn current_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|err| format!("current exe: {err}"))?;
    Ok(fs::canonicalize(&exe).unwrap_or(exe))
}

fn stale_path_for(exe: &Path) -> PathBuf {
    let stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("orbitry");
    let ext = exe.extension().and_then(|s| s.to_str());
    match ext {
        Some(ext) => exe.with_file_name(format!("{stem}.old.{ext}")),
        None => exe.with_file_name(format!("{stem}.old")),
    }
}

fn platform_asset() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("orbitry-windows-x64.exe")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("orbitry-macos-aarch64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("orbitry-macos-x64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("orbitry-linux-x64")
    } else {
        None
    }
}

fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_string()
}

fn parse_semver(value: &str) -> Option<(u64, u64, u64)> {
    let normalized = normalize_tag(value);
    let core = normalized
        .split_once('-')
        .map(|(core, _)| core)
        .unwrap_or(&normalized);
    let core = core.split_once('+').map(|(core, _)| core).unwrap_or(core);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn truncate_notes(body: String) -> String {
    const LIMIT: usize = 2000;
    let trimmed = body.trim();
    if trimmed.chars().count() <= LIMIT {
        trimmed.to_string()
    } else {
        let shortened: String = trimmed.chars().take(LIMIT).collect();
        format!("{shortened}…")
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

async fn fetch_latest() -> Result<GithubRelease, String> {
    let client = api_client(API_TIMEOUT)?;
    let response = client
        .get(GITHUB_LATEST)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|err| format!("Could not reach GitHub: {err}"))?;
    let status = response.status();
    if status.as_u16() == 404 {
        return Err("not-found".into());
    }
    if !status.is_success() {
        return Err(format!("GitHub returned {status} while checking for updates."));
    }
    response
        .json()
        .await
        .map_err(|err| format!("Could not read the GitHub release: {err}"))
}

fn api_client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .build()
        .map_err(|err| format!("http client: {err}"))
}

async fn download_and_replace(
    asset_url: &str,
    asset_name: &str,
    checksums_url: &str,
    port: u16,
) -> Result<(), String> {
    let client = api_client(DOWNLOAD_TIMEOUT)?;
    let bytes = download(&client, asset_url).await?;
    let checksums = download_text(&client, checksums_url).await?;
    let expected = parse_checksum(&checksums, asset_name).ok_or_else(|| {
        format!("The checksums file does not list {asset_name}.")
    })?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(
            "The downloaded file did not match its checksum. The update was not installed.".into(),
        );
    }

    let exe = current_exe()?;
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!("{asset_name}.download"));
    write_atomic(&tmp, &bytes)?;

    let replaced = replace_exe(&exe, &tmp)?;
    let _ = fs::remove_file(&tmp);
    spawn_replacement(&replaced, port)?;
    Ok(())
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("download failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("download failed: GitHub returned {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("download failed: {err}"))?;
    Ok(bytes.to_vec())
}

async fn download_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let bytes = download(client, url).await?;
    String::from_utf8(bytes).map_err(|err| format!("checksums file is not text: {err}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_checksum(text: &str, asset: &str) -> Option<String> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash, name) = split_checksum_line(line)?;
        let name = name
            .trim()
            .trim_start_matches('*')
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(name);
        if name == asset {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

fn split_checksum_line(line: &str) -> Option<(&str, &str)> {
    if let Some((hash, name)) = line.split_once("  ") {
        return Some((hash.trim(), name.trim()));
    }
    if let Some((hash, name)) = line.split_once(" *") {
        return Some((hash.trim(), name.trim()));
    }
    let mut parts = line.split_whitespace();
    Some((parts.next()?, parts.next()?))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::File::create(path)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    Ok(())
}

fn replace_exe(exe: &Path, new_file: &Path) -> Result<PathBuf, String> {
    let old = stale_path_for(exe);
    if old.exists() {
        let _ = fs::remove_file(&old);
    }
    fs::rename(exe, &old).map_err(|err| {
        format!(
            "could not move the running app aside ({}): {err}",
            exe.display()
        )
    })?;
    if let Err(err) = fs::copy(new_file, exe) {
        let _ = fs::rename(&old, exe);
        return Err(format!(
            "could not write the new app ({}): {err}",
            exe.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(exe) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(exe, perms);
        }
    }
    Ok(exe.to_path_buf())
}

fn spawn_replacement(exe: &Path, port: u16) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--replace-wait")
        .arg(std::process::id().to_string())
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map_err(|err| format!("could not start the new Orbitry process: {err}"))?;
    Ok(())
}
