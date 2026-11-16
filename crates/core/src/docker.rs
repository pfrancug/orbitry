//! Docker Compose detection and actions. Orbitry works without Docker:
//! compose files are still listed, and actions are disabled with an explanation
//! when the Docker CLI is missing or not responding.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::config::display_path;
use crate::group::RepoGroup;
#[cfg(windows)]
use crate::logpump::create_no_window_flags;

use std::sync::Mutex;

static DOCKER_CLI: Mutex<Option<(std::time::Instant, Result<String, String>)>> =
    Mutex::new(None);

const CLI_TIMEOUT: Duration = Duration::from_secs(2);
const ACTION_TIMEOUT: Duration = Duration::from_secs(120);
const LOG_TIMEOUT: Duration = Duration::from_secs(15);
const LOG_TAIL: u32 = 200;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeService {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProject {
    pub project_id: String,
    pub project_name: String,
    pub workdir: String,
    pub file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<ComposeService>,
    pub docker_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComposeAction {
    Up,
    Down,
    Restart,
}

pub fn compose_file_in(dir: &Path) -> Option<PathBuf> {
    const NAMES: &[&str] = &[
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ];
    for name in NAMES {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Service names from a compose file, without calling Docker.
pub fn parse_compose_services(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_services = false;
    let mut services_indent: Option<usize> = None;
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
        if let Some(base) = services_indent {
            if indent <= base && trimmed.ends_with(':') && !trimmed.starts_with("services:") {
                break;
            }
            if indent > base && looks_like_service_key(trimmed) && !is_compose_field(trimmed) {
                if let Some(name) = service_key(trimmed) {
                    if !names.iter().any(|existing| existing == &name) {
                        names.push(name);
                    }
                }
            }
        }
    }
    names
}

fn looks_like_service_key(trimmed: &str) -> bool {
    let Some(key) = trimmed.split(':').next() else {
        return false;
    };
    !key.is_empty() && !key.starts_with('-') && !key.contains(' ') && !key.contains('{')
}

fn is_compose_field(trimmed: &str) -> bool {
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

fn service_key(trimmed: &str) -> Option<String> {
    let key = trimmed.split(':').next()?.trim();
    if key.is_empty() || key.starts_with('&') || key.starts_with('*') {
        return None;
    }
    Some(key.to_string())
}

pub fn static_compose_projects(groups: &[RepoGroup]) -> Vec<ComposeProject> {
    let mut out = Vec::new();
    for group in groups {
        let dir = PathBuf::from(&group.path);
        let Some(file) = compose_file_in(&dir) else {
            continue;
        };
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let names = parse_compose_services(&text);
        let services = names
            .into_iter()
            .map(|name| ComposeService {
                name,
                ..ComposeService::default()
            })
            .collect();
        out.push(ComposeProject {
            project_id: group.id.clone(),
            project_name: group.name.clone(),
            workdir: display_path(&dir),
            file: display_path(&file),
            services,
            docker_available: false,
            docker_command: None,
            error: None,
        });
    }
    out
}

/// Detect the Docker CLI and fill live service state. Safe when Docker is missing.
pub async fn refresh_compose(projects: &mut [ComposeProject]) {
    let cli = detect_docker_cli().await;
    match &cli {
        Ok(command) => {
            for project in projects.iter_mut() {
                project.docker_available = true;
                project.docker_command = Some(command.clone());
                project.error = None;
                if let Err(err) = fill_live_state(project, command).await {
                    project.error = Some(err);
                }
            }
        }
        Err(err) => {
            for project in projects.iter_mut() {
                project.docker_available = false;
                project.docker_command = None;
                project.error = Some(err.clone());
            }
        }
    }
}

pub async fn run_compose_action(
    project: &ComposeProject,
    action: ComposeAction,
    service: Option<&str>,
) -> Result<String, String> {
    let command = detect_docker_cli().await?;
    let mut args = compose_args(&command, project);
    match action {
        ComposeAction::Up => {
            args.extend(["up".into(), "-d".into(), "--remove-orphans".into()]);
        }
        ComposeAction::Down => {
            if service.is_some() {
                args.push("stop".into());
            } else {
                args.push("down".into());
            }
        }
        ComposeAction::Restart => args.push("restart".into()),
    }
    if let Some(service) = service {
        if !service.is_empty() {
            args.push(service.to_string());
        }
    }
    run_docker(&command, &args, ACTION_TIMEOUT).await
}

pub async fn compose_logs(
    project: &ComposeProject,
    service: Option<&str>,
) -> Result<String, String> {
    let command = detect_docker_cli().await?;
    let mut args = compose_args(&command, project);
    args.extend([
        "logs".into(),
        "--no-color".into(),
        "--timestamps".into(),
        "--tail".into(),
        LOG_TAIL.to_string(),
    ]);
    if let Some(service) = service {
        if !service.is_empty() {
            args.push(service.to_string());
        }
    }
    run_docker(&command, &args, LOG_TIMEOUT).await
}

fn compose_args(command: &str, project: &ComposeProject) -> Vec<String> {
    let mut args = Vec::new();
    if command != "docker-compose" {
        args.push("compose".into());
    }
    args.extend([
        "--project-directory".into(),
        project.workdir.clone(),
        "-f".into(),
        project.file.clone(),
    ]);
    args
}

async fn fill_live_state(project: &mut ComposeProject, command: &str) -> Result<(), String> {
    let mut args = compose_args(command, project);
    args.extend(["ps".into(), "-a".into(), "--format".into(), "json".into()]);
    let raw = run_docker(command, &args, CLI_TIMEOUT).await?;
    let live = parse_ps_json(&raw);
    for item in live {
        if let Some(slot) = project
            .services
            .iter_mut()
            .find(|service| service.name == item.name)
        {
            *slot = item;
        } else {
            project.services.push(item);
        }
    }
    Ok(())
}

fn parse_ps_json(raw: &str) -> Vec<ComposeService> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.starts_with('[') {
        if let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) {
            return rows.into_iter().filter_map(service_from_ps).collect();
        }
    }
    let mut out = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(service) = service_from_ps(value) {
                out.push(service);
            }
        }
    }
    out
}

fn service_from_ps(value: serde_json::Value) -> Option<ComposeService> {
    let name = value
        .get("Service")
        .or_else(|| value.get("service"))
        .or_else(|| value.get("Name"))
        .and_then(|item| item.as_str())
        .map(str::to_string)?;
    let state = value
        .get("State")
        .or_else(|| value.get("state"))
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    let status = value
        .get("Status")
        .or_else(|| value.get("status"))
        .and_then(|item| item.as_str())
        .map(str::to_string);
    let container = value
        .get("ID")
        .or_else(|| value.get("Id"))
        .or_else(|| value.get("Container"))
        .and_then(|item| item.as_str())
        .map(str::to_string);
    Some(ComposeService {
        name,
        state,
        status,
        ports: extract_ports(&value),
        container,
    })
}

fn extract_ports(value: &serde_json::Value) -> Vec<String> {
    if let Some(text) = value.get("Publishers").and_then(|item| item.as_str()) {
        return split_ports(text);
    }
    if let Some(rows) = value
        .get("Publishers")
        .or_else(|| value.get("Ports"))
        .and_then(|item| item.as_array())
    {
        let mut out = Vec::new();
        for row in rows {
            if let Some(text) = row.as_str() {
                if !text.is_empty() {
                    out.push(text.to_string());
                }
                continue;
            }
            let published = row
                .get("PublishedPort")
                .or_else(|| row.get("publishedPort"))
                .and_then(|item| item.as_u64());
            let target = row
                .get("TargetPort")
                .or_else(|| row.get("targetPort"))
                .and_then(|item| item.as_u64());
            let ip = row
                .get("URL")
                .or_else(|| row.get("url"))
                .and_then(|item| item.as_str())
                .unwrap_or("");
            if let (Some(published), Some(target)) = (published, target) {
                if ip.is_empty() {
                    out.push(format!("{published}->{target}"));
                } else {
                    out.push(format!("{ip}:{published}->{target}"));
                }
            }
        }
        return out;
    }
    if let Some(text) = value.get("Ports").and_then(|item| item.as_str()) {
        return split_ports(text);
    }
    Vec::new()
}

fn split_ports(text: &str) -> Vec<String> {
    text.split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn cached_docker_cli() -> Option<Result<String, String>> {
    let Ok(guard) = DOCKER_CLI.lock() else {
        return None;
    };
    let (at, result) = guard.as_ref()?;
    let ttl = if result.is_ok() {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(60)
    };
    if at.elapsed() < ttl {
        Some(result.clone())
    } else {
        None
    }
}

fn store_docker_cli(result: Result<String, String>) -> Result<String, String> {
    if let Ok(mut guard) = DOCKER_CLI.lock() {
        *guard = Some((std::time::Instant::now(), result.clone()));
    }
    result
}

async fn detect_docker_cli() -> Result<String, String> {
    if let Some(cached) = cached_docker_cli() {
        return cached;
    }
    if run_docker("docker", &["compose".into(), "version".into()], CLI_TIMEOUT)
        .await
        .is_ok()
    {
        return store_docker_cli(Ok("docker".into()));
    }
    match run_docker("docker-compose", &["version".into()], CLI_TIMEOUT).await {
        Ok(_) => store_docker_cli(Ok("docker-compose".into())),
        Err(_) => store_docker_cli(Err("Docker is not available".into())),
    }
}

async fn run_docker(program: &str, args: &[String], max: Duration) -> Result<String, String> {
    let mut cmd = hidden_command(program);
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    match timeout(max, cmd.output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            if output.status.success() {
                Ok(stdout)
            } else {
                let detail = stderr.trim();
                if detail.is_empty() {
                    Err(format!("{program} exited with {}", output.status))
                } else {
                    Err(detail.to_string())
                }
            }
        }
        Ok(Err(err)) => Err(format!("{program}: {err}")),
        Err(_) => Err(format!("{program} timed out")),
    }
}

fn hidden_command(program: &str) -> Command {
    let mut cmd = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", program]);
        #[cfg(windows)]
        {
            command.creation_flags(create_no_window_flags());
        }
        command
    } else {
        Command::new(program)
    };
    cmd.stdin(Stdio::null());
    cmd
}
