//! Project-agnostic anomaly and port-conflict warnings.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::group::{PartRole, RepoGroup};
use crate::health::HealthStatus;
use crate::ports::{
    framework_label, is_all_interfaces, is_ipv6_only, is_lan_only, Listener, PortInfo,
    PortSource,
};
use crate::process::{Run, RunStatus};
use crate::util::now_ms;

const STARTUP_GRACE_MS: i64 = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WarningSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub code: String,
    pub severity: WarningSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

#[derive(Clone)]
struct PartView {
    id: String,
    name: String,
    project: String,
    path: String,
    parent: Option<String>,
    role: PartRole,
    framework: Option<String>,
    ports: Vec<PortInfo>,
}

pub fn evaluate_warnings(groups: &[RepoGroup], runs: &[Run], listeners: &[Listener]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    let parts = part_views(groups);
    let now = now_ms();

    for run in runs.iter().filter(|run| run.status == RunStatus::Running) {
        let part = parts.iter().find(|part| part.id == run.part_id);
        let part_name = part
            .map(|part| part.name.clone())
            .unwrap_or_else(|| run.part_name.clone());
        let project = part
            .map(|part| part.project.clone())
            .unwrap_or_else(|| run.part_name.clone());
        warnings.extend(run_anomalies(run, &part_name, &project, now));
    }

    warnings.extend(actual_conflicts(runs, &parts));
    warnings.extend(foreign_occupancy(groups, runs, &parts, listeners));
    warnings.extend(possible_conflicts(&parts, runs));

    dedupe(warnings)
}

pub fn attach_run_warnings(runs: &mut [Run], warnings: &[Warning]) {
    for run in runs {
        run.warnings = warnings
            .iter()
            .filter(|warning| warning_matches_run(warning, run))
            .cloned()
            .collect();
    }
}

fn warning_matches_run(warning: &Warning, run: &Run) -> bool {
    if warning.run_id.as_deref() == Some(run.id.as_str()) {
        return true;
    }
    warning.part_id.as_deref() == Some(run.part_id.as_str())
        && (warning.run_id.is_none() || warning.run_id.as_deref() == Some(run.id.as_str()))
}

fn part_views(groups: &[RepoGroup]) -> Vec<PartView> {
    let mut parts = Vec::new();
    for group in groups {
        for part in &group.parts {
            parts.push(PartView {
                id: part.id.clone(),
                name: display_part_name(group, part),
                project: group.name.clone(),
                path: part.path.clone(),
                parent: part.parent.clone(),
                role: part.role,
                framework: part.framework.clone(),
                ports: part.ports.clone(),
            });
        }
    }
    parts
}

fn display_part_name(group: &RepoGroup, part: &crate::group::ProjectPart) -> String {
    if part.relative_path == "." || matches!(part.role, PartRole::Root) && group.parts.len() == 1 {
        group.name.clone()
    } else if part.relative_path == "." {
        part.name.clone()
    } else {
        format!("{}/{}", group.name, part.relative_path)
    }
}

fn run_anomalies(run: &Run, part_name: &str, project: &str, now: i64) -> Vec<Warning> {
    let mut warnings = Vec::new();
    let mature = now.saturating_sub(run.started_at) >= STARTUP_GRACE_MS;

    for port in &run.ports {
        if is_ipv6_only(&port.binds) {
            warnings.push(warn(
                "ipv6_only",
                WarningSeverity::Warning,
                format!(
                    "Port {} listens only on ::1; 127.0.0.1 won't connect.",
                    port.port
                ),
                Some("Bind to 127.0.0.1 as well, or set the server host to localhost."),
                Some(port.port),
                Some(run),
                part_name,
                project,
            ));
        }
        if is_all_interfaces(&port.binds) {
            warnings.push(warn(
                "bound_all_interfaces",
                WarningSeverity::Info,
                format!(
                    "Port {} is bound to all interfaces (0.0.0.0), so it is reachable on the LAN.",
                    port.port
                ),
                Some("Bind to 127.0.0.1 if this should stay local."),
                Some(port.port),
                Some(run),
                part_name,
                project,
            ));
        }
        if is_lan_only(&port.binds) {
            let addrs = port.binds.join(", ");
            warnings.push(warn(
                "lan_only",
                WarningSeverity::Warning,
                format!("Port {} listens only on {addrs}, not on localhost.", port.port),
                Some("Bind to 127.0.0.1 so local tools can connect."),
                Some(port.port),
                Some(run),
                part_name,
                project,
            ));
        }
        if run
            .health
            .ports
            .iter()
            .any(|item| item.port == port.port && item.status == HealthStatus::Fail)
        {
            warnings.push(warn(
                "health_fail",
                WarningSeverity::Warning,
                format!("Port {} is not responding.", port.port),
                Some("The process is listening, but the health probe failed."),
                Some(port.port),
                Some(run),
                part_name,
                project,
            ));
        }
    }

    if !mature {
        return warnings;
    }

    let live: BTreeSet<u16> = run.ports.iter().map(|item| item.port).collect();
    let expected: BTreeSet<u16> = run.expected_ports.iter().map(|item| item.port).collect();
    if live.is_empty() && !expected.is_empty() {
        warnings.push(warn(
            "no_port_when_expected",
            WarningSeverity::Info,
            format!(
                "{} has been running but is not listening on any port (expected {}).",
                part_name,
                format_ports(&expected)
            ),
            Some("If this app should serve HTTP, check the logs. Some tools never bind a port."),
            expected.iter().copied().next(),
            Some(run),
            part_name,
            project,
        ));
        return warnings;
    }

    if live.is_empty() || expected.is_empty() {
        return warnings;
    }

    let missing: BTreeSet<u16> = expected.difference(&live).copied().collect();
    let extra: BTreeSet<u16> = live.difference(&expected).copied().collect();
    if !missing.is_empty() && live.is_disjoint(&expected) {
        warnings.push(warn(
            "port_mismatch",
            WarningSeverity::Warning,
            format!(
                "Expected {}, but {} is listening on {}. The expected port may already have been taken.",
                format_ports(&missing),
                part_name,
                format_ports(&extra)
            ),
            Some("Set a unique fixed port, or free the original port."),
            missing.iter().copied().next(),
            Some(run),
            part_name,
            project,
        ));
    } else {
        for port in missing {
            let source = run
                .expected_ports
                .iter()
                .find(|item| item.port == port)
                .map(|item| item.source);
            if source == Some(PortSource::Default) {
                continue;
            }
            warnings.push(warn(
                "expected_port_missing",
                WarningSeverity::Warning,
                format!(
                    "Configured port {} is not listening on {}.",
                    port, part_name
                ),
                Some("The process may still be starting, or another service took the port."),
                Some(port),
                Some(run),
                part_name,
                project,
            ));
        }
    }

    warnings
}

fn actual_conflicts(runs: &[Run], parts: &[PartView]) -> Vec<Warning> {
    let mut by_port: BTreeMap<u16, Vec<&Run>> = BTreeMap::new();
    for run in runs.iter().filter(|run| run.status == RunStatus::Running) {
        let mut seen = HashSet::new();
        for port in &run.ports {
            if seen.insert(port.port) {
                by_port.entry(port.port).or_default().push(run);
            }
        }
    }

    let mut warnings = Vec::new();
    for (port, claimants) in by_port {
        let claimants = drop_ancestor_runs(claimants, parts);
        if claimants.len() < 2 {
            continue;
        }
        let names = join_names(claimants.iter().map(|run| part_name_of(parts, run)));
        let message = format!("Port {port} is used by {names} at the same time.");
        for run in claimants {
            let name = part_name_of(parts, run);
            let project = parts
                .iter()
                .find(|part| part.id == run.part_id)
                .map(|part| part.project.as_str())
                .unwrap_or(run.part_name.as_str());
            warnings.push(warn(
                "port_conflict",
                WarningSeverity::Error,
                message.clone(),
                Some("Stop one of the apps or give each a unique port."),
                Some(port),
                Some(run),
                &name,
                project,
            ));
        }
    }
    warnings
}

fn foreign_occupancy(
    _groups: &[RepoGroup],
    runs: &[Run],
    parts: &[PartView],
    listeners: &[Listener],
) -> Vec<Warning> {
    let mut owned: HashSet<u32> = HashSet::new();
    for run in runs.iter().filter(|run| run.status == RunStatus::Running) {
        if let Some(pid) = run.pid {
            owned.insert(pid);
        }
        for port in &run.ports {
            if let Some(pid) = port.pid {
                owned.insert(pid);
            }
        }
    }

    let mut foreign: BTreeMap<u16, &Listener> = BTreeMap::new();
    for listener in listeners {
        if owned.contains(&listener.pid) {
            continue;
        }
        foreign.entry(listener.port).or_insert(listener);
    }

    let mut warnings = Vec::new();
    for (port, listener) in foreign {
        let process = listener
            .process
            .as_deref()
            .unwrap_or("an unknown process");
        let occupant = format!("{process} (pid {})", listener.pid);

        for run in runs.iter().filter(|run| run.status == RunStatus::Running) {
            let expects = run.expected_ports.iter().any(|item| {
                item.port == port && item.source != PortSource::Default
            });
            let already_bound = run.ports.iter().any(|item| item.port == port);
            if expects && !already_bound {
                let name = part_name_of(parts, run);
                let project = parts
                    .iter()
                    .find(|part| part.id == run.part_id)
                    .map(|part| part.project.as_str())
                    .unwrap_or(run.part_name.as_str());
                warnings.push(warn(
                    "port_occupied_foreign",
                    WarningSeverity::Warning,
                    format!("Port {port} expected by {name} is in use by {occupant}."),
                    Some("Stop that process, or give this app a different port."),
                    Some(port),
                    Some(run),
                    &name,
                    project,
                ));
            }
        }

        for part in parts {
            let explicit = part
                .ports
                .iter()
                .any(|item| item.port == port && is_explicit(item));
            if !explicit {
                continue;
            }
            let already = warnings.iter().any(|warning: &Warning| {
                warning.code == "port_occupied_foreign"
                    && warning.port == Some(port)
                    && warning.part_id.as_deref() == Some(part.id.as_str())
            });
            if already {
                continue;
            }
            let running_here = runs.iter().any(|run| {
                run.status == RunStatus::Running
                    && run.part_id == part.id
                    && run.ports.iter().any(|item| item.port == port)
            });
            if running_here {
                continue;
            }
            let taken_by_orbitry = runs.iter().any(|run| {
                run.status == RunStatus::Running && run.ports.iter().any(|item| item.port == port)
            });
            if taken_by_orbitry {
                continue;
            }
            warnings.push(warn(
                "port_occupied_foreign",
                WarningSeverity::Warning,
                format!(
                    "Port {port} expected by {} is in use by {occupant}.",
                    part.name
                ),
                Some("Start may fail until that process exits, or set a different port."),
                Some(port),
                None,
                &part.name,
                &part.project,
            ));
            if let Some(last) = warnings.last_mut() {
                last.part_id = Some(part.id.clone());
            }
        }
    }

    warnings
}

fn possible_conflicts(parts: &[PartView], runs: &[Run]) -> Vec<Warning> {
    let mut claims: BTreeMap<u16, Vec<Claim>> = BTreeMap::new();
    for part in parts {
        if matches!(part.role, PartRole::Lib) {
            continue;
        }
        for port in &part.ports {
            claims.entry(port.port).or_default().push(Claim {
                part_id: part.id.clone(),
                name: part.name.clone(),
                project: part.project.clone(),
                framework: part.framework.clone(),
                source: port.source,
                running: runs.iter().any(|run| {
                    run.status == RunStatus::Running
                        && run.part_id == part.id
                        && run.ports.iter().any(|item| item.port == port.port)
                }),
            });
        }
    }

    let mut warnings = Vec::new();
    for (port, claimants) in claims {
        if claimants.len() < 2 {
            continue;
        }
        let unique_parts: BTreeSet<&str> = independent_part_ids(&claimants, parts);
        if unique_parts.len() < 2 {
            continue;
        }
        let actual_running = claimants.iter().filter(|item| item.running).count();
        if actual_running >= 2 {
            continue;
        }

        let defaulting: Vec<&Claim> = claimants
            .iter()
            .filter(|item| item.source == PortSource::Default)
            .filter(|item| {
                !parts.iter().any(|part| {
                    part.id == item.part_id && part.ports.iter().any(is_explicit)
                })
            })
            .collect();

        if !defaulting.is_empty() {
            for item in &defaulting {
                let others = claimants
                    .iter()
                    .filter(|other| other.part_id != item.part_id)
                    .map(|other| other.name.as_str());
                let other_names = join_str(others);
                if other_names.is_empty() {
                    continue;
                }
                let framework = item
                    .framework
                    .as_deref()
                    .map(framework_label)
                    .unwrap_or_else(|| "framework".to_string());
                let already = warnings.iter().any(|warning: &Warning| {
                    warning.code == "possible_port_conflict"
                        && warning.port == Some(port)
                        && warning.part_id.as_deref() == Some(item.part_id.as_str())
                });
                if already {
                    continue;
                }
                let mut warning = warn(
                    "possible_port_conflict",
                    WarningSeverity::Info,
                    format!(
                        "No explicit port; {framework} default {port} is also used by {other_names}. Set a fixed port."
                    ),
                    Some("Set a unique port in the app config or start command so the two cannot collide."),
                    Some(port),
                    None,
                    &item.name,
                    &item.project,
                );
                warning.part_id = Some(item.part_id.clone());
                warnings.push(warning);
            }
            continue;
        }

        let explicit: Vec<&Claim> = claimants.iter().filter(|item| is_source_explicit(item.source)).collect();
        let unique_explicit: BTreeSet<&str> = independent_part_ids(explicit.iter().copied(), parts);
        if unique_explicit.len() >= 2 {
            let names = join_str(explicit.iter().map(|item| item.name.as_str()));
            let mut seen = HashSet::new();
            for item in explicit {
                if !seen.insert(item.part_id.as_str()) {
                    continue;
                }
                let mut warning = warn(
                    "possible_port_conflict",
                    WarningSeverity::Info,
                    format!("Port {port} is configured on {names}. Only one can bind it at a time."),
                    Some("Give each app a unique port before starting both."),
                    Some(port),
                    None,
                    &item.name,
                    &item.project,
                );
                warning.part_id = Some(item.part_id.clone());
                warnings.push(warning);
            }
        }
    }
    warnings
}

struct Claim {
    part_id: String,
    name: String,
    project: String,
    framework: Option<String>,
    source: PortSource,
    running: bool,
}

fn independent_part_ids<'a, I>(claimants: I, parts: &[PartView]) -> BTreeSet<&'a str>
where
    I: IntoIterator<Item = &'a Claim>,
{
    let claimants: Vec<&Claim> = claimants.into_iter().collect();
    claimants
        .iter()
        .filter(|item| {
            !claimants.iter().any(|other| {
                other.part_id != item.part_id
                    && is_ancestor_of(&item.part_id, &other.part_id, parts)
            })
        })
        .map(|item| item.part_id.as_str())
        .collect()
}

fn drop_ancestor_runs<'a>(runs: Vec<&'a Run>, parts: &[PartView]) -> Vec<&'a Run> {
    runs.iter()
        .copied()
        .filter(|run| {
            !runs.iter().any(|other| {
                other.part_id != run.part_id
                    && is_ancestor_of(&run.part_id, &other.part_id, parts)
            })
        })
        .collect()
}

fn is_ancestor_of(ancestor_id: &str, descendant_id: &str, parts: &[PartView]) -> bool {
    let Some(ancestor) = parts.iter().find(|part| part.id == ancestor_id) else {
        return false;
    };
    let Some(descendant) = parts.iter().find(|part| part.id == descendant_id) else {
        return false;
    };
    if descendant.parent.as_deref() == Some(ancestor.id.as_str()) {
        return true;
    }
    is_path_ancestor(&ancestor.path, &descendant.path)
}

fn is_path_ancestor(parent: &str, child: &str) -> bool {
    let parent = crate::config::normalize_path_str(parent);
    let child = crate::config::normalize_path_str(child);
    if child.len() <= parent.len() {
        return false;
    }
    if child.as_bytes().get(parent.len()) != Some(&b'\\') {
        return false;
    }
    if cfg!(windows) {
        child[..parent.len()].eq_ignore_ascii_case(&parent)
    } else {
        child.starts_with(&parent)
    }
}

fn is_explicit(port: &PortInfo) -> bool {
    is_source_explicit(port.source)
}

fn is_source_explicit(source: PortSource) -> bool {
    matches!(
        source,
        PortSource::Config | PortSource::Env | PortSource::Flag | PortSource::Bridge
    )
}

fn part_name_of(parts: &[PartView], run: &Run) -> String {
    parts
        .iter()
        .find(|part| part.id == run.part_id)
        .map(|part| part.name.clone())
        .unwrap_or_else(|| run.part_name.clone())
}

fn format_ports(ports: &BTreeSet<u16>) -> String {
    let items: Vec<String> = ports.iter().map(|port| format!(":{port}")).collect();
    match items.as_slice() {
        [] => "a port".into(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            format!("{} and {last}", rest.join(", "))
        }
    }
}

fn join_names(names: impl IntoIterator<Item = String>) -> String {
    let owned: Vec<String> = names.into_iter().collect();
    join_str(owned.iter().map(String::as_str))
}

fn join_str<'a, I>(names: I) -> String
where
    I: Iterator<Item = &'a str>,
{
    let mut unique = Vec::new();
    for name in names {
        if !unique.iter().any(|item: &&str| *item == name) {
            unique.push(name);
        }
    }
    match unique.as_slice() {
        [] => String::new(),
        [one] => (*one).to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, rest) = unique.split_last().unwrap();
            format!("{} and {last}", rest.join(", "))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn warn(
    code: &str,
    severity: WarningSeverity,
    message: String,
    hint: Option<&str>,
    port: Option<u16>,
    run: Option<&Run>,
    part_name: &str,
    project: &str,
) -> Warning {
    Warning {
        code: code.to_string(),
        severity,
        message,
        hint: hint.map(str::to_string),
        port,
        run_id: run.map(|item| item.id.clone()),
        part_id: run.map(|item| item.part_id.clone()),
        part_name: Some(part_name.to_string()),
        project: Some(project.to_string()),
    }
}

fn dedupe(mut warnings: Vec<Warning>) -> Vec<Warning> {
    let mut seen = HashSet::new();
    warnings.retain(|warning| {
        seen.insert((
            warning.code.clone(),
            warning.port,
            warning.part_id.clone(),
            warning.run_id.clone(),
            warning.message.clone(),
        ))
    });
    warnings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.port.cmp(&b.port))
            .then(a.code.cmp(&b.code))
            .then(a.message.cmp(&b.message))
    });
    warnings
}
