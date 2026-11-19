//! Script runners: package-manager detection, process-tree kill, and run types.
//!
//! Child processes use piped stdout/stderr rather than a pty. On Windows,
//! ConPTY plus npm trees and a reliable tree-kill is painful, and a hidden
//! `cmd /C` with `CREATE_NO_WINDOW` already avoids new console windows.

use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};

use crate::health::HealthInfo;
use crate::ports::{localhost_url, merge_ports, preferred_port_name, PortInfo, PortSource};
use crate::warnings::Warning;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub part_id: String,
    pub part_name: String,
    pub part_path: String,
    pub script: String,
    pub command: String,
    pub cwd: String,
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    /// True when the user asked Orbitry to stop this run. Present from 0.13.0;
    /// older clients ignore the field.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_requested: bool,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub ports: Vec<PortInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_ports: Vec<PortInfo>,
    pub health: HealthInfo,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    #[serde(default, skip_serializing)]
    pub log_ports: Vec<PortInfo>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Run {
    pub fn merge_ports(&mut self, incoming: impl IntoIterator<Item = PortInfo>) {
        merge_ports(&mut self.ports, incoming);
        self.retain_display_ports();
    }

    pub fn merge_log_ports(&mut self, incoming: impl IntoIterator<Item = PortInfo>) {
        merge_ports(&mut self.log_ports, incoming);
    }

    pub fn apply_live_ports(&mut self, live: Vec<PortInfo>) {
        let mut live = live;
        for item in &mut live {
            item.name = preferred_port_name(
                item.port,
                item.name.as_deref(),
                self.expected_ports.iter().chain(self.log_ports.iter()),
            );
            if item.url.is_none() {
                item.url = Some(localhost_url(item.port, None));
            }
        }
        self.ports.retain(|port| port.source != PortSource::Process);
        merge_ports(&mut self.ports, live);
        self.retain_display_ports();
    }

    fn retain_display_ports(&mut self) {
        if self.status != RunStatus::Running {
            return;
        }
        if self.ports.iter().any(|port| port.source == PortSource::Process) {
            self.ports.retain(|port| port.source == PortSource::Process);
        } else {
            self.ports.clear();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub stream: LogStream,
    pub text: String,
    pub ts: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunEvent {
    Line {
        seq: u64,
        stream: LogStream,
        text: String,
        ts: i64,
    },
    Status {
        run: Run,
    },
}

pub use crate::util::now_ms;

/// Pick npm, pnpm, yarn, or bun from lockfiles, walking up to the git root.
pub fn detect_package_manager(start: &Path) -> &'static str {
    for dir in start.ancestors() {
        if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
            return "bun";
        }
        if dir.join("pnpm-lock.yaml").exists() {
            return "pnpm";
        }
        if dir.join("yarn.lock").exists() {
            return "yarn";
        }
        if dir.join("package-lock.json").exists() || dir.join("npm-shrinkwrap.json").exists() {
            return "npm";
        }
        if dir.join(".git").exists() {
            break;
        }
    }
    "npm"
}

pub fn run_command_label(pm: &str, script: &str) -> String {
    format!("{pm} run {script}")
}

/// `pm run <script>` in `cwd`, without opening a new console window.
pub fn script_command(pm: &str, script: &str, cwd: &Path) -> tokio::process::Command {
    let mut cmd = if cfg!(windows) {
        let mut command = tokio::process::Command::new("cmd");
        command.args(["/C", pm, "run", script]);
        #[cfg(windows)]
        {
            command.creation_flags(crate::logpump::detach_creation_flags());
        }
        command
    } else {
        let mut command = tokio::process::Command::new(pm);
        command.args(["run", script]);
        crate::logpump::apply_detach_flags_tokio(&mut command);
        command
    };
    cmd.current_dir(cwd)
        .env("FORCE_COLOR", "1")
        .env("npm_config_color", "always")
        .env("TERM", "xterm-256color")
        .env("PATH", path_with_node_bins(cwd))
        .stdin(Stdio::null())
        .kill_on_drop(false);
    cmd
}

pub fn install_command_label(pm: &str) -> String {
    format!("{pm} install")
}

/// `pm install` in `cwd`, without opening a new console window.
pub fn install_command(pm: &str, cwd: &Path) -> tokio::process::Command {
    let mut cmd = if cfg!(windows) {
        let mut command = tokio::process::Command::new("cmd");
        command.args(["/C", pm, "install"]);
        #[cfg(windows)]
        {
            command.creation_flags(crate::logpump::detach_creation_flags());
        }
        command
    } else {
        let mut command = tokio::process::Command::new(pm);
        command.args(["install"]);
        crate::logpump::apply_detach_flags_tokio(&mut command);
        command
    };
    cmd.current_dir(cwd)
        .env("FORCE_COLOR", "1")
        .env("npm_config_color", "always")
        .env("TERM", "xterm-256color")
        .env("PATH", path_with_node_bins(cwd))
        .stdin(Stdio::null())
        .kill_on_drop(false);
    cmd
}

/// Prepend ancestor `node_modules/.bin` folders so hoisted workspace binaries resolve.
fn path_with_node_bins(start: &Path) -> std::ffi::OsString {
    let mut bins = Vec::new();
    for dir in start.ancestors() {
        let bin = dir.join("node_modules").join(".bin");
        if bin.is_dir() {
            bins.push(bin);
        }
        if dir.join(".git").exists() {
            break;
        }
    }
    let mut path = std::env::var_os("PATH").unwrap_or_default();
    if bins.is_empty() {
        return path;
    }
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut prefixed = std::ffi::OsString::new();
    for bin in bins {
        if !prefixed.is_empty() {
            prefixed.push(sep);
        }
        prefixed.push(bin);
    }
    if !path.is_empty() {
        prefixed.push(sep);
        prefixed.push(&path);
        path = prefixed;
    } else {
        path = prefixed;
    }
    path
}

pub fn kill_process_tree(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| format!("taskkill {pid}: {err}"))?;
        Ok(())
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
        Ok(())
    }
}

pub fn kill_process_tree_force(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        kill_process_tree(pid)
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .status();
        Ok(())
    }
}

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;


#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::HealthInfo;
    use crate::ports::{PortInfo, PortSource};

    fn named(port: u16, source: PortSource, name: Option<&str>) -> PortInfo {
        PortInfo::named(port, source, name.map(str::to_string)).expect("plausible port")
    }

    fn run_with(expected: Vec<PortInfo>, logs: Vec<PortInfo>) -> Run {
        Run {
            id: "run".into(),
            part_id: "part".into(),
            part_name: "part".into(),
            part_path: "/tmp/part".into(),
            script: "dev".into(),
            command: "npm run dev".into(),
            cwd: "/tmp/part".into(),
            pid: Some(1),
            process_start: None,
            log_path: None,
            status: RunStatus::Running,
            exit_code: None,
            stop_requested: false,
            started_at: 0,
            ended_at: None,
            ports: Vec::new(),
            expected_ports: expected,
            health: HealthInfo::unknown(),
            warnings: Vec::new(),
            log_ports: logs,
        }
    }

    #[test]
    fn live_port_names_prefer_config_over_logs() {
        let mut run = run_with(
            vec![
                named(3000, PortSource::Default, Some("server")),
                named(5178, PortSource::Config, Some("dashboard")),
            ],
            vec![
                named(5177, PortSource::Log, Some("server")),
                named(5178, PortSource::Log, Some("server")),
            ],
        );
        run.apply_live_ports(vec![
            named(5177, PortSource::Process, Some("server")),
            named(5178, PortSource::Process, Some("server")),
        ]);
        let names: Vec<(u16, Option<&str>)> = run
            .ports
            .iter()
            .map(|port| (port.port, port.name.as_deref()))
            .collect();
        assert_eq!(
            names,
            vec![(5177, Some("server")), (5178, Some("dashboard"))],
            "config name must win for the dashboard port; the API port keeps the server label"
        );

        let mut monorepo = run_with(
            vec![
                named(3190, PortSource::Config, Some("backend")),
                named(5190, PortSource::Config, Some("frontend")),
            ],
            vec![
                named(3190, PortSource::Log, Some("backend")),
                named(5190, PortSource::Log, Some("frontend")),
            ],
        );
        monorepo.apply_live_ports(vec![
            named(3190, PortSource::Process, Some("backend")),
            named(5190, PortSource::Process, Some("frontend")),
        ]);
        let names: Vec<(u16, Option<&str>)> = monorepo
            .ports
            .iter()
            .map(|port| (port.port, port.name.as_deref()))
            .collect();
        assert_eq!(names, vec![(3190, Some("backend")), (5190, Some("frontend"))]);
    }
}
