//! HTTP health checks against ports discovered for a running process.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::ports::{localhost_url, probe_hosts, PortInfo, PortSource};

const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const HEALTH_PATHS: &[&str] = &["/api/health", "/health", "/healthz", "/"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Unknown,
    Ok,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortHealth {
    pub port: u16,
    pub status: HealthStatus,
    pub url: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthInfo {
    pub status: HealthStatus,
    pub url: Option<String>,
    pub checked_at: Option<i64>,
    #[serde(default)]
    pub ports: Vec<PortHealth>,
}

impl HealthInfo {
    pub fn unknown() -> Self {
        Self {
            status: HealthStatus::Unknown,
            url: None,
            checked_at: None,
            ports: Vec::new(),
        }
    }

    pub fn skipped(now: i64) -> Self {
        Self {
            status: HealthStatus::Skipped,
            url: None,
            checked_at: Some(now),
            ports: Vec::new(),
        }
    }

    pub fn is_stale(&self, now: i64, max_age_ms: i64) -> bool {
        match self.checked_at {
            Some(at) => now.saturating_sub(at) >= max_age_ms,
            None => true,
        }
    }
}

pub async fn check_health(ports: &[PortInfo]) -> HealthInfo {
    let now = now_ms();
    let live: Vec<PortInfo> = ports
        .iter()
        .filter(|item| item.source == PortSource::Process)
        .cloned()
        .collect();
    if live.is_empty() {
        return HealthInfo::skipped(now);
    }

    let mut handles = Vec::new();
    for item in live {
        handles.push(tokio::spawn(async move { probe_item(item).await }));
    }

    let mut per_port = Vec::new();
    for handle in handles {
        if let Ok(item) = handle.await {
            per_port.push(item);
        }
    }
    per_port.sort_by_key(|item| item.port);

    let any_ok = per_port.iter().any(|item| item.status == HealthStatus::Ok);
    let any_fail = per_port.iter().any(|item| item.status == HealthStatus::Fail);
    let status = if any_ok {
        HealthStatus::Ok
    } else if any_fail {
        HealthStatus::Fail
    } else {
        HealthStatus::Skipped
    };
    let url = per_port
        .iter()
        .find(|item| item.status == HealthStatus::Ok)
        .or_else(|| per_port.first())
        .and_then(|item| item.url.clone());

    HealthInfo {
        status,
        url,
        checked_at: Some(now),
        ports: per_port,
    }
}

async fn probe_item(item: PortInfo) -> PortHealth {
    if let Some(custom) = item.health_url.clone() {
        if let Some(status) = http_get_url(&custom).await {
            let ok = (200..400).contains(&status);
            return PortHealth {
                port: item.port,
                status: if ok { HealthStatus::Ok } else { HealthStatus::Fail },
                url: Some(custom),
                name: item.name,
            };
        }
        return PortHealth {
            port: item.port,
            status: HealthStatus::Fail,
            url: Some(custom),
            name: item.name,
        };
    }

    let hosts = probe_hosts(&item.binds);
    let display = localhost_url(item.port, None);

    for host in &hosts {
        for path in HEALTH_PATHS {
            if let Some(status) = http_get(host, item.port, path).await {
                if (200..400).contains(&status) || *path == "/" {
                    let url = if (200..400).contains(&status) {
                        localhost_url(item.port, Some(path))
                    } else {
                        display.clone()
                    };
                    return PortHealth {
                        port: item.port,
                        status: HealthStatus::Ok,
                        url: Some(url),
                        name: item.name,
                    };
                }
            }
        }
    }

    for host in &hosts {
        if tcp_ok(host, item.port).await {
            return PortHealth {
                port: item.port,
                status: HealthStatus::Ok,
                url: Some(display),
                name: item.name,
            };
        }
    }

    PortHealth {
        port: item.port,
        status: HealthStatus::Fail,
        url: Some(display),
        name: item.name,
    }
}

async fn tcp_ok(host: &str, port: u16) -> bool {
    timeout(PROBE_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
}

async fn http_get_url(url: &str) -> Option<u16> {
    let (host, port, path) = parse_http_url(url)?;
    http_get(&host, port, &path).await
}

fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let (hostport, path) = match rest.split_once('/') {
        Some((host, rest)) => (host, format!("/{rest}")),
        None => (rest, "/".to_string()),
    };
    if hostport.is_empty() {
        return None;
    }
    let (host, port) = if let Some(rest) = hostport.strip_prefix('[') {
        let end = rest.find("]:")?;
        let port = rest[end + 2..].parse().ok()?;
        (rest[..end].to_string(), port)
    } else if let Some((host, port)) = hostport.rsplit_once(':') {
        (host.to_string(), port.parse().ok()?)
    } else {
        (hostport.to_string(), 80)
    };
    Some((host, port, path))
}

async fn http_get(host: &str, port: u16, path: &str) -> Option<u16> {
    let mut stream = timeout(PROBE_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .ok()?
        .ok()?;
    let request = format!(
        "GET {path} HTTP/1.0\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
    );
    timeout(PROBE_TIMEOUT, stream.write_all(request.as_bytes()))
        .await
        .ok()?
        .ok()?;
    let _ = stream.shutdown().await;
    let mut buf = [0u8; 128];
    let n = timeout(PROBE_TIMEOUT, stream.read(&mut buf))
        .await
        .ok()?
        .ok()?;
    if n == 0 {
        return None;
    }
    parse_status(&buf[..n])
}

fn parse_status(bytes: &[u8]) -> Option<u16> {
    let text = std::str::from_utf8(bytes).ok()?;
    let line = text.lines().next()?;
    let mut parts = line.split_whitespace();
    let version = parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse().ok()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
