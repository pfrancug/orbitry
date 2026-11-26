//! Per-app autostart after Orbitry itself starts.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{paths_equal, AutostartApp, Config, ScriptKind};
use crate::group::{ProjectPart, RepoGroup};
use crate::ports::{
    apply_port_overrides, detect_expected_ports, merge_ports, Listener, ListeningSnapshot, PortInfo,
};
use crate::process::{Run, RunStatus};
use crate::scripts::DetectedScript;
use crate::util::now_ms;
use crate::warnings::{Warning, WarningSeverity};
use crate::{ProcessManager, StartError};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppAutostartState {
    #[default]
    Idle,
    Running,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppAutostartOutcome {
    Started,
    Skipped,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAutostartEvent {
    pub part_id: String,
    pub part_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    pub outcome: AppAutostartOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAutostartReport {
    pub state: AppAutostartState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub started: Vec<AppAutostartEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<AppAutostartEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked: Vec<AppAutostartEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<AppAutostartEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
}

impl AppAutostartReport {
    pub fn running() -> Self {
        Self {
            state: AppAutostartState::Running,
            ..Self::default()
        }
    }

    fn push(&mut self, event: AppAutostartEvent) {
        match event.outcome {
            AppAutostartOutcome::Started => self.started.push(event),
            AppAutostartOutcome::Skipped => self.skipped.push(event),
            AppAutostartOutcome::Blocked => self.blocked.push(event),
            AppAutostartOutcome::Failed => self.failed.push(event),
        }
    }

    fn finish(&mut self) {
        self.state = AppAutostartState::Done;
        self.completed_at = Some(now_ms());
    }

    pub fn is_empty(&self) -> bool {
        self.started.is_empty()
            && self.skipped.is_empty()
            && self.blocked.is_empty()
            && self.failed.is_empty()
    }
}

/// Prefer first dev, else first preview (historical behaviour). If the part only
/// has other runnable scripts (e.g. AutoHotkey), fall back to the first script.
pub fn default_autostart_script(part: &ProjectPart) -> Option<&DetectedScript> {
    part.scripts
        .iter()
        .find(|script| matches!(script.kind, ScriptKind::Dev))
        .or_else(|| {
            part.scripts
                .iter()
                .find(|script| matches!(script.kind, ScriptKind::Preview))
        })
        .or_else(|| part.scripts.first())
}

/// Resolve the script named in an autostart entry, or the historical default when
/// the stored script is empty (legacy bare ids).
pub fn resolve_autostart_script<'a>(
    part: &'a ProjectPart,
    script_name: &str,
) -> Option<&'a DetectedScript> {
    let wanted = script_name.trim();
    if !wanted.is_empty() {
        return part
            .scripts
            .iter()
            .find(|script| script.name == wanted);
    }
    default_autostart_script(part)
}

/// Drop autostart entries for parts with no runnable scripts, fill empty script
/// fields with the historical default pick, and drop entries whose script is gone.
/// Returns true when the list changed.
pub fn prune_autostart_apps(config: &mut Config, groups: &[RepoGroup]) -> bool {
    let before = config.autostart_apps.clone();
    let mut next = Vec::new();
    for entry in &before {
        let Some(part) = groups
            .iter()
            .flat_map(|group| group.parts.iter())
            .find(|part| paths_equal(&part.id, &entry.part_id))
        else {
            // Keep temporarily missing parts so a flaky scan does not wipe them.
            next.push(entry.clone());
            continue;
        };
        if part.scripts.is_empty() {
            continue;
        }
        let Some(script) = resolve_autostart_script(part, &entry.script) else {
            continue;
        };
        next.push(AutostartApp::new(part.id.clone(), script.name.clone()));
    }
    config.autostart_apps = next;
    config.normalize_autostart_apps();
    config.autostart_apps != before
}

/// Listeners currently bound to any of the given ports (IPv4 and IPv6).
pub fn occupied_listeners<'a>(
    snapshot: &'a ListeningSnapshot,
    ports: &[PortInfo],
) -> Vec<&'a Listener> {
    let wanted: BTreeSet<u16> = ports.iter().map(|port| port.port).collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut seen: BTreeSet<(u16, u32)> = BTreeSet::new();
    let mut out = Vec::new();
    for listener in &snapshot.listeners {
        if !wanted.contains(&listener.port) {
            continue;
        }
        if seen.insert((listener.port, listener.pid)) {
            out.push(listener);
        }
    }
    out
}

pub fn format_occupant(listener: &Listener) -> String {
    let process = listener
        .process
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or("an unknown process");
    format!("{process} (pid {})", listener.pid)
}

struct PartRef<'a> {
    part: &'a ProjectPart,
    name: String,
    project: String,
}

/// Start configured apps after a fresh scan. Call only once per Orbitry process,
/// and only when the OS-level "start with the system" entry is enabled.
pub async fn run_app_autostart(
    manager: &Arc<ProcessManager>,
    groups: &[crate::RepoGroup],
    config: &Config,
    should_stop: impl Fn() -> bool,
) -> (AppAutostartReport, Vec<Warning>) {
    let mut report = AppAutostartReport::running();
    let mut warnings = Vec::new();
    let parts: Vec<PartRef<'_>> = groups
        .iter()
        .flat_map(|group| {
            group.parts.iter().map(|part| PartRef {
                part,
                name: display_name(group, part),
                project: group.name.clone(),
            })
        })
        .collect();

    let mut first = true;
    for entry in &config.autostart_apps {
        if should_stop() {
            break;
        }
        let part_id = &entry.part_id;
        let Some(item) = find_part(&parts, part_id) else {
            let reason = "not in the latest scan".to_string();
            eprintln!("orbitry: autostart: skipped {part_id} ({reason})");
            report.push(AppAutostartEvent {
                part_id: part_id.clone(),
                part_name: part_id.clone(),
                script: None,
                outcome: AppAutostartOutcome::Skipped,
                reason: Some(reason),
                port: None,
                process: None,
            });
            continue;
        };
        let part = item.part;
        let name = item.name.clone();

        if !first {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        first = false;

        if already_running(manager, &part.id).await {
            let reason = "already running".to_string();
            eprintln!("orbitry: autostart: skipped {name} ({reason})");
            report.push(AppAutostartEvent {
                part_id: part.id.clone(),
                part_name: name,
                script: None,
                outcome: AppAutostartOutcome::Skipped,
                reason: Some(reason),
                port: None,
                process: None,
            });
            continue;
        }

        let Some(script) = resolve_autostart_script(part, &entry.script) else {
            let reason = if part.scripts.is_empty() {
                "no runnable scripts".to_string()
            } else if entry.script.trim().is_empty() {
                "no runnable scripts".to_string()
            } else {
                format!("script '{}' was not found", entry.script.trim())
            };
            eprintln!("orbitry: autostart: skipped {name} ({reason})");
            report.push(AppAutostartEvent {
                part_id: part.id.clone(),
                part_name: name,
                script: None,
                outcome: AppAutostartOutcome::Skipped,
                reason: Some(reason),
                port: None,
                process: None,
            });
            continue;
        };

        let expected = expected_ports(part, &script.name, &config.port_overrides);
        let snapshot = tokio::task::spawn_blocking(ListeningSnapshot::collect)
            .await
            .unwrap_or_else(|_| ListeningSnapshot::default());
        let occupied = occupied_listeners(&snapshot, &expected);
        if !occupied.is_empty() {
            for listener in occupied {
                let occupant = format_occupant(listener);
                let reason = format!(
                    "port {} is in use by {occupant}",
                    listener.port
                );
                eprintln!("orbitry: autostart: blocked {name} ({reason})");
                warnings.push(Warning {
                    code: "autostart_port_occupied".into(),
                    severity: WarningSeverity::Warning,
                    message: format!(
                        "Autostart skipped {name}: port {} is in use by {occupant}.",
                        listener.port
                    ),
                    hint: Some(
                        "Stop that process, or give this app a different port.".into(),
                    ),
                    port: Some(listener.port),
                    run_id: None,
                    part_id: Some(part.id.clone()),
                    part_name: Some(name.clone()),
                    project: Some(item.project.clone()),
                });
                report.push(AppAutostartEvent {
                    part_id: part.id.clone(),
                    part_name: name.clone(),
                    script: Some(script.name.clone()),
                    outcome: AppAutostartOutcome::Blocked,
                    reason: Some(reason),
                    port: Some(listener.port),
                    process: Some(occupant),
                });
            }
            continue;
        }

        eprintln!(
            "orbitry: autostart: starting {name} ({})",
            script.name
        );
        match manager
            .start(
                part.id.clone(),
                part.name.clone(),
                part.path.clone(),
                script.name.clone(),
                script.cwd.clone(),
                script.source,
                Some(script.command.clone()),
            )
            .await
        {
            Ok(run) => {
                let pid = run
                    .pid
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "?".into());
                eprintln!(
                    "orbitry: autostart: started {name} ({}, pid {pid})",
                    script.name
                );
                report.push(AppAutostartEvent {
                    part_id: part.id.clone(),
                    part_name: name,
                    script: Some(script.name.clone()),
                    outcome: AppAutostartOutcome::Started,
                    reason: None,
                    port: None,
                    process: None,
                });
            }
            Err(StartError::AlreadyRunning) => {
                let reason = "already running".to_string();
                eprintln!("orbitry: autostart: skipped {name} ({reason})");
                report.push(AppAutostartEvent {
                    part_id: part.id.clone(),
                    part_name: name,
                    script: Some(script.name.clone()),
                    outcome: AppAutostartOutcome::Skipped,
                    reason: Some(reason),
                    port: None,
                    process: None,
                });
            }
            Err(StartError::Message(err)) => {
                eprintln!("orbitry: autostart: failed {name}: {err}");
                report.push(AppAutostartEvent {
                    part_id: part.id.clone(),
                    part_name: name,
                    script: Some(script.name.clone()),
                    outcome: AppAutostartOutcome::Failed,
                    reason: Some(err),
                    port: None,
                    process: None,
                });
            }
        }
    }

    report.finish();
    (report, warnings)
}

async fn already_running(manager: &ProcessManager, part_id: &str) -> bool {
    manager.list_cached().await.iter().any(|run: &Run| {
        run.status == RunStatus::Running && paths_equal(&run.part_id, part_id)
    })
}

fn expected_ports(part: &ProjectPart, script: &str, overrides: &[crate::PortOverride]) -> Vec<PortInfo> {
    let mut ports = detect_expected_ports(
        Path::new(&part.path),
        &part.scripts,
        Some(script),
        part.framework.as_deref(),
    );
    merge_ports(&mut ports, part.ports.clone());
    apply_port_overrides(&mut ports, &part.id, overrides);
    ports
}

fn display_name(group: &crate::RepoGroup, part: &ProjectPart) -> String {
    if part.relative_path == "."
        || (matches!(part.role, crate::PartRole::Root) && group.parts.len() == 1)
    {
        group.name.clone()
    } else if part.name.contains('/') || part.name.contains('\\') {
        part.name.clone()
    } else {
        format!("{}/{}", group.name, part.name)
    }
}

fn find_part<'a>(parts: &'a [PartRef<'a>], id: &str) -> Option<&'a PartRef<'a>> {
    parts
        .iter()
        .find(|item| paths_equal(&item.part.id, id) || paths_equal(&item.part.path, id))
}
