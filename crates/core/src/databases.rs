//! Detect local databases a project is likely to use.
//!
//! Signals are read from config-like files only (env, Prisma, compose,
//! package manifests, shallow SQLite paths). Nothing connects to a database
//! and connection-string passwords are never retained.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::display_path;
use crate::docker::compose_file_in;

const ENV_FILES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.development",
    ".env.development.local",
    ".env.dev",
];

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "target",
    ".git",
    ".next",
    "out",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
    "coverage",
    "vendor",
    ".turbo",
    ".cache",
];

const SQLITE_EXTS: &[&str] = &[".db", ".sqlite", ".sqlite3"];

const MAX_SQLITE_DEPTH: usize = 3;
const MAX_SQLITE_FILES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseSource {
    Env,
    Prisma,
    Compose,
    Dependency,
    File,
}

impl DatabaseSource {
    fn as_str(self) -> &'static str {
        match self {
            DatabaseSource::Env => "env",
            DatabaseSource::Prisma => "prisma",
            DatabaseSource::Compose => "compose",
            DatabaseSource::Dependency => "dependency",
            DatabaseSource::File => "file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    /// Engine id: postgres, mysql, mariadb, mongodb, redis, sqlite, …
    pub engine: String,
    pub source: DatabaseSource,
    /// Where the signal came from, e.g. `.env`, `prisma/schema.prisma`, `docker-compose.yml:db`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Host only when it looks local (localhost / 127.0.0.1 / ::1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Published or configured port when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Relative SQLite (or other file) path when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// File size in bytes for on-disk databases. Omitted for server DBs
    /// (compose/env) — Orbitry never connects to measure them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

impl DatabaseInfo {
    fn key(&self) -> (String, Option<u16>, Option<String>) {
        (
            self.engine.to_ascii_lowercase(),
            self.port,
            self.path
                .as_ref()
                .map(|p| p.replace('\\', "/").to_ascii_lowercase()),
        )
    }
}

/// Detect local databases under a project directory. Fast and side-effect free.
pub fn detect_databases(dir: &Path) -> Vec<DatabaseInfo> {
    let mut found = Vec::new();
    collect_env_databases(dir, &mut found);
    collect_prisma_databases(dir, &mut found);
    collect_compose_databases(dir, &mut found);
    collect_dependency_databases(dir, &mut found);
    collect_sqlite_files(dir, &mut found);
    let mut merged = merge_databases(found);
    attach_file_sizes(dir, &mut merged);
    merged
}

/// Fill `size_bytes` for on-disk DB files when the path resolves under the project.
/// Server DBs (compose/env without a file) stay without a size — we never connect.
fn attach_file_sizes(dir: &Path, dbs: &mut [DatabaseInfo]) {
    for db in dbs {
        if db.size_bytes.is_some() {
            continue;
        }
        let Some(rel) = db.path.as_ref() else {
            continue;
        };
        let candidate = {
            let path = Path::new(rel);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                dir.join(path)
            }
        };
        if let Ok(meta) = fs::metadata(&candidate) {
            if meta.is_file() {
                db.size_bytes = Some(meta.len());
            }
        }
    }
}

fn merge_databases(items: Vec<DatabaseInfo>) -> Vec<DatabaseInfo> {
    let mut out: Vec<DatabaseInfo> = Vec::new();
    for item in items {
        if let Some(existing) = out.iter_mut().find(|other| other.key() == item.key()) {
            enrich(existing, &item);
            continue;
        }
        // Fold a weak same-engine hint (no port/path) into a richer entry.
        if is_weak_db(&item) {
            if let Some(existing) = out
                .iter_mut()
                .find(|other| other.engine.eq_ignore_ascii_case(&item.engine) && !is_weak_db(other))
            {
                enrich(existing, &item);
                continue;
            }
        } else if let Some(existing) = out
            .iter_mut()
            .find(|other| other.engine.eq_ignore_ascii_case(&item.engine) && is_weak_db(other))
        {
            let mut merged = item.clone();
            enrich(&mut merged, existing);
            *existing = merged;
            continue;
        }
        out.push(item);
    }
    out.sort_by(|a, b| {
        a.engine
            .to_ascii_lowercase()
            .cmp(&b.engine.to_ascii_lowercase())
            .then_with(|| a.port.cmp(&b.port))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.source.as_str().cmp(b.source.as_str()))
    });
    out
}

fn is_weak_db(item: &DatabaseInfo) -> bool {
    item.port.is_none() && item.path.is_none()
}

fn enrich(target: &mut DatabaseInfo, other: &DatabaseInfo) {
    if target.host.is_none() {
        target.host = other.host.clone();
    }
    if target.port.is_none() {
        target.port = other.port;
    }
    if target.path.is_none() {
        target.path = other.path.clone();
    }
    if target.size_bytes.is_none() {
        target.size_bytes = other.size_bytes;
    }
    // Prefer a more specific source when the existing one is only a weak hint.
    let weak_sources = [DatabaseSource::Dependency, DatabaseSource::Prisma];
    if weak_sources.contains(&target.source) && !weak_sources.contains(&other.source) {
        target.source = other.source;
        if other.detail.is_some() {
            target.detail = other.detail.clone();
        }
    } else if target.detail.is_none() {
        target.detail = other.detail.clone();
    }
}

fn collect_env_databases(dir: &Path, out: &mut Vec<DatabaseInfo>) {
    for name in ENV_FILES {
        let path = dir.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for (key, value) in env_pairs(&text) {
            if let Some(mut info) = database_from_env_pair(&key, &value) {
                info.detail = Some((*name).to_string());
                out.push(info);
            }
        }
    }
}

fn env_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key
            .trim()
            .trim_start_matches("export ")
            .trim()
            .to_ascii_uppercase();
        if key.is_empty() {
            continue;
        }
        let value = strip_env_quotes(value.trim());
        if value.is_empty() {
            continue;
        }
        pairs.push((key, value));
    }
    pairs
}

fn strip_env_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

fn database_from_env_pair(key: &str, value: &str) -> Option<DatabaseInfo> {
    let key_l = key.to_ascii_lowercase();
    if is_url_like_db_key(&key_l) {
        return parse_connection_url(value).map(|mut info| {
            info.source = DatabaseSource::Env;
            info
        });
    }
    if key_l == "postgres_port" || key_l == "postgresql_port" || key_l == "pgport" {
        let port = parse_port(value)?;
        return Some(DatabaseInfo {
            engine: "postgres".into(),
            source: DatabaseSource::Env,
            detail: None,
            host: Some("localhost".into()),
            port: Some(port),
            path: None,
            size_bytes: None,
        });
    }
    if key_l == "mysql_port" || key_l == "mariadb_port" {
        let port = parse_port(value)?;
        let engine = if key_l.starts_with("maria") {
            "mariadb"
        } else {
            "mysql"
        };
        return Some(DatabaseInfo {
            engine: engine.into(),
            source: DatabaseSource::Env,
            detail: None,
            host: Some("localhost".into()),
            port: Some(port),
            path: None,
            size_bytes: None,
        });
    }
    if key_l == "redis_port" {
        let port = parse_port(value)?;
        return Some(DatabaseInfo {
            engine: "redis".into(),
            source: DatabaseSource::Env,
            detail: None,
            host: Some("localhost".into()),
            port: Some(port),
            path: None,
            size_bytes: None,
        });
    }
    if key_l == "mongo_port" || key_l == "mongodb_port" {
        let port = parse_port(value)?;
        return Some(DatabaseInfo {
            engine: "mongodb".into(),
            source: DatabaseSource::Env,
            detail: None,
            host: Some("localhost".into()),
            port: Some(port),
            path: None,
            size_bytes: None,
        });
    }
    if key_l == "sqlite_path"
        || key_l == "sqlite_db"
        || key_l == "sqlite_database"
        || key_l == "db_file"
        || key_l == "database_path"
    {
        if looks_remote_host(value) {
            return None;
        }
        return Some(DatabaseInfo {
            engine: "sqlite".into(),
            source: DatabaseSource::Env,
            detail: None,
            host: None,
            port: None,
            path: Some(shorten_path(value)),
            size_bytes: None,
        });
    }
    // Host-only hints that clearly point at a local DB engine.
    if matches!(
        key_l.as_str(),
        "postgres_host" | "postgresql_host" | "pghost" | "mysql_host" | "mariadb_host"
            | "redis_host" | "mongo_host" | "mongodb_host"
    ) {
        if !is_local_host(value) {
            return None;
        }
        let engine = match key_l.as_str() {
            "postgres_host" | "postgresql_host" | "pghost" => "postgres",
            "mysql_host" => "mysql",
            "mariadb_host" => "mariadb",
            "redis_host" => "redis",
            "mongo_host" | "mongodb_host" => "mongodb",
            _ => return None,
        };
        return Some(DatabaseInfo {
            engine: engine.into(),
            source: DatabaseSource::Env,
            detail: None,
            host: Some(normalize_host(value)),
            port: None,
            path: None,
            size_bytes: None,
        });
    }
    None
}

fn is_url_like_db_key(key: &str) -> bool {
    matches!(
        key,
        "database_url"
            | "database_uri"
            | "db_url"
            | "db_uri"
            | "postgres_url"
            | "postgresql_url"
            | "postgres_uri"
            | "postgresql_uri"
            | "mysql_url"
            | "mysql_uri"
            | "mariadb_url"
            | "mariadb_uri"
            | "mongodb_uri"
            | "mongodb_url"
            | "mongo_uri"
            | "mongo_url"
            | "redis_url"
            | "redis_uri"
            | "sqlite_url"
            | "drizzle_database_url"
            | "prisma_database_url"
            | "connection_string"
            | "connection_url"
    ) || key.ends_with("_database_url")
        || key.ends_with("_database_uri")
}

fn parse_connection_url(raw: &str) -> Option<DatabaseInfo> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    // file: and bare relative sqlite paths
    if let Some(path) = sqlite_path_from_url(value) {
        return Some(DatabaseInfo {
            engine: "sqlite".into(),
            source: DatabaseSource::Env,
            detail: None,
            host: None,
            port: None,
            path: Some(path),
            size_bytes: None,
        });
    }

    let (scheme, rest) = split_scheme(value)?;
    let engine = engine_from_scheme(&scheme)?;

    // redis://localhost:6379/0  or  postgres://user:pass@localhost:5432/db
    let after_auth = strip_userinfo(rest);
    let (host_port, _) = after_auth
        .split_once('/')
        .map(|(h, p)| (h, Some(p)))
        .unwrap_or((after_auth, None));
    let (host, port) = split_host_port(host_port);
    if !is_local_host(&host) {
        return None;
    }
    Some(DatabaseInfo {
        engine: engine.into(),
        source: DatabaseSource::Env,
        detail: None,
        host: Some(normalize_host(&host)),
        port,
        path: None,
        size_bytes: None,
    })
}

fn sqlite_path_from_url(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("file:") {
        let rest = value[5..].trim_start_matches('/');
        // file:/absolute or file:./relative — keep a short display form
        let path = if value[5..].starts_with("///") {
            // file:///C:/... or file:///home/...
            value[7..].split('?').next().unwrap_or("").to_string()
        } else if value[5..].starts_with("//localhost/") || value[5..].starts_with("//./") {
            value
                .split_once("localhost/")
                .or_else(|| value.split_once("//./"))
                .map(|(_, p)| p.split('?').next().unwrap_or("").to_string())
                .unwrap_or_else(|| rest.split('?').next().unwrap_or("").to_string())
        } else {
            rest.split('?').next().unwrap_or("").to_string()
        };
        if path.is_empty() || looks_remote_host(&path) {
            return None;
        }
        return Some(shorten_path(&path));
    }
    if lower.starts_with("sqlite:") || lower.starts_with("sqlite3:") {
        let rest = value
            .split_once(':')
            .map(|(_, r)| r)
            .unwrap_or("")
            .trim_start_matches('/');
        let path = rest.split('?').next().unwrap_or("").to_string();
        if path.is_empty() {
            return None;
        }
        return Some(shorten_path(&path));
    }
    // Bare relative path ending in a sqlite extension.
    if !value.contains("://") && looks_like_sqlite_file(value) && !looks_remote_host(value) {
        return Some(shorten_path(value));
    }
    None
}

fn split_scheme(value: &str) -> Option<(String, &str)> {
    let (scheme, rest) = value.split_once("://")?;
    let scheme = scheme
        .split('+')
        .next()
        .unwrap_or(scheme)
        .trim()
        .to_ascii_lowercase();
    Some((scheme, rest))
}

fn engine_from_scheme(scheme: &str) -> Option<&'static str> {
    match scheme {
        "postgres" | "postgresql" | "pgsql" => Some("postgres"),
        "mysql" | "mysql2" => Some("mysql"),
        "mariadb" => Some("mariadb"),
        "mongodb" | "mongodb+srv" | "mongo" => Some("mongodb"),
        "redis" | "rediss" => Some("redis"),
        "sqlite" | "sqlite3" => Some("sqlite"),
        _ => None,
    }
}

/// Drop `user:password@` without keeping any of it.
fn strip_userinfo(rest: &str) -> &str {
    if let Some(at) = rest.rfind('@') {
        // Avoid treating `@` inside path segments after the host as userinfo.
        let before = &rest[..at];
        if !before.contains('/') {
            return &rest[at + 1..];
        }
    }
    rest
}

fn split_host_port(host_port: &str) -> (String, Option<u16>) {
    let host_port = host_port.trim().trim_matches(['[', ']']);
    if host_port.is_empty() {
        return ("localhost".into(), None);
    }
    // IPv6 [::1]:6379
    if let Some(end) = host_port.find(']') {
        let host = host_port[1..end].to_string();
        let port = host_port[end + 1..]
            .strip_prefix(':')
            .and_then(parse_port);
        return (host, port);
    }
    if let Some((host, port)) = host_port.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return (host.to_string(), parse_port(port));
        }
    }
    (host_port.to_string(), None)
}

fn parse_port(value: &str) -> Option<u16> {
    let trimmed = value.trim().trim_matches(['\'', '"']);
    let digits: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let port: u16 = digits.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some(port)
}

fn is_local_host(host: &str) -> bool {
    let h = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    matches!(
        h.as_str(),
        "" | "localhost"
            | "127.0.0.1"
            | "0.0.0.0"
            | "::1"
            | "host.docker.internal"
            | "docker.for.mac.localhost"
            | "docker.for.win.localhost"
    )
}

fn normalize_host(host: &str) -> String {
    let h = host.trim().trim_matches(['[', ']']);
    if h.is_empty() {
        return "localhost".into();
    }
    h.to_string()
}

fn looks_remote_host(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("://") {
        if let Some((_, rest)) = split_scheme(value) {
            let after = strip_userinfo(rest);
            let host = after.split('/').next().unwrap_or("");
            let (host, _) = split_host_port(host);
            return !is_local_host(&host);
        }
    }
    false
}

fn shorten_path(path: &str) -> String {
    let cleaned = path.replace('\\', "/");
    // Prefer a project-relative looking tail; never include credentials.
    if let Some(idx) = cleaned.rfind("/prisma/") {
        return cleaned[idx + 1..].to_string();
    }
    if cleaned.len() > 80 {
        if let Some(name) = Path::new(&cleaned).file_name().and_then(|s| s.to_str()) {
            return name.to_string();
        }
    }
    cleaned.trim_start_matches("./").to_string()
}

fn looks_like_sqlite_file(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    SQLITE_EXTS.iter().any(|ext| lower.ends_with(ext))
}

fn collect_prisma_databases(dir: &Path, out: &mut Vec<DatabaseInfo>) {
    const CANDIDATES: &[&str] = &[
        "prisma/schema.prisma",
        "schema.prisma",
        "db/schema.prisma",
        "database/schema.prisma",
    ];
    for rel in CANDIDATES {
        let path = dir.join(rel);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(mut info) = database_from_prisma(&text) {
            info.detail = Some((*rel).replace('\\', "/"));
            out.push(info);
        }
    }
}

fn database_from_prisma(text: &str) -> Option<DatabaseInfo> {
    // Look for: datasource db { provider = "postgresql" ... }
    let lower = text.to_ascii_lowercase();
    let Some(ds_at) = lower.find("datasource") else {
        return None;
    };
    let after = &text[ds_at..];
    let Some(brace) = after.find('{') else {
        return None;
    };
    let body = &after[brace + 1..];
    let Some(end) = body.find('}') else {
        return None;
    };
    let block = &body[..end];
    let block_lower = block.to_ascii_lowercase();
    let provider = prisma_string_field(&block_lower, block, "provider")?;
    let engine = match provider.as_str() {
        "postgresql" | "postgres" => "postgres".to_string(),
        "mysql" => "mysql".to_string(),
        "sqlite" => "sqlite".to_string(),
        "mongodb" | "mongo" => "mongodb".to_string(),
        "sqlserver" => "sqlserver".to_string(),
        "cockroachdb" => "cockroachdb".to_string(),
        other => other.to_string(),
    };
    let mut path = None;
    let mut host = None;
    let mut port = None;
    if let Some(url) = prisma_string_field(&block_lower, block, "url") {
        // url = env("DATABASE_URL") → no concrete target here; dependency on env.
        if !url.to_ascii_lowercase().starts_with("env(") {
            if let Some(parsed) = parse_connection_url(&url) {
                path = parsed.path;
                host = parsed.host;
                port = parsed.port;
            }
        }
    }
    // Non-local remote URLs in prisma should not be listed as local DBs,
    // unless sqlite (file) or provider alone with no remote host.
    if let Some(ref h) = host {
        if !is_local_host(h) {
            return None;
        }
    }
    Some(DatabaseInfo {
        engine,
        source: DatabaseSource::Prisma,
        detail: None,
        host,
        port,
        path,
        size_bytes: None,
    })
}

fn prisma_string_field(block_lower: &str, block: &str, field: &str) -> Option<String> {
    let needle = format!("{field}");
    let mut search = 0;
    while let Some(rel) = block_lower[search..].find(&needle) {
        let abs = search + rel;
        let after_name = abs + needle.len();
        let rest = block[after_name..].trim_start();
        if !rest.starts_with('=') {
            search = after_name;
            continue;
        }
        let rest = rest[1..].trim_start();
        // "postgresql" or env("DATABASE_URL")
        if rest.starts_with('"') || rest.starts_with('\'') {
            let quote = rest.chars().next()?;
            let inner = &rest[1..];
            let end = inner.find(quote)?;
            return Some(inner[..end].to_string());
        }
        if rest.to_ascii_lowercase().starts_with("env(") {
            let close = rest.find(')')?;
            return Some(rest[..=close].to_string());
        }
        search = after_name;
    }
    None
}

fn collect_compose_databases(dir: &Path, out: &mut Vec<DatabaseInfo>) {
    let Some(file) = compose_file_in(dir) else {
        return;
    };
    let Ok(text) = fs::read_to_string(&file) else {
        return;
    };
    let file_name = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("compose.yml");
    for service in parse_compose_db_services(&text) {
        out.push(DatabaseInfo {
            engine: service.engine,
            source: DatabaseSource::Compose,
            detail: Some(format!("{file_name}:{}", service.name)),
            host: Some("localhost".into()),
            port: service.port,
            path: None,
            size_bytes: None,
        });
    }
}

struct ComposeDbService {
    name: String,
    engine: String,
    port: Option<u16>,
}

fn parse_compose_db_services(text: &str) -> Vec<ComposeDbService> {
    let mut services = Vec::new();
    let mut in_services = false;
    let mut services_indent: Option<usize> = None;
    let mut current: Option<ComposeServiceScratch> = None;

    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw.chars().take_while(|ch| *ch == ' ' || *ch == '\t').count();
        if !in_services {
            if trimmed == "services:" || trimmed.starts_with("services:") {
                in_services = true;
                services_indent = Some(indent);
            }
            continue;
        }
        let Some(base) = services_indent else {
            continue;
        };
        if indent <= base && trimmed.ends_with(':') && !trimmed.starts_with("services:") {
            break;
        }
        // New service key
        if indent > base
            && looks_like_yaml_key(trimmed)
            && !is_known_compose_field(trimmed)
            && trimmed.contains(':')
            && !trimmed.starts_with('-')
        {
            if let Some(done) = current.take().and_then(finish_compose_service) {
                services.push(done);
            }
            if let Some(name) = yaml_key(trimmed) {
                current = Some(ComposeServiceScratch {
                    name,
                    image: None,
                    ports: Vec::new(),
                    collecting_ports: false,
                });
            }
            continue;
        }
        let Some(scratch) = current.as_mut() else {
            continue;
        };
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("image:") {
            let value = trimmed.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
            scratch.image = Some(value.trim_matches(['\'', '"']).to_string());
        } else if lower.starts_with("- ") && scratch.collecting_ports {
            if let Some(port) = host_port_from_compose_mapping(&trimmed[2..]) {
                scratch.ports.push(port);
            }
        } else if lower.starts_with("ports:") {
            scratch.collecting_ports = true;
            // Inline: ports: ["5432:5432"]
            if let Some(rest) = trimmed.split_once(':').map(|(_, v)| v.trim()) {
                if rest.starts_with('[') {
                    for part in rest.trim_matches(['[', ']']).split(',') {
                        if let Some(port) = host_port_from_compose_mapping(part) {
                            scratch.ports.push(port);
                        }
                    }
                    scratch.collecting_ports = false;
                } else if !rest.is_empty() {
                    if let Some(port) = host_port_from_compose_mapping(rest) {
                        scratch.ports.push(port);
                    }
                    scratch.collecting_ports = false;
                }
            }
        } else if scratch.collecting_ports && !trimmed.starts_with('-') {
            scratch.collecting_ports = false;
        }
    }
    if let Some(done) = current.take().and_then(finish_compose_service) {
        services.push(done);
    }
    services
}

struct ComposeServiceScratch {
    name: String,
    image: Option<String>,
    ports: Vec<u16>,
    collecting_ports: bool,
}

fn finish_compose_service(scratch: ComposeServiceScratch) -> Option<ComposeDbService> {
    let engine = engine_from_compose(&scratch.name, scratch.image.as_deref())?;
    let port = scratch
        .ports
        .first()
        .copied()
        .or_else(|| default_port_for_engine(engine));
    Some(ComposeDbService {
        name: scratch.name,
        engine: engine.to_string(),
        port,
    })
}

fn engine_from_compose(name: &str, image: Option<&str>) -> Option<&'static str> {
    let blob = format!("{} {}", name, image.unwrap_or(""))
        .to_ascii_lowercase()
        .replace(['_', '-'], "");
    if blob.contains("postgres") || blob.contains("pgsql") {
        return Some("postgres");
    }
    if blob.contains("mariadb") {
        return Some("mariadb");
    }
    if blob.contains("mysql") {
        return Some("mysql");
    }
    if blob.contains("mongo") {
        return Some("mongodb");
    }
    if blob.contains("redis") || blob.contains("valkey") {
        return Some("redis");
    }
    if blob.contains("sqlite") {
        return Some("sqlite");
    }
    if blob.contains("cockroach") {
        return Some("cockroachdb");
    }
    if blob.contains("clickhouse") {
        return Some("clickhouse");
    }
    if blob.contains("elasticsearch") || blob.contains("opensearch") {
        return Some("elasticsearch");
    }
    None
}

fn default_port_for_engine(engine: &str) -> Option<u16> {
    match engine {
        "postgres" | "cockroachdb" => Some(5432),
        "mysql" | "mariadb" => Some(3306),
        "mongodb" => Some(27017),
        "redis" => Some(6379),
        "elasticsearch" => Some(9200),
        "clickhouse" => Some(8123),
        "sqlserver" => Some(1433),
        _ => None,
    }
}

fn host_port_from_compose_mapping(raw: &str) -> Option<u16> {
    let value = raw.trim().trim_matches(['\'', '"']);
    // Long syntax: target: 5432 / published: 5432
    if value.to_ascii_lowercase().starts_with("target:")
        || value.to_ascii_lowercase().starts_with("published:")
    {
        return None;
    }
    // "5432:5432", "127.0.0.1:5432:5432", "5432"
    let parts: Vec<&str> = value.split(':').collect();
    match parts.as_slice() {
        // host:hostPort:containerPort
        [host, host_port, _container] => {
            if is_local_host(host) || *host == "0.0.0.0" {
                parse_port(host_port)
            } else {
                None
            }
        }
        // hostPort:containerPort
        [host_port, _container]
            if host_port.chars().all(|c| c.is_ascii_digit()) =>
        {
            parse_port(host_port)
        }
        // host:hostPort (unusual but seen)
        [host, host_port] if is_local_host(host) => parse_port(host_port),
        [port] => parse_port(port),
        _ => None,
    }
}

fn looks_like_yaml_key(trimmed: &str) -> bool {
    let Some(key) = trimmed.split(':').next() else {
        return false;
    };
    !key.is_empty() && !key.starts_with('-') && !key.contains(' ') && !key.contains('{')
}

fn is_known_compose_field(trimmed: &str) -> bool {
    const FIELDS: &[&str] = &[
        "image:", "build:", "command:", "ports:", "environment:", "env_file:", "volumes:",
        "depends_on:", "restart:", "networks:", "network_mode:", "container_name:", "profiles:",
        "healthcheck:", "labels:", "expose:", "stdin_open:", "tty:", "user:", "working_dir:",
        "entrypoint:", "extra_hosts:", "cap_add:", "cap_drop:", "devices:", "deploy:", "logging:",
        "secrets:", "configs:", "extends:", "pid:", "privileged:", "pull_policy:", "scale:",
        "init:", "read_only:", "platform:", "runtime:", "sysctls:", "shm_size:", "hostname:",
    ];
    let lower = trimmed.to_ascii_lowercase();
    FIELDS.iter().any(|field| lower.starts_with(field))
}

fn yaml_key(trimmed: &str) -> Option<String> {
    let key = trimmed.split(':').next()?.trim();
    if key.is_empty() || key.starts_with('&') || key.starts_with('*') {
        return None;
    }
    Some(key.to_string())
}

fn collect_dependency_databases(dir: &Path, out: &mut Vec<DatabaseInfo>) {
    let mut engines = BTreeSet::new();
    if let Some(names) = read_js_dep_names(dir) {
        for name in names {
            if let Some(engine) = engine_from_js_dep(&name) {
                engines.insert(engine.to_string());
            }
        }
    }
    if let Some(names) = read_cargo_dep_names(dir) {
        for name in names {
            if let Some(engine) = engine_from_rust_dep(&name) {
                engines.insert(engine.to_string());
            }
        }
    }
    if let Some(names) = read_python_dep_names(dir) {
        for name in names {
            if let Some(engine) = engine_from_python_dep(&name) {
                engines.insert(engine.to_string());
            }
        }
    }
    if let Some(names) = read_go_dep_names(dir) {
        for name in names {
            if let Some(engine) = engine_from_go_dep(&name) {
                engines.insert(engine.to_string());
            }
        }
    }
    for engine in engines {
        out.push(DatabaseInfo {
            engine,
            source: DatabaseSource::Dependency,
            detail: Some("package manifest".into()),
            host: None,
            port: None,
            path: None,
            size_bytes: None,
        });
    }
}

fn read_js_dep_names(dir: &Path) -> Option<Vec<String>> {
    let data = fs::read(dir.join("package.json")).ok()?;
    let data = crate::config::strip_bom(&data);
    let value: serde_json::Value = serde_json::from_slice(data).ok()?;
    let mut names = Vec::new();
    for key in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        if let Some(map) = value.get(key).and_then(|v| v.as_object()) {
            names.extend(map.keys().cloned());
        }
    }
    Some(names)
}

fn engine_from_js_dep(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        "better-sqlite3" | "sqlite3" | "sql.js" | "sqlite" => Some("sqlite"),
        "pg" | "postgres" | "postgresql" | "@neondatabase/serverless" | "pg-native" => {
            Some("postgres")
        }
        "mysql" | "mysql2" => Some("mysql"),
        "mariadb" => Some("mariadb"),
        "mongodb" | "mongoose" => Some("mongodb"),
        "ioredis" | "redis" | "@redis/client" => Some("redis"),
        "drizzle-orm" | "prisma" | "@prisma/client" | "typeorm" | "sequelize" | "knex"
        | "mikro-orm" | "@mikro-orm/core" => None, // ORM without a concrete engine
        _ => {
            if n.contains("sqlite") {
                Some("sqlite")
            } else if n.starts_with("@prisma/") && n.contains("sqlite") {
                Some("sqlite")
            } else {
                None
            }
        }
    }
}

fn read_cargo_dep_names(dir: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let value: toml::Value = text.parse().ok()?;
    let mut names = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = value.get(section).and_then(|v| v.as_table()) {
            names.extend(table.keys().cloned());
        }
    }
    Some(names)
}

fn engine_from_rust_dep(name: &str) -> Option<&'static str> {
    match name {
        "sqlx" | "diesel" | "sea-orm" | "rusqlite" | "sqlite" => {
            // sqlx/diesel are multi-engine; rusqlite is sqlite. Prefer concrete ones.
            if name == "rusqlite" || name == "sqlite" {
                Some("sqlite")
            } else {
                None
            }
        }
        "tokio-postgres" | "postgres" | "deadpool-postgres" => Some("postgres"),
        "mysql" | "mysql_async" | "sqlx-mysql" => Some("mysql"),
        "mongodb" => Some("mongodb"),
        "redis" | "fred" => Some("redis"),
        _ => None,
    }
}

fn read_python_dep_names(dir: &Path) -> Option<Vec<String>> {
    let mut names = Vec::new();
    if let Ok(text) = fs::read_to_string(dir.join("pyproject.toml")) {
        if let Ok(value) = text.parse::<toml::Value>() {
            if let Some(project) = value.get("project") {
                if let Some(deps) = project.get("dependencies").and_then(|v| v.as_array()) {
                    for dep in deps {
                        if let Some(s) = dep.as_str() {
                            names.push(python_dep_name(s));
                        }
                    }
                }
            }
            if let Some(poetry) = value.get("tool").and_then(|t| t.get("poetry")) {
                if let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table()) {
                    names.extend(deps.keys().cloned());
                }
            }
        }
    }
    if let Ok(text) = fs::read_to_string(dir.join("requirements.txt")) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
                continue;
            }
            names.push(python_dep_name(line));
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

fn python_dep_name(spec: &str) -> String {
    let base = spec
        .split(['=', '<', '>', '!', '~', ';', '['])
        .next()
        .unwrap_or(spec)
        .trim();
    base.to_ascii_lowercase()
}

fn engine_from_python_dep(name: &str) -> Option<&'static str> {
    match name {
        "psycopg" | "psycopg2" | "psycopg2-binary" | "asyncpg" => Some("postgres"),
        "mysqlclient" | "pymysql" | "mysql-connector-python" | "aiomysql" => Some("mysql"),
        "mariadb" => Some("mariadb"),
        "pymongo" | "motor" => Some("mongodb"),
        "redis" | "aioredis" => Some("redis"),
        "aiosqlite" => Some("sqlite"),
        _ => None,
    }
}

fn read_go_dep_names(dir: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(dir.join("go.mod")).ok()?;
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("require ") && !line.contains('(') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                names.push(parts[1].to_string());
            }
        } else if line.contains('/') && !line.starts_with("module ") && !line.starts_with("go ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if !parts.is_empty() && parts[0].contains('/') {
                names.push(parts[0].to_string());
            }
        }
    }
    Some(names)
}

fn engine_from_go_dep(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.contains("lib/pq") || n.contains("jackc/pgx") || n.contains("go-pg/pg") {
        return Some("postgres");
    }
    if n.contains("go-sql-driver/mysql") {
        return Some("mysql");
    }
    if n.contains("go-redis") || n.contains("redis/go-redis") {
        return Some("redis");
    }
    if n.contains("mongo-driver") {
        return Some("mongodb");
    }
    if n.contains("mattn/go-sqlite3") || n.contains("modernc.org/sqlite") {
        return Some("sqlite");
    }
    None
}

fn collect_sqlite_files(dir: &Path, out: &mut Vec<DatabaseInfo>) {
    let mut files = Vec::new();
    walk_sqlite(dir, dir, 0, &mut files);
    for (rel, size) in files {
        out.push(DatabaseInfo {
            engine: "sqlite".into(),
            source: DatabaseSource::File,
            detail: Some(rel.clone()),
            host: None,
            port: None,
            path: Some(rel),
            size_bytes: size,
        });
    }
}

fn walk_sqlite(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, Option<u64>)>) {
    if out.len() >= MAX_SQLITE_FILES || depth > MAX_SQLITE_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= MAX_SQLITE_FILES {
            break;
        }
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            walk_sqlite(root, &path, depth + 1, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if !SQLITE_EXTS.iter().any(|ext| lower.ends_with(ext)) {
            continue;
        }
        // Skip obvious non-DB artifacts.
        if lower.ends_with(".db-journal")
            || lower.ends_with(".db-wal")
            || lower.ends_with(".db-shm")
            || lower.contains("thumbs")
        {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|p| display_path(p))
            .unwrap_or_else(|_| display_path(&path));
        let rel = rel.replace('\\', "/");
        if out.iter().any(|(existing, _)| existing == &rel) {
            continue;
        }
        let size = fs::metadata(&path).ok().map(|meta| meta.len());
        out.push((rel, size));
    }
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };
    name.starts_with('.') && name != ".prisma"
        || SKIP_DIRS.iter().any(|skip| *skip == name)
}
