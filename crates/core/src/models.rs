//! Local AI model inventory: Ollama, common caches, and weight files in project roots.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{
    canonical, config_path, display_path, normalize_path_str, paths_equal, Config,
};
use crate::git::find_git_root;
use crate::group::RepoGroup;
use crate::scan_cache::now_ms;

/// Bump when the inventory shape or detection rules change.
pub const MODELS_SCANNER_VERSION: u32 = 5;

const OLLAMA_PORT: u16 = 11434;
const OLLAMA_HOST: [u8; 4] = [127, 0, 0, 1];
const HTTP_TIMEOUT: Duration = Duration::from_millis(800);
const BIN_MIN_BYTES: u64 = 2 * 1024 * 1024;
const ATTRIB_MAX_FILE_BYTES: u64 = 512 * 1024;
const ATTRIB_MAX_FILES: usize = 4000;

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".venv",
    "venv",
    ".git",
    "dist",
    "target",
    "__pycache__",
    ".cache",
    ".svelte-kit",
    ".next",
    ".output",
    ".nuxt",
    "coverage",
    ".turbo",
    ".pnpm-store",
    "backups",
    "backup",
    "archive",
    "archives",
    "datasets",
    "dataset",
    ".huggingface",
    "huggingface",
    ".ollama",
    "lost+found",
];

const ATTRIB_SKIP_DIRS: &[&str] = &[
    "data",
    "datasets",
    "dataset",
    "outputs",
    "output",
    "logs",
    "tmp",
    "temp",
    "build",
    "vendor",
];

const WEIGHT_EXTS: &[&str] = &[
    "gguf", "ggml", "onnx", "safetensors", "pt", "pth", "ckpt", "joblib", "tflite",
];
const LARGE_ONLY_EXTS: &[&str] = &["bin", "pkl"];

const TEXT_EXTS: &[&str] = &[
    "py", "pyi", "ipynb", "ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "jsonc", "toml",
    "yml", "yaml", "ini", "cfg", "conf", "env", "md", "rs", "go", "java", "kt", "cs", "rb",
    "php", "sh", "bash", "zsh", "ps1", "bat", "cmd", "svelte", "vue", "txt", "xml", "sql",
    "r", "jl", "lua", "plist",
];

const GENERIC_NEEDLES: &[&str] = &[
    "model", "models", "config", "weights", "cache", "hub", "main", "data", "default",
    "test", "vocab", "merges", "index", "license",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelsScanState {
    #[default]
    Idle,
    Scanning,
    Fresh,
    Cached,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsScanStatus {
    pub state: ModelsScanState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scanned_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub stale: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModel {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_by: Vec<ModelUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaInventory {
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    pub service_url: String,
    pub service_reachable: bool,
    pub models_dir: String,
    pub models_dir_bytes: u64,
    /// "api" when listed from the running service, "manifests" when read from disk.
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<OllamaModel>,
}

impl Default for OllamaInventory {
    fn default() -> Self {
        Self {
            installed: false,
            version: None,
            install_path: None,
            service_url: format!("http://127.0.0.1:{OLLAMA_PORT}"),
            service_reachable: false,
            models_dir: String::new(),
            models_dir_bytes: 0,
            source: "manifests".into(),
            models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedModel {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_by: Vec<ModelUsage>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSource {
    pub id: String,
    pub label: String,
    pub path: String,
    pub present: bool,
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<CachedModel>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightFile {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_by: Vec<ModelUsage>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWeights {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<WeightFile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTotal {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInventory {
    pub scan: ModelsScanStatus,
    pub ollama: OllamaInventory,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caches: Vec<CacheSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_files: Vec<ProjectWeights>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<SourceTotal>,
    pub grand_total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub scanner_version: u32,
}

pub fn models_cache_path() -> PathBuf {
    config_path().with_file_name("models-cache.json")
}

pub fn models_index_path() -> PathBuf {
    config_path().with_file_name("models-index.json")
}

pub fn models_fingerprint(config: &Config) -> String {
    let mut roots: Vec<String> = config
        .roots
        .iter()
        .map(|root| normalize_path_str(root).to_ascii_lowercase())
        .collect();
    roots.sort();
    roots.dedup();
    serde_json::to_string(&serde_json::json!({ "roots": roots })).unwrap_or_default()
}

pub fn load_models_cache(path: &Path, config: &Config) -> Option<ModelsInventory> {
    let raw = fs::read(path).ok()?;
    let mut inventory: ModelsInventory = serde_json::from_slice(&raw).ok()?;
    if inventory.scanner_version != MODELS_SCANNER_VERSION {
        return None;
    }
    let current = models_fingerprint(config);
    inventory.scan.stale = inventory.fingerprint.as_deref() != Some(current.as_str());
    inventory.scan.state = ModelsScanState::Cached;
    inventory.scan.error = None;
    Some(inventory)
}

pub fn save_models_cache(path: &Path, inventory: &ModelsInventory) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("models cache dir: {err}"))?;
    }
    let json =
        serde_json::to_vec_pretty(inventory).map_err(|err| format!("models cache encode: {err}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|err| format!("models cache write: {err}"))?;
    fs::rename(&tmp, path).map_err(|err| format!("models cache rename: {err}"))?;
    Ok(())
}

pub fn empty_models_inventory() -> ModelsInventory {
    ModelsInventory {
        scan: ModelsScanStatus {
            state: ModelsScanState::Idle,
            ..ModelsScanStatus::default()
        },
        scanner_version: MODELS_SCANNER_VERSION,
        ..ModelsInventory::default()
    }
}

/// Full inventory. Intended for a background thread, never the request path.
pub fn scan_models(config: &Config, groups: &[RepoGroup]) -> ModelsInventory {
    scan_models_with_sink(config, groups, |_| {})
}

/// Same as [`scan_models`], but publishes partial inventories (Ollama and caches
/// first, then project weights) so the UI is not empty for the whole walk.
pub fn scan_models_with_sink<F>(config: &Config, groups: &[RepoGroup], mut sink: F) -> ModelsInventory
where
    F: FnMut(&ModelsInventory),
{
    let started_at = now_ms();
    let ollama = scan_ollama();
    let caches = scan_caches();
    let partial = assemble_inventory(
        config,
        ollama.clone(),
        caches.clone(),
        Vec::new(),
        ModelsScanState::Scanning,
        None,
        Some(started_at),
        None,
    );
    sink(&partial);

    let mut index = load_models_index(&models_index_path(), config);
    let mut ollama = ollama;
    let mut caches = caches;
    let mut project_files = scan_project_weights(&config.roots, groups, &mut index);
    let mid = assemble_inventory(
        config,
        ollama.clone(),
        caches.clone(),
        project_files.clone(),
        ModelsScanState::Scanning,
        None,
        Some(started_at),
        None,
    );
    sink(&mid);

    attribute_usages(&mut ollama, &mut caches, &mut project_files, &config.roots, groups, &mut index);
    let _ = save_models_index(&models_index_path(), &index);
    assemble_inventory(
        config,
        ollama,
        caches,
        project_files,
        ModelsScanState::Fresh,
        Some(now_ms()),
        Some(started_at),
        None,
    )
}

fn assemble_inventory(
    config: &Config,
    ollama: OllamaInventory,
    caches: Vec<CacheSource>,
    project_files: Vec<ProjectWeights>,
    state: ModelsScanState,
    scanned_at: Option<i64>,
    started_at: Option<i64>,
    error: Option<String>,
) -> ModelsInventory {
    let mut totals = Vec::new();
    if !ollama.models_dir.is_empty() || ollama.installed || !ollama.models.is_empty() {
        totals.push(SourceTotal {
            id: "ollama".into(),
            label: "Ollama".into(),
            path: Some(ollama.models_dir.clone()),
            bytes: ollama.models_dir_bytes,
        });
    }
    for cache in &caches {
        if cache.present || cache.total_bytes > 0 {
            totals.push(SourceTotal {
                id: cache.id.clone(),
                label: cache.label.clone(),
                path: Some(cache.path.clone()),
                bytes: cache.total_bytes,
            });
        }
    }
    for project in &project_files {
        totals.push(SourceTotal {
            id: format!("project:{}", project.project_id),
            label: project.project_name.clone(),
            path: Some(project.project_path.clone()),
            bytes: project.total_bytes,
        });
    }
    let grand_total_bytes = totals.iter().map(|item| item.bytes).sum();
    ModelsInventory {
        scan: ModelsScanStatus {
            state,
            scanned_at,
            started_at,
            error,
            stale: false,
        },
        ollama,
        caches,
        project_files,
        totals,
        grand_total_bytes,
        fingerprint: Some(models_fingerprint(config)),
        scanner_version: MODELS_SCANNER_VERSION,
    }
}

fn scan_ollama() -> OllamaInventory {
    let models_path = ollama_models_dir();
    let models_dir = display_path(&models_path);
    let models_dir_bytes = if models_path.is_dir() {
        dir_size(&models_path)
    } else {
        0
    };
    let (installed, install_path) = detect_ollama_install();
    let mut version = version_from_metadata(&models_path);
    let mut inventory = OllamaInventory {
        installed: installed || models_path.is_dir(),
        version: version.clone(),
        install_path,
        service_url: format!("http://127.0.0.1:{OLLAMA_PORT}"),
        service_reachable: false,
        models_dir,
        models_dir_bytes,
        source: "manifests".into(),
        models: Vec::new(),
    };

    if let Some(api) = query_ollama_api() {
        inventory.service_reachable = true;
        inventory.source = "api".into();
        if let Some(api_version) = api.version {
            version = Some(api_version);
            inventory.version = version;
        }
        inventory.models = api.models;
        return inventory;
    }

    inventory.models = read_ollama_manifests(&models_path);
    inventory
}

struct OllamaApi {
    version: Option<String>,
    models: Vec<OllamaModel>,
}

fn query_ollama_api() -> Option<OllamaApi> {
    let version = http_get_json("/api/version")
        .ok()
        .and_then(|value| {
            value
                .get("version")
                .and_then(|item| item.as_str())
                .map(|item| item.to_string())
        });
    let tags = http_get_json("/api/tags").ok()?;
    let models = tags
        .get("models")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(ollama_model_from_api)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // A successful /api/tags (even with an empty list) counts as reachable.
    Some(OllamaApi { version, models })
}

fn ollama_model_from_api(value: &serde_json::Value) -> Option<OllamaModel> {
    let name = value
        .get("name")
        .or_else(|| value.get("model"))
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let digest = value
        .get("digest")
        .and_then(|item| item.as_str())
        .and_then(short_digest);
    let size_bytes = value.get("size").and_then(|item| item.as_u64()).unwrap_or(0);
    let modified_at = value
        .get("modified_at")
        .or_else(|| value.get("modifiedAt"))
        .and_then(|item| item.as_str())
        .and_then(parse_rfc3339_ms);
    Some(OllamaModel {
        name,
        id: digest,
        size_bytes,
        modified_at,
        used_by: Vec::new(),
    })
}

fn read_ollama_manifests(models_dir: &Path) -> Vec<OllamaModel> {
    let mut by_name: BTreeMap<String, OllamaModel> = BTreeMap::new();
    // Prefer the v1 manifests tree: it carries layer sizes and the short id.
    collect_manifest_tree(models_dir.join("manifests"), &mut by_name, true);
    collect_manifest_tree(models_dir.join("manifests-v2"), &mut by_name, false);
    by_name.into_values().collect()
}

fn collect_manifest_tree(root: PathBuf, into: &mut BTreeMap<String, OllamaModel>, overwrite: bool) {
    if !root.is_dir() {
        return;
    }
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let Some(name) = ollama_name_from_manifest_path(&root, &path) else {
                continue;
            };
            if !overwrite && into.contains_key(&name) {
                continue;
            }
            let Some(parsed) = parse_ollama_manifest(&path, &name, meta_mtime_ms(&meta)) else {
                continue;
            };
            if let Some(existing) = into.get(&name) {
                // Keep the entry that actually has a size / id if the new one does not.
                if existing.size_bytes > 0 && parsed.size_bytes == 0 {
                    continue;
                }
            }
            into.insert(name, parsed);
        }
    }
}

fn ollama_name_from_manifest_path(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let mut parts: Vec<String> = rel
        .iter()
        .map(|comp| comp.to_string_lossy().to_string())
        .collect();
    if parts.is_empty() {
        return None;
    }
    // First component is the registry host (registry.ollama.ai / ollama.com).
    if parts.len() < 3 {
        return None;
    }
    parts.remove(0);
    let tag = parts.pop()?;
    if parts.is_empty() {
        return None;
    }
    let library = parts[0].eq_ignore_ascii_case("library");
    let model = if library {
        parts[1..].join("/")
    } else {
        parts.join("/")
    };
    if model.is_empty() || tag.is_empty() {
        return None;
    }
    Some(format!("{model}:{tag}"))
}

fn parse_ollama_manifest(path: &Path, name: &str, modified_at: Option<i64>) -> Option<OllamaModel> {
    let raw = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let mut size_bytes = 0_u64;
    let mut id = None;
    if let Some(layers) = value.get("layers").and_then(|item| item.as_array()) {
        for layer in layers {
            let media = layer
                .get("mediaType")
                .and_then(|item| item.as_str())
                .unwrap_or("");
            let digest = layer.get("digest").and_then(|item| item.as_str());
            if media == "application/vnd.docker.distribution.manifest.v2+json" {
                if id.is_none() {
                    id = digest.and_then(short_digest);
                }
                continue;
            }
            if media.contains("manifest") {
                continue;
            }
            size_bytes += layer.get("size").and_then(|item| item.as_u64()).unwrap_or(0);
        }
    }
    if let Some(manifests) = value.get("manifests").and_then(|item| item.as_array()) {
        // v2 list: pick the llamacpp digest as the public id; sizes live in the v1 tree.
        if id.is_none() {
            let preferred = manifests.iter().find(|item| {
                item.get("runner")
                    .and_then(|runner| runner.as_str())
                    .is_some_and(|runner| runner.eq_ignore_ascii_case("llamacpp"))
            });
            let digest = preferred
                .or_else(|| manifests.first())
                .and_then(|item| item.get("digest"))
                .and_then(|item| item.as_str());
            id = digest.and_then(short_digest);
        }
    }
    if id.is_none() {
        id = value
            .pointer("/config/digest")
            .and_then(|item| item.as_str())
            .and_then(short_digest);
    }
    Some(OllamaModel {
        name: name.to_string(),
        id,
        size_bytes,
        modified_at,
        used_by: Vec::new(),
    })
}

fn short_digest(digest: &str) -> Option<String> {
    let hex = digest
        .strip_prefix("sha256:")
        .or_else(|| digest.strip_prefix("SHA256:"))
        .unwrap_or(digest)
        .trim();
    if hex.len() < 12 {
        return None;
    }
    Some(hex[..12].to_string())
}

fn version_from_metadata(models_dir: &Path) -> Option<String> {
    let dir = models_dir.join("metadata");
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(version) = value
            .get("ollama_version")
            .and_then(|item| item.as_str())
            .filter(|item| !item.is_empty())
        {
            return Some(version.to_string());
        }
    }
    None
}

fn ollama_models_dir() -> PathBuf {
    if let Some(path) = env_path("OLLAMA_MODELS") {
        return path;
    }
    if let Some(home) = env_path("OLLAMA_HOME") {
        return home.join("models");
    }
    home_dir()
        .map(|home| home.join(".ollama").join("models"))
        .unwrap_or_else(|| PathBuf::from(".ollama/models"))
}

fn detect_ollama_install() -> (bool, Option<String>) {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join("Programs").join("Ollama").join("ollama.exe"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            candidates.push(PathBuf::from(program_files).join("Ollama").join("ollama.exe"));
        }
        if let Some(home) = home_dir() {
            candidates.push(
                home.join("AppData")
                    .join("Local")
                    .join("Programs")
                    .join("Ollama")
                    .join("ollama.exe"),
            );
        }
    } else if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/Applications/Ollama.app"));
        candidates.push(PathBuf::from("/usr/local/bin/ollama"));
        candidates.push(PathBuf::from("/opt/homebrew/bin/ollama"));
    } else {
        candidates.push(PathBuf::from("/usr/local/bin/ollama"));
        candidates.push(PathBuf::from("/usr/bin/ollama"));
        if let Some(home) = home_dir() {
            candidates.push(home.join(".local").join("bin").join("ollama"));
        }
    }
    for dir in path_dirs() {
        if cfg!(windows) {
            candidates.push(dir.join("ollama.exe"));
        } else {
            candidates.push(dir.join("ollama"));
        }
    }
    for path in candidates {
        if path.exists() {
            return (true, Some(display_path(&path)));
        }
    }
    (false, None)
}

fn scan_caches() -> Vec<CacheSource> {
    vec![
        scan_hf_hub(),
        scan_named_cache(
            "torch",
            "torch hub",
            unix_cache_home().map(|home| home.join("torch").join("hub")),
            CacheListMode::WeightFiles,
        ),
        scan_named_cache(
            "insightface",
            "InsightFace",
            home_dir().map(|home| home.join(".insightface").join("models")),
            CacheListMode::Packs,
        ),
        scan_named_cache(
            "whisper",
            "Whisper",
            whisper_cache_dir(),
            CacheListMode::WeightFiles,
        ),
    ]
}

enum CacheListMode {
    WeightFiles,
    Packs,
}

fn scan_hf_hub() -> CacheSource {
    let path = hf_hub_dir();
    let mut source = CacheSource {
        id: "huggingface".into(),
        label: "Hugging Face".into(),
        path: path
            .as_ref()
            .map(|item| display_path(item))
            .unwrap_or_default(),
        present: path.as_ref().is_some_and(|item| item.is_dir()),
        total_bytes: 0,
        models: Vec::new(),
    };
    let Some(root) = path.filter(|item| item.is_dir()) else {
        return source;
    };
    source.total_bytes = dir_size(&root);
    let Ok(entries) = fs::read_dir(&root) else {
        return source;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        let Ok(meta) = fs::symlink_metadata(&child) else {
            continue;
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            continue;
        }
        let Some(folder) = child.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(name) = hf_repo_from_dir(folder) else {
            continue;
        };
        source.models.push(CachedModel {
            name,
            path: display_path(&child),
            size_bytes: dir_size(&child),
            used_by: Vec::new(),
        });
    }
    source
        .models
        .sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.name.cmp(&b.name)));
    source
}

fn hf_repo_from_dir(folder: &str) -> Option<String> {
    let rest = folder.strip_prefix("models--")?;
    if rest.is_empty() {
        return None;
    }
    Some(rest.replace("--", "/"))
}

fn hf_hub_dir() -> Option<PathBuf> {
    if let Some(path) = env_path("HF_HUB_CACHE") {
        return Some(path);
    }
    if let Some(home) = env_path("HF_HOME") {
        return Some(home.join("hub"));
    }
    unix_cache_home().map(|home| home.join("huggingface").join("hub"))
}

fn whisper_cache_dir() -> Option<PathBuf> {
    if let Some(path) = env_path("WHISPER_CACHE_DIR") {
        return Some(path);
    }
    unix_cache_home().map(|home| home.join("whisper"))
}

fn scan_named_cache(
    id: &str,
    label: &str,
    path: Option<PathBuf>,
    mode: CacheListMode,
) -> CacheSource {
    let mut source = CacheSource {
        id: id.into(),
        label: label.into(),
        path: path
            .as_ref()
            .map(|item| display_path(item))
            .unwrap_or_default(),
        present: path.as_ref().is_some_and(|item| item.is_dir()),
        total_bytes: 0,
        models: Vec::new(),
    };
    let Some(root) = path.filter(|item| item.is_dir()) else {
        return source;
    };
    source.total_bytes = dir_size(&root);
    match mode {
        CacheListMode::Packs => {
            let Ok(entries) = fs::read_dir(&root) else {
                return source;
            };
            for entry in entries.flatten() {
                let child = entry.path();
                let Ok(meta) = fs::symlink_metadata(&child) else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                let name = file_name(&child);
                if name.starts_with('.') {
                    continue;
                }
                let size = if meta.is_dir() {
                    dir_size(&child)
                } else {
                    meta.len()
                };
                source.models.push(CachedModel {
                    name,
                    path: display_path(&child),
                    size_bytes: size,
                    used_by: Vec::new(),
                });
            }
        }
        CacheListMode::WeightFiles => {
            let mut files = Vec::new();
            walk_files(&root, WalkMode::Weights, &mut files);
            for (path, size) in files {
                if !is_cache_listable(&path, size) {
                    continue;
                }
                source.models.push(CachedModel {
                    name: rel_name(&root, &path),
                    path: display_path(&path),
                    size_bytes: size,
                    used_by: Vec::new(),
                });
            }
        }
    }
    source
        .models
        .sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.name.cmp(&b.name)));
    source
}

fn is_cache_listable(path: &Path, size: u64) -> bool {
    if is_false_positive(path) {
        return false;
    }
    if has_ext(path, WEIGHT_EXTS) || (has_ext(path, LARGE_ONLY_EXTS) && size > BIN_MIN_BYTES) {
        return true;
    }
    size >= 1024 * 1024
}

fn scan_project_weights(roots: &[String], groups: &[RepoGroup], index: &mut ModelsIndex) -> Vec<ProjectWeights> {
    let mut by_project: BTreeMap<String, ProjectWeights> = BTreeMap::new();
    for root in roots {
        let root_path = PathBuf::from(root);
        if !root_path.exists() {
            continue;
        }
        let mut files = Vec::new();
        if root_path.is_file() {
            if let Ok(meta) = fs::symlink_metadata(&root_path) {
                files.push((root_path, meta.len()));
            }
        } else {
            walk_weights_indexed(&root_path, index, &mut files);
        }
        for (path, size) in files {
            if !is_weight_file(&path, size) {
                continue;
            }
            let (project_id, project_name, project_path) =
                project_for(&path, groups, roots);
            let entry = by_project
                .entry(project_id.clone())
                .or_insert_with(|| ProjectWeights {
                    project_id,
                    project_name,
                    project_path,
                    total_bytes: 0,
                    files: Vec::new(),
                });
            entry.total_bytes += size;
            entry.files.push(WeightFile {
                name: file_name(&path),
                path: display_path(&path),
                size_bytes: size,
                used_by: Vec::new(),
            });
        }
    }
    let mut projects: Vec<ProjectWeights> = by_project.into_values().collect();
    for project in &mut projects {
        project
            .files
            .sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.name.cmp(&b.name)));
    }
    projects.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes).then_with(|| a.project_name.cmp(&b.project_name)));
    projects
}

fn is_weight_file(path: &Path, size: u64) -> bool {
    if is_false_positive(path) {
        return false;
    }
    if has_ext(path, WEIGHT_EXTS) {
        return true;
    }
    has_ext(path, LARGE_ONLY_EXTS) && size > BIN_MIN_BYTES
}

fn is_false_positive(path: &Path) -> bool {
    let raw = path.to_string_lossy().to_ascii_lowercase();
    if raw.contains("tokenizer") || raw.contains("tokenisers") {
        return true;
    }
    if raw.contains("put_") && raw.contains("_here") {
        return true;
    }
    if raw.contains("placeholder") || raw.contains("put_loras_here") {
        return true;
    }
    let name = file_name(path).to_ascii_lowercase();
    matches!(name.as_str(), ".gitkeep" | "desktop.ini" | "thumbs.db")
}

fn project_for(path: &Path, groups: &[RepoGroup], roots: &[String]) -> (String, String, String) {
    let mut best: Option<(&RepoGroup, usize)> = None;
    for group in groups {
        let group_path = PathBuf::from(&group.path);
        if path_is_within(path, &group_path) {
            let len = normalize_path_str(&group.path).len();
            if best.map(|(_, existing)| len > existing).unwrap_or(true) {
                best = Some((group, len));
            }
        }
    }
    if let Some((group, _)) = best {
        return (
            group.id.clone(),
            group.name.clone(),
            display_path(Path::new(&group.path)),
        );
    }
    if let Some(git_root) = find_git_root(path) {
        let git_disp = display_path(&canonical(&git_root));
        if let Some(group) = groups.iter().find(|group| {
            paths_equal(&group.path, &git_disp) || paths_equal(&group.id, &git_disp)
        }) {
            return (
                group.id.clone(),
                group.name.clone(),
                display_path(Path::new(&group.path)),
            );
        }
        let name = file_name(&git_root);
        return (git_disp.clone(), name, git_disp);
    }
    for root in roots {
        let root_path = PathBuf::from(root);
        if !path_is_within(path, &root_path) {
            continue;
        }
        let root_c = canonical(&root_path);
        let path_c = canonical(path);
        if let Ok(rel) = path_c.strip_prefix(&root_c).or_else(|_| path.strip_prefix(&root_path)) {
            if let Some(first) = rel.iter().next() {
                let folder = root_path.join(first);
                if folder.is_dir() {
                    let folder_disp = display_path(&canonical(&folder));
                    if let Some(group) = groups.iter().find(|group| {
                        paths_equal(&group.path, &folder_disp) || paths_equal(&group.id, &folder_disp)
                    }) {
                        return (
                            group.id.clone(),
                            group.name.clone(),
                            display_path(Path::new(&group.path)),
                        );
                    }
                    let name = first.to_string_lossy().to_string();
                    return (folder_disp.clone(), name, folder_disp);
                }
            }
            let root_disp = display_path(&root_c);
            return (root_disp.clone(), file_name(&root_c), root_disp);
        }
    }
    // Last resort: nearest folder name, not every path segment.
    let parent = path.parent().unwrap_or(path);
    (display_path(parent), file_name(parent), display_path(parent))
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    // normalize_path_str rewrites separators to `\` on every platform, so the
    // containment suffix must always be `\` (not the OS native separator).
    let path_l = normalize_path_str(&display_path(&canonical(path))).to_ascii_lowercase();
    let root_l = normalize_path_str(&display_path(&canonical(root))).to_ascii_lowercase();
    if path_l == root_l {
        return true;
    }
    if root_l.is_empty() {
        return false;
    }
    path_l.starts_with(&(root_l + "\\"))
}

fn attribute_usages(
    ollama: &mut OllamaInventory,
    caches: &mut [CacheSource],
    project_files: &mut [ProjectWeights],
    roots: &[String],
    groups: &[RepoGroup],
    index: &mut ModelsIndex,
) {
    let mut needles: Vec<AttribNeedle> = Vec::new();
    for model in &ollama.models {
        push_needles(&mut needles, AttribTarget::Ollama(model.name.clone()), &model.name, None);
    }
    for cache in caches.iter() {
        for model in &cache.models {
            push_needles(
                &mut needles,
                AttribTarget::Cache {
                    source: cache.id.clone(),
                    name: model.name.clone(),
                },
                &model.name,
                Some(&model.path),
            );
        }
    }
    for project in project_files.iter() {
        for file in &project.files {
            push_needles(
                &mut needles,
                AttribTarget::ProjectFile {
                    project_id: project.project_id.clone(),
                    name: file.name.clone(),
                },
                &file.name,
                Some(&file.path),
            );
        }
    }
    if needles.is_empty() {
        return;
    }
    needles.sort_by(|a, b| b.pattern.len().cmp(&a.pattern.len()));
    let needle_fp = attrib_needle_fingerprint(&needles);
    let reuse_attrib = index.attrib_fingerprint.as_deref() == Some(needle_fp.as_str());
    let mut next_attrib: BTreeMap<String, AttribFile> = BTreeMap::new();

    let mut hits: Vec<(AttribTarget, ModelUsage)> = Vec::new();
    for root in roots {
        let root_path = PathBuf::from(root);
        if !root_path.exists() {
            continue;
        }
        let mut files = Vec::new();
        walk_files(&root_path, WalkMode::Attribution, &mut files);
        let mut read = 0_usize;
        for (path, size) in files {
            if size == 0 || size > ATTRIB_MAX_FILE_BYTES || !is_text_file(&path) {
                continue;
            }
            if read >= ATTRIB_MAX_FILES {
                break;
            }
            let key = display_path(&path);
            let mtime = fs::metadata(&path).ok().and_then(|meta| meta_mtime_ms(&meta));
            let cached = reuse_attrib
                .then(|| index.attrib_files.get(&key))
                .flatten()
                .filter(|item| item.size == size && item.mtime == mtime);
            let matched: Vec<String> = if let Some(cached) = cached {
                cached.hits.clone()
            } else {
                let Ok(raw) = fs::read_to_string(&path) else {
                    continue;
                };
                read += 1;
                let lower = raw.to_ascii_lowercase();
                needles
                    .iter()
                    .filter(|needle| lower.contains(&needle.pattern))
                    .map(|needle| needle.pattern.clone())
                    .collect()
            };
            next_attrib.insert(
                key.clone(),
                AttribFile {
                    size,
                    mtime,
                    hits: matched.clone(),
                },
            );
            if matched.is_empty() {
                continue;
            }
            let (project_id, project_name, project_path) = project_for(&path, groups, roots);
            let rel = display_path(&path);
            for needle in &needles {
                if !matched.iter().any(|item| item == &needle.pattern) {
                    continue;
                }
                hits.push((
                    needle.target.clone(),
                    ModelUsage {
                        project_id: project_id.clone(),
                        project_name: project_name.clone(),
                        project_path: project_path.clone(),
                        files: vec![rel.clone()],
                    },
                ));
            }
        }
    }
    index.attrib_fingerprint = Some(needle_fp);
    index.attrib_files = next_attrib;

    for (target, usage) in hits {
        match target {
            AttribTarget::Ollama(name) => {
                if let Some(model) = ollama.models.iter_mut().find(|item| item.name == name) {
                    merge_usage(&mut model.used_by, usage);
                }
            }
            AttribTarget::Cache { source, name } => {
                if let Some(cache) = caches.iter_mut().find(|item| item.id == source) {
                    if let Some(model) = cache.models.iter_mut().find(|item| item.name == name) {
                        merge_usage(&mut model.used_by, usage);
                    }
                }
            }
            AttribTarget::ProjectFile { project_id, name } => {
                if let Some(project) = project_files
                    .iter_mut()
                    .find(|item| item.project_id == project_id)
                {
                    if let Some(file) = project.files.iter_mut().find(|item| item.name == name) {
                        merge_usage(&mut file.used_by, usage);
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
enum AttribTarget {
    Ollama(String),
    Cache { source: String, name: String },
    ProjectFile { project_id: String, name: String },
}

struct AttribNeedle {
    pattern: String,
    target: AttribTarget,
}

fn push_needles(into: &mut Vec<AttribNeedle>, target: AttribTarget, name: &str, path: Option<&str>) {
    let mut seen = BTreeSet::new();
    let mut add = |raw: &str, target: AttribTarget| {
        let pattern = raw.trim().to_ascii_lowercase();
        if !is_useful_needle(&pattern) || !seen.insert(pattern.clone()) {
            return;
        }
        into.push(AttribNeedle { pattern, target });
    };
    add(name, target.clone());
    if let Some(slash) = name.rfind('/') {
        add(&name[slash + 1..], target.clone());
    }
    if let Some(colon) = name.rfind(':') {
        // Full "name:tag" is the primary needle; the untagged name is only used
        // when it stays distinctive.
        let untagged = &name[..colon];
        if untagged.contains('/') || untagged.len() >= 8 {
            add(untagged, target.clone());
        }
    }
    if let Some(path) = path {
        add(&file_name(Path::new(path)), target);
    }
}

fn is_useful_needle(pattern: &str) -> bool {
    if pattern.len() < 4 {
        return false;
    }
    if GENERIC_NEEDLES.iter().any(|item| *item == pattern) {
        return false;
    }
    if pattern.len() < 5 && !pattern.contains('/') && !pattern.contains(':') && !pattern.contains('.')
    {
        return false;
    }
    true
}

fn merge_usage(slot: &mut Vec<ModelUsage>, usage: ModelUsage) {
    if let Some(existing) = slot
        .iter_mut()
        .find(|item| paths_equal(&item.project_id, &usage.project_id))
    {
        for file in usage.files {
            if !existing
                .files
                .iter()
                .any(|item| paths_equal(item, &file))
            {
                existing.files.push(file);
            }
        }
        return;
    }
    slot.push(usage);
}

#[derive(Clone, Copy)]
enum WalkMode {
    Weights,
    Attribution,
}

fn walk_files(root: &Path, mode: WalkMode, out: &mut Vec<(PathBuf, u64)>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if is_skipped_dir(&path, mode) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if meta.is_file() {
                let size = meta.len();
                if matches!(mode, WalkMode::Weights) && !is_weight_file(&path, size) {
                    continue;
                }
                out.push((path, size));
            }
        }
    }
}

fn is_skipped_dir(path: &Path, mode: WalkMode) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if SKIP_DIRS.iter().any(|skip| name.eq_ignore_ascii_case(skip)) {
        return true;
    }
    matches!(mode, WalkMode::Attribution)
        && ATTRIB_SKIP_DIRS
            .iter()
            .any(|skip| name.eq_ignore_ascii_case(skip))
}

fn is_text_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        if name.starts_with(".env") {
            return true;
        }
    }
    has_ext(path, TEXT_EXTS)
}

fn has_ext(path: &Path, exts: &[&str]) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    exts.iter().any(|item| ext.eq_ignore_ascii_case(item))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| display_path(path))
}

fn rel_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| !rel.is_empty())
        .unwrap_or_else(|| file_name(path))
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0_u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let Ok(meta) = fs::symlink_metadata(&child) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(child);
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

fn meta_mtime_ms(meta: &fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
}

fn home_dir() -> Option<PathBuf> {
    if let Some(path) = dirs::home_dir() {
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    if let Some(path) = env_path("USERPROFILE") {
        return Some(path);
    }
    let drive = std::env::var("HOMEDRIVE").ok();
    let rest = std::env::var("HOMEPATH").ok();
    match (drive, rest) {
        (Some(drive), Some(rest)) if !drive.trim().is_empty() && !rest.trim().is_empty() => {
            Some(PathBuf::from(format!("{drive}{rest}")))
        }
        _ => None,
    }
}

fn unix_cache_home() -> Option<PathBuf> {
    if let Some(path) = env_path("XDG_CACHE_HOME") {
        return Some(path);
    }
    home_dir().map(|home| home.join(".cache"))
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn http_get_json(path: &str) -> Result<serde_json::Value, String> {
    let body = http_get(path)?;
    serde_json::from_str(&body).map_err(|err| format!("ollama json {path}: {err}"))
}

fn http_get(path: &str) -> Result<String, String> {
    let addr = SocketAddr::from((OLLAMA_HOST, OLLAMA_PORT));
    let mut stream = TcpStream::connect_timeout(&addr, HTTP_TIMEOUT)
        .map_err(|err| format!("connect 127.0.0.1:{OLLAMA_PORT}: {err}"))?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .map_err(|err| err.to_string())?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .map_err(|err| err.to_string())?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{OLLAMA_PORT}\r\nUser-Agent: Orbitry\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("write {path}: {err}"))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|err| format!("read {path}: {err}"))?;
    let text = String::from_utf8_lossy(&raw);
    let (header, body) = text.split_once("\r\n\r\n").or_else(|| text.split_once("\n\n")).ok_or_else(|| {
        format!("ollama {path}: malformed response")
    })?;
    let status = header
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("");
    if status != "200" {
        return Err(format!("ollama {path}: HTTP {status}"));
    }
    Ok(body.trim_end_matches('\0').to_string())
}

/// Minimal RFC 3339 / ISO-8601 parser for Ollama's `modified_at` values.
fn parse_rfc3339_ms(input: &str) -> Option<i64> {
    let s = input.trim();
    let (date, rest) = s.split_once('T').or_else(|| s.split_once('t'))?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    let (time, offset) = split_offset(rest)?;
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second_raw = time_parts.next().unwrap_or("0");
    let (second_str, frac) = second_raw
        .split_once('.')
        .unwrap_or((second_raw, "0"));
    let second: i64 = second_str.parse().ok()?;
    let frac_digits: String = frac.chars().filter(|ch| ch.is_ascii_digit()).take(3).collect();
    let millis: i64 = format!("{:0<3}", frac_digits).parse().ok().unwrap_or(0);
    let days = days_from_civil(year, month, day)?;
    let mut total = days * 86_400 + hour * 3600 + minute * 60 + second;
    total -= offset;
    Some(total * 1000 + millis)
}

fn split_offset(time: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = time.strip_suffix('Z').or_else(|| time.strip_suffix('z')) {
        return Some((stripped, 0));
    }
    let bytes = time.as_bytes();
    let pos = time.rfind(['+', '-'])?;
    if pos == 0 {
        return None;
    }
    let (clock, offset) = time.split_at(pos);
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let body = &offset[1..];
    let (oh, om) = if body.contains(':') {
        let (h, m) = body.split_once(':')?;
        (h, m)
    } else if body.len() == 4 {
        (&body[..2], &body[2..])
    } else if body.len() == 2 {
        (body, "0")
    } else {
        return None;
    };
    let hours: i64 = oh.parse().ok()?;
    let minutes: i64 = om.parse().ok().unwrap_or(0);
    let _ = bytes;
    Some((clock, sign * (hours * 3600 + minutes * 60)))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > 31 {
        return None;
    }
    let mut y = year as i64;
    let m = month as i64;
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelsIndex {
    #[serde(default)]
    scanner_version: u32,
    #[serde(default)]
    fingerprint: String,
    #[serde(default)]
    dirs: BTreeMap<String, IndexedDir>,
    #[serde(default)]
    attrib_fingerprint: Option<String>,
    #[serde(default)]
    attrib_files: BTreeMap<String, AttribFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexedDir {
    #[serde(default)]
    sig: String,
    #[serde(default)]
    weights: Vec<IndexedWeight>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexedWeight {
    path: String,
    size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttribFile {
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mtime: Option<i64>,
    #[serde(default)]
    hits: Vec<String>,
}

fn load_models_index(path: &Path, config: &Config) -> ModelsIndex {
    let Ok(raw) = fs::read(path) else {
        return ModelsIndex {
            scanner_version: MODELS_SCANNER_VERSION,
            fingerprint: models_fingerprint(config),
            ..ModelsIndex::default()
        };
    };
    let Ok(index) = serde_json::from_slice::<ModelsIndex>(&raw) else {
        return ModelsIndex {
            scanner_version: MODELS_SCANNER_VERSION,
            fingerprint: models_fingerprint(config),
            ..ModelsIndex::default()
        };
    };
    let fingerprint = models_fingerprint(config);
    if index.scanner_version != MODELS_SCANNER_VERSION || index.fingerprint != fingerprint {
        return ModelsIndex {
            scanner_version: MODELS_SCANNER_VERSION,
            fingerprint,
            ..ModelsIndex::default()
        };
    }
    index
}

fn save_models_index(path: &Path, index: &ModelsIndex) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("models index dir: {err}"))?;
    }
    let json = serde_json::to_vec_pretty(index).map_err(|err| format!("models index encode: {err}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|err| format!("models index write: {err}"))?;
    fs::rename(&tmp, path).map_err(|err| format!("models index rename: {err}"))?;
    Ok(())
}

fn attrib_needle_fingerprint(needles: &[AttribNeedle]) -> String {
    let mut parts: Vec<&str> = needles.iter().map(|item| item.pattern.as_str()).collect();
    parts.sort();
    parts.dedup();
    parts.join("\n")
}

fn walk_weights_indexed(root: &Path, index: &mut ModelsIndex, out: &mut Vec<(PathBuf, u64)>) {
    let previous = index.clone();
    let mut next = ModelsIndex {
        scanner_version: MODELS_SCANNER_VERSION,
        fingerprint: index.fingerprint.clone(),
        attrib_fingerprint: index.attrib_fingerprint.clone(),
        attrib_files: index.attrib_files.clone(),
        dirs: BTreeMap::new(),
    };
    walk_weights_dir(root, &previous, &mut next, out);
    // Keep dirs from other roots already recorded in this scan.
    for (key, value) in next.dirs {
        index.dirs.insert(key, value);
    }
}

fn walk_weights_dir(
    dir: &Path,
    previous: &ModelsIndex,
    next: &mut ModelsIndex,
    out: &mut Vec<(PathBuf, u64)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut listing: Vec<(String, bool, u64, Option<i64>)> = Vec::new();
    let mut children: Vec<(PathBuf, fs::Metadata, bool)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let is_dir = meta.is_dir();
        if is_dir && is_skipped_dir(&path, WalkMode::Weights) {
            continue;
        }
        let name = file_name(&path);
        listing.push((name, is_dir, meta.len(), meta_mtime_ms(&meta)));
        children.push((path, meta, is_dir));
    }
    listing.sort();
    let sig = dir_signature(&listing);
    let key = display_path(dir);
    if let Some(cached) = previous.dirs.get(&key) {
        if cached.sig == sig {
            for file in &cached.weights {
                out.push((PathBuf::from(&file.path), file.size));
            }
            next.dirs.insert(key, cached.clone());
            return;
        }
    }
    let start = out.len();
    for (path, meta, is_dir) in children {
        if is_dir {
            walk_weights_dir(&path, previous, next, out);
            continue;
        }
        if meta.is_file() {
            let size = meta.len();
            if is_weight_file(&path, size) {
                out.push((path, size));
            }
        }
    }
    let weights = out[start..]
        .iter()
        .map(|(path, size)| IndexedWeight {
            path: display_path(path),
            size: *size,
        })
        .collect();
    next.dirs.insert(key, IndexedDir { sig, weights });
}

fn dir_signature(listing: &[(String, bool, u64, Option<i64>)]) -> String {
    let mut parts = Vec::with_capacity(listing.len());
    for (name, is_dir, size, mtime) in listing {
        parts.push(format!(
            "{name}|{}|{size}|{}",
            if *is_dir { "d" } else { "f" },
            mtime.unwrap_or(0)
        ));
    }
    parts.join("\n")
}
