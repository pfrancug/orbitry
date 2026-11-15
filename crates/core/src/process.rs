//! Script runners: package-manager detection, process-tree kill, and run types.
//!
//! Child processes use piped stdout/stderr rather than a pty. On Windows,
//! ConPTY plus npm trees and a reliable tree-kill is painful, and a hidden
//! `cmd /C` with `CREATE_NO_WINDOW` already avoids new console windows.

use std::path::Path;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

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
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
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

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

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
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    } else {
        let mut command = tokio::process::Command::new(pm);
        command.args(["run", script]);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        command
    };
    cmd.current_dir(cwd)
        .env("FORCE_COLOR", "1")
        .env("npm_config_color", "always")
        .env("TERM", "xterm-256color")
        .env("PATH", path_with_node_bins(cwd))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

#[cfg(windows)]
pub struct WinJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for WinJob {}

#[cfg(windows)]
unsafe impl Sync for WinJob {}

#[cfg(windows)]
impl WinJob {
    pub fn new() -> Option<Self> {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let handle: HANDLE = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                CloseHandle(handle);
                return None;
            }
            Some(Self { handle })
        }
    }

    pub fn assign(&self, pid: u32) -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        unsafe {
            let proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if proc.is_null() {
                return false;
            }
            let ok = AssignProcessToJobObject(self.handle, proc);
            CloseHandle(proc);
            ok != 0
        }
    }
}

#[cfg(windows)]
impl Drop for WinJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}
