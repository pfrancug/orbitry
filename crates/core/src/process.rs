//! Script runners: package-manager detection, process-tree kill, and run types.
//!
//! Child processes use piped stdout/stderr rather than a pty. On Windows,
//! ConPTY plus npm trees and a reliable tree-kill is painful, and a hidden
//! `cmd /C` with `CREATE_NO_WINDOW` already avoids new console windows.

use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};

use crate::health::HealthInfo;
use crate::ports::{localhost_url, merge_ports, PortInfo, PortSource};
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
            if item.name.is_none() {
                item.name = self
                    .log_ports
                    .iter()
                    .chain(self.expected_ports.iter())
                    .find(|port| port.port == item.port)
                    .and_then(|port| port.name.clone());
            }
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
