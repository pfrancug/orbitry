//! HTTP health checks against ports discovered for a running process.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::ports::PortInfo;

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
    if ports.is_empty() {
        return HealthInfo::skipped(now);
    }

    let mut handles = Vec::new();
    for item in ports {
        let port = item.port;
        let name = item.name.clone();
        handles.push(tokio::spawn(async move {
            let probed = probe_port(port).await;
            PortHealth {
                port,
                status: probed.0,
                url: probed.1,
                name,
            }
        }));
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

async fn probe_port(port: u16) -> (HealthStatus, Option<String>) {
    let addr = ("127.0.0.1", port);
    match timeout(PROBE_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => {}
        _ => {
            return (
                HealthStatus::Fail,
                Some(format!("http://127.0.0.1:{port}")),
            );
        }
    }

    for path in HEALTH_PATHS {
        match http_get(port, path).await {
            Some(status) if (200..400).contains(&status) => {
                return (
                    HealthStatus::Ok,
                    Some(format!("http://127.0.0.1:{port}{path}")),
                );
            }
            Some(_) if *path == "/" => {
                return (HealthStatus::Ok, Some(format!("http://127.0.0.1:{port}/")));
            }
            Some(_) => {}
            None if *path == "/" => {
                return (HealthStatus::Ok, Some(format!("http://127.0.0.1:{port}")));
            }
            None => {}
        }
    }

    (
        HealthStatus::Ok,
        Some(format!("http://127.0.0.1:{port}")),
    )
}

async fn http_get(port: u16, path: &str) -> Option<u16> {
    let mut stream = timeout(PROBE_TIMEOUT, TcpStream::connect(("127.0.0.1", port)))
        .await
        .ok()?
        .ok()?;
    let request = format!(
        "GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
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
