//! In-process manager for running detected scripts and streaming their logs.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::{broadcast, Mutex};

use crate::config::PortOverride;
use crate::display_path;
use crate::health::{check_health, HealthInfo, HealthStatus};
use crate::logpump::apply_detach_flags;
#[cfg(windows)]
use crate::logpump::{detach_creation_flags, detach_creation_flags_no_breakaway};
use crate::logs::{
    append_line, atomic_write, cleanup_logs, ensure_logs_dir, file_cap_bytes, run_log_path,
    run_log_rotated_path, run_system_log_path, runs_path, write_log_limit, FinishedLog,
    DEFAULT_LOG_LIMIT_MB,
};
use crate::metrics::{pid_still_same, process_start_time, RunPidIndex};
use crate::ahk::{parse_requires, resolve_interpreter, AhkVersion};
use crate::ports::{apply_port_overrides, detect_expected_ports, ports_from_log, ListeningSnapshot};
use crate::scripts::ScriptSource;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::process::{
    ahk_command, ahk_command_label, detect_package_manager, install_command, install_command_label,
    kill_process_tree, kill_process_tree_force, now_ms, run_command_label, script_command, LogLine,
    LogStream, Run, RunEvent, RunStatus,
};

pub const LOG_BUFFER_CAP: usize = 2000;
const FINISHED_CAP: usize = 50;
const BROADCAST_CAP: usize = 256;
const TAIL_POLL: Duration = Duration::from_millis(150);

pub struct ProcessManager {
    inner: Mutex<Inner>,
    log_limit_mb: AtomicU32,
    pids: RunPidIndex,
}

struct Inner {
    runs: HashMap<String, RunRecord>,
    overrides: Vec<PortOverride>,
}

struct RunRecord {
    info: Run,
    buffer: VecDeque<LogLine>,
    tx: broadcast::Sender<RunEvent>,
    next_seq: u64,
    stop_requested: bool,
    tail_offset: u64,
}

pub struct Subscription {
    pub run: Run,
    pub lines: Vec<LogLine>,
    pub rx: broadcast::Receiver<RunEvent>,
}

#[derive(Debug)]
pub enum StartError {
    AlreadyRunning,
    Message(String),
}

impl ProcessManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                runs: HashMap::new(),
                overrides: Vec::new(),
            }),
            log_limit_mb: AtomicU32::new(DEFAULT_LOG_LIMIT_MB),
            pids: RunPidIndex::default(),
        })
    }

    pub fn pid_index(&self) -> &RunPidIndex {
        &self.pids
    }

    pub async fn set_log_limit_mb(self: &Arc<Self>, limit_mb: u32) {
        let limit = crate::logs::clamp_log_limit_mb(limit_mb);
        self.log_limit_mb.store(limit, Ordering::Relaxed);
        let _ = write_log_limit(limit);
        let manager = Arc::clone(self);
        tokio::task::spawn_blocking(move || manager.cleanup_now()).await.ok();
    }

    pub fn log_limit_mb(&self) -> u32 {
        self.log_limit_mb.load(Ordering::Relaxed)
    }

    pub async fn restore(self: &Arc<Self>) {
        let _ = ensure_logs_dir();
        let _ = write_log_limit(self.log_limit_mb());
        let path = runs_path();
        let Ok(data) = std::fs::read(&path) else {
            self.spawn_reaper();
            return;
        };
        let persisted: Vec<Run> = serde_json::from_slice(&data).unwrap_or_default();
        for mut info in persisted {
            let id = info.id.clone();
            let was_running = info.status == RunStatus::Running;
            if was_running {
                let still = match (info.pid, info.process_start) {
                    (Some(pid), Some(start)) => pid_still_same(pid, start),
                    _ => false,
                };
                if !still {
                    info.status = RunStatus::Exited;
                    info.ended_at = Some(now_ms());
                    info.exit_code = None;
                    info.health = HealthInfo::unknown();
                }
            }
            let (tx, _) = broadcast::channel(BROADCAST_CAP);
            let mut rec = RunRecord {
                info: info.clone(),
                buffer: VecDeque::new(),
                tx,
                next_seq: 1,
                stop_requested: false,
                tail_offset: 0,
            };
            load_log_history(&mut rec);
            if rec.info.status == RunStatus::Running {
                rec.push_system("reconnected after Orbitry restart");
            } else if was_running {
                rec.push_system("process was no longer running after Orbitry restart");
            }
            let running_again = rec.info.status == RunStatus::Running;
            let offset = rec.file_end();
            rec.tail_offset = offset;
            {
                let mut inner = self.inner.lock().await;
                inner.runs.insert(id.clone(), rec);
            }
            if running_again {
                let manager = Arc::clone(self);
                tokio::spawn(async move { manager.tail_and_reap(&id).await });
            }
        }
        self.publish_pids().await;
        self.persist().await;
        self.spawn_reaper();
        let manager = Arc::clone(self);
        tokio::task::spawn_blocking(move || manager.cleanup_now());
    }

    pub async fn start(
        self: &Arc<Self>,
        part_id: String,
        part_name: String,
        part_path: String,
        script: String,
        cwd: Option<String>,
        source: ScriptSource,
        command: Option<String>,
    ) -> Result<Run, StartError> {
        let target = match source {
            ScriptSource::Ahk => {
                let script_path = command
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .ok_or_else(|| StartError::Message("AHK script path missing".into()))?;
                if !script_path.is_file() {
                    return Err(StartError::Message(format!(
                        "AHK script not found: {}",
                        display_path(&script_path)
                    )));
                }
                let version = parse_requires(&script_path).unwrap_or(AhkVersion::V2);
                let exe = resolve_interpreter(version).ok_or_else(|| {
                    StartError::Message(crate::ahk::missing_version_message(version))
                })?;
                SpawnTarget::Ahk { exe, script: script_path }
            }
            _ => SpawnTarget::Npm,
        };
        self.spawn_run(
            part_id,
            part_name,
            part_path,
            script,
            cwd,
            SpawnKind::Script,
            target,
        )
        .await
    }

    pub async fn start_install(
        self: &Arc<Self>,
        part_id: String,
        part_name: String,
        part_path: String,
    ) -> Result<Run, StartError> {
        self.spawn_run(
            part_id,
            part_name,
            part_path,
            "install".into(),
            None,
            SpawnKind::Install,
            SpawnTarget::Npm,
        )
        .await
    }

    async fn spawn_run(
        self: &Arc<Self>,
        part_id: String,
        part_name: String,
        part_path: String,
        script: String,
        cwd: Option<String>,
        kind: SpawnKind,
        target: SpawnTarget,
    ) -> Result<Run, StartError> {
        let cwd_raw = cwd
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(part_path.as_str());
        let cwd = PathBuf::from(cwd_raw);
        if !cwd.is_dir() {
            return Err(StartError::Message(format!(
                "project folder not found: {}",
                display_path(&cwd)
            )));
        }

        {
            let inner = self.inner.lock().await;
            if inner.runs.values().any(|run| {
                run.info.status == RunStatus::Running
                    && run.info.part_id == part_id
                    && run.info.script == script
            }) {
                return Err(StartError::AlreadyRunning);
            }
        }

        let overrides = {
            let inner = self.inner.lock().await;
            inner.overrides.clone()
        };
        let _ = ensure_logs_dir();
        let pm = detect_package_manager(&cwd);
        let command = match (&kind, &target) {
            (SpawnKind::Install, _) => install_command_label(pm),
            (SpawnKind::Script, SpawnTarget::Ahk { exe, script }) => {
                ahk_command_label(exe, script)
            }
            (SpawnKind::Script, SpawnTarget::Npm) => run_command_label(pm, &script),
        };
        let id = uuid::Uuid::new_v4().to_string();
        let log_path = run_log_path(&id);
        let file_cap = file_cap_bytes(self.log_limit_mb());
        let child_and_pid =
            spawn_detached_app(pm, &script, &cwd, &log_path, file_cap, kind, &target)
                .map_err(StartError::Message)?;
        let (mut child, pid) = child_and_pid;
        let process_start = pid.and_then(process_start_time);

        let mut expected_ports = match target {
            SpawnTarget::Ahk { .. } => Vec::new(),
            SpawnTarget::Npm => detect_expected_ports(&cwd, &[], Some(&script), None),
        };
        apply_port_overrides(&mut expected_ports, &part_id, &overrides);

        let info = Run {
            id: id.clone(),
            part_id,
            part_name,
            part_path,
            script: script.clone(),
            command: command.clone(),
            cwd: display_path(&cwd),
            pid,
            process_start,
            log_path: Some(log_path.display().to_string()),
            status: RunStatus::Running,
            exit_code: None,
            stop_requested: false,
            started_at: now_ms(),
            ended_at: None,
            ports: Vec::new(),
            expected_ports,
            health: HealthInfo::unknown(),
            warnings: Vec::new(),
            log_ports: Vec::new(),
        };

        let (tx, _) = broadcast::channel(BROADCAST_CAP);
        {
            let mut inner = self.inner.lock().await;
            let mut rec = RunRecord {
                info: info.clone(),
                buffer: VecDeque::new(),
                tx,
                next_seq: 1,
                stop_requested: false,
                tail_offset: 0,
            };
            let pid_label = pid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".into());
            rec.push_system(&format!("started {command} (pid {pid_label})"));
            inner.runs.insert(id.clone(), rec);
        }

        let waiter = Arc::clone(self);
        let wait_id = id.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            let code = status.ok().and_then(|s| s.code());
            waiter.finish(&wait_id, code).await;
        });

        let tailer = Arc::clone(self);
        let tail_id = id.clone();
        tokio::spawn(async move { tailer.tail_logs(&tail_id).await });

        self.publish_pids().await;
        self.persist().await;

        let inner = self.inner.lock().await;
        Ok(inner
            .runs
            .get(&id)
            .map(|rec| rec.info.clone())
            .unwrap_or(info))
    }

    /// Locate a live run the same way `stop_all` does (status + part + script).
    pub async fn running_id_for(&self, part_id: &str, script: &str) -> Option<String> {
        let inner = self.inner.lock().await;
        inner
            .runs
            .values()
            .find(|rec| {
                rec.info.status == RunStatus::Running
                    && rec.info.part_id == part_id
                    && rec.info.script == script
            })
            .map(|rec| rec.info.id.clone())
    }

    pub async fn stop(&self, id: &str) -> Result<Run, String> {
        let pid = {
            let mut inner = self.inner.lock().await;
            let rec = inner
                .runs
                .get_mut(id)
                .ok_or_else(|| "run not found".to_string())?;
            if rec.info.status != RunStatus::Running {
                return Ok(rec.info.clone());
            }
            rec.stop_requested = true;
            rec.push_system("stopping...");
            rec.info.pid
        };
        if let Some(pid) = pid {
            let _ = kill_process_tree(pid);
        }

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            {
                let inner = self.inner.lock().await;
                if let Some(rec) = inner.runs.get(id) {
                    if rec.info.status != RunStatus::Running {
                        self.persist_sync(&inner);
                        return Ok(rec.info.clone());
                    }
                } else {
                    return Err("run not found".into());
                }
            }
            if tokio::time::Instant::now() >= deadline {
                if let Some(pid) = pid {
                    let _ = kill_process_tree_force(pid);
                }
                tokio::time::sleep(Duration::from_millis(800)).await;
                self.finish(id, None).await;
                let inner = self.inner.lock().await;
                return inner
                    .runs
                    .get(id)
                    .map(|rec| rec.info.clone())
                    .ok_or_else(|| "run not found".to_string());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn stop_all(&self) {
        let ids: Vec<String> = {
            let inner = self.inner.lock().await;
            inner
                .runs
                .values()
                .filter(|rec| rec.info.status == RunStatus::Running)
                .map(|rec| rec.info.id.clone())
                .collect()
        };
        for id in ids {
            let _ = self.stop(&id).await;
        }
    }

    pub async fn list(&self) -> Vec<Run> {
        self.refresh_live().await;
        self.list_cached().await
    }

    pub async fn set_port_overrides(&self, overrides: Vec<PortOverride>) {
        let mut inner = self.inner.lock().await;
        inner.overrides = overrides;
    }

    pub async fn list_cached(&self) -> Vec<Run> {
        let inner = self.inner.lock().await;
        let mut runs: Vec<Run> = inner.runs.values().map(|rec| rec.info.clone()).collect();
        runs.sort_by(|a, b| {
            let a_running = a.status == RunStatus::Running;
            let b_running = b.status == RunStatus::Running;
            b_running
                .cmp(&a_running)
                .then(b.started_at.cmp(&a.started_at))
        });
        runs
    }

    pub async fn get(&self, id: &str) -> Option<Run> {
        let inner = self.inner.lock().await;
        inner.runs.get(id).map(|rec| rec.info.clone())
    }

    pub async fn subscribe(&self, id: &str) -> Option<Subscription> {
        let inner = self.inner.lock().await;
        let rec = inner.runs.get(id)?;
        Some(Subscription {
            run: rec.info.clone(),
            lines: rec.buffer.iter().cloned().collect(),
            rx: rec.tx.subscribe(),
        })
    }

    async fn finish(&self, id: &str, exit_code: Option<i32>) {
        {
            let mut inner = self.inner.lock().await;
            let Some(rec) = inner.runs.get_mut(id) else {
                return;
            };
            if rec.info.status != RunStatus::Running {
                return;
            }
            rec.info.exit_code = exit_code;
            rec.info.ended_at = Some(now_ms());
            rec.info.stop_requested = rec.stop_requested;
            rec.info.status = if rec.stop_requested || exit_code == Some(0) {
                RunStatus::Exited
            } else {
                RunStatus::Failed
            };
            rec.info.health = HealthInfo {
                status: HealthStatus::Unknown,
                url: rec.info.health.url.clone(),
                checked_at: rec.info.health.checked_at,
                ports: rec.info.health.ports.clone(),
            };
            rec.info.warnings.clear();
            let code_label = exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".into());
            rec.push_system(&format!("exited with code {code_label}"));
            let run = rec.info.clone();
            let _ = rec.tx.send(RunEvent::Status { run });
            prune_finished(&mut inner);
            self.persist_sync(&inner);
        }
        self.publish_pids().await;
        let manager_limit = self.log_limit_mb();
        let manager = {
            // capture finished + running for cleanup
            let inner = self.inner.lock().await;
            collect_cleanup_state(&inner)
        };
        tokio::task::spawn_blocking(move || {
            cleanup_logs(manager_limit, &manager.0, &manager.1);
        });
    }

    async fn push_line(&self, id: &str, stream: LogStream, text: String) {
        let mut inner = self.inner.lock().await;
        if let Some(rec) = inner.runs.get_mut(id) {
            let found = ports_from_log(&text);
            rec.push(stream, text);
            if !found.is_empty() {
                rec.info.merge_log_ports(found);
            }
        }
    }

    pub async fn refresh_live(&self) {
        let snapshots: Vec<(String, Option<u32>, bool)> = {
            let inner = self.inner.lock().await;
            inner
                .runs
                .values()
                .filter(|rec| rec.info.status == RunStatus::Running)
                .map(|rec| {
                    (
                        rec.info.id.clone(),
                        rec.info.pid,
                        rec.info.health.is_stale(now_ms(), 2500),
                    )
                })
                .collect()
        };
        if snapshots.is_empty() {
            return;
        }

        let snapshot = tokio::task::spawn_blocking(ListeningSnapshot::collect)
            .await
            .unwrap_or_default();

        let probes: Vec<(String, Vec<crate::ports::PortInfo>)> = {
            let mut inner = self.inner.lock().await;
            let overrides = inner.overrides.clone();
            let mut ready = Vec::new();
            for (id, pid, stale) in snapshots {
                let Some(rec) = inner.runs.get_mut(&id) else {
                    continue;
                };
                if rec.info.status != RunStatus::Running {
                    continue;
                }
                let live = pid
                    .map(|value| snapshot.ports_for(value))
                    .unwrap_or_default();
                rec.info.apply_live_ports(live);
                apply_port_overrides(
                    &mut rec.info.ports,
                    &rec.info.part_id,
                    &overrides,
                );
                if stale {
                    ready.push((id, rec.info.ports.clone()));
                }
            }
            ready
        };

        let mut healths = Vec::new();
        for (id, ports) in probes {
            let health = check_health(&ports).await;
            healths.push((id, health));
        }

        if healths.is_empty() {
            return;
        }
        let mut inner = self.inner.lock().await;
        for (id, health) in healths {
            if let Some(rec) = inner.runs.get_mut(&id) {
                if rec.info.status == RunStatus::Running {
                    rec.info.health = health;
                }
            }
        }
    }

    async fn persist(&self) {
        let inner = self.inner.lock().await;
        self.persist_sync(&inner);
    }

    fn persist_sync(&self, inner: &Inner) {
        let runs: Vec<Run> = inner.runs.values().map(|rec| rec.info.clone()).collect();
        if let Ok(data) = serde_json::to_vec_pretty(&runs) {
            let _ = atomic_write(&runs_path(), &data);
        }
    }

    async fn publish_pids(&self) {
        let inner = self.inner.lock().await;
        let pids: Vec<(String, u32)> = inner
            .runs
            .values()
            .filter(|rec| rec.info.status == RunStatus::Running)
            .filter_map(|rec| rec.info.pid.map(|pid| (rec.info.id.clone(), pid)))
            .collect();
        self.pids.set(pids);
    }

    fn spawn_reaper(self: &Arc<Self>) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                manager.reap_missing().await;
            }
        });
    }

    async fn reap_missing(&self) {
        let stale: Vec<String> = {
            let inner = self.inner.lock().await;
            inner
                .runs
                .values()
                .filter(|rec| rec.info.status == RunStatus::Running)
                .filter(|rec| match (rec.info.pid, rec.info.process_start) {
                    (Some(pid), Some(start)) => !pid_still_same(pid, start),
                    (Some(pid), None) => process_start_time(pid).is_none(),
                    _ => true,
                })
                .map(|rec| rec.info.id.clone())
                .collect()
        };
        for id in stale {
            self.finish(&id, None).await;
        }
    }

    async fn tail_and_reap(&self, id: &str) {
        tokio::join!(self.tail_logs(id), async {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let running = {
                    let inner = self.inner.lock().await;
                    inner
                        .runs
                        .get(id)
                        .is_some_and(|rec| rec.info.status == RunStatus::Running)
                };
                if !running {
                    break;
                }
            }
        });
    }

    async fn tail_logs(&self, id: &str) {
        let log_path = run_log_path(id);
        let rotated = run_log_rotated_path(id);
        let mut offset = {
            let inner = self.inner.lock().await;
            inner.runs.get(id).map(|rec| rec.tail_offset).unwrap_or(0)
        };
        let mut buf = Vec::new();
        loop {
            {
                let inner = self.inner.lock().await;
                if inner
                    .runs
                    .get(id)
                    .is_none_or(|rec| rec.info.status != RunStatus::Running)
                {
                    // One last read so the final lines still appear.
                    let _ = self.read_new_lines(id, &log_path, &rotated, &mut offset, &mut buf).await;
                    break;
                }
            }
            let _ = self
                .read_new_lines(id, &log_path, &rotated, &mut offset, &mut buf)
                .await;
            tokio::time::sleep(TAIL_POLL).await;
        }
    }

    async fn read_new_lines(
        &self,
        id: &str,
        path: &Path,
        rotated: &Path,
        offset: &mut u64,
        leftover: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        let size = match tokio::fs::metadata(path).await {
            Ok(meta) => meta.len(),
            Err(_) => return Ok(()),
        };
        if size < *offset {
            // The live file was rotated; drain the remainder of the previous file first.
            if rotated.exists() {
                if let Ok(mut file) = tokio::fs::File::open(rotated).await {
                    let _ = file.seek(tokio::io::SeekFrom::Start(*offset)).await;
                    let mut rest = Vec::new();
                    let _ = file.read_to_end(&mut rest).await;
                    leftover.extend_from_slice(&rest);
                    self.flush_lines(id, leftover, false).await;
                }
            }
            *offset = 0;
        }
        if size == *offset {
            return Ok(());
        }
        let mut file = tokio::fs::File::open(path).await?;
        file.seek(tokio::io::SeekFrom::Start(*offset)).await?;
        let mut chunk = Vec::new();
        file.read_to_end(&mut chunk).await?;
        *offset += chunk.len() as u64;
        leftover.extend_from_slice(&chunk);
        self.flush_lines(id, leftover, false).await;
        Ok(())
    }

    async fn flush_lines(&self, id: &str, leftover: &mut Vec<u8>, drain: bool) {
        let text = String::from_utf8_lossy(leftover);
        let last_nl = text.rfind('\n');
        let (complete, rest) = if let Some(index) = last_nl {
            let split = index + 1;
            let bytes = split.min(leftover.len());
            let complete = String::from_utf8_lossy(&leftover[..bytes]).to_string();
            let rest = leftover[bytes..].to_vec();
            (complete, rest)
        } else if drain {
            let complete = String::from_utf8_lossy(leftover).to_string();
            leftover.clear();
            if complete.is_empty() {
                return;
            }
            (complete, Vec::new())
        } else {
            return;
        };
        *leftover = rest;
        for line in complete.split_inclusive(['\n', '\r']) {
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                continue;
            }
            self.push_line(id, LogStream::Stdout, line.to_string()).await;
        }
    }

    fn cleanup_now(&self) {
        // Best-effort; called from a blocking thread.
        let running;
        let finished;
        if let Ok(inner) = self.inner.try_lock() {
            let state = collect_cleanup_state(&inner);
            running = state.0;
            finished = state.1;
        } else {
            return;
        }
        cleanup_logs(self.log_limit_mb(), &running, &finished);
    }
}

impl RunRecord {
    fn push(&mut self, stream: LogStream, text: String) {
        if stream == LogStream::System {
            let path = run_system_log_path(&self.info.id);
            let _ = append_line(&path, &text);
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let ts = now_ms();
        let line = LogLine {
            seq,
            stream,
            text: text.clone(),
            ts,
        };
        self.buffer.push_back(line);
        while self.buffer.len() > LOG_BUFFER_CAP {
            self.buffer.pop_front();
        }
        let _ = self.tx.send(RunEvent::Line {
            seq,
            stream,
            text,
            ts,
        });
    }

    fn push_system(&mut self, text: &str) {
        self.push(LogStream::System, text.to_string());
    }

    fn file_end(&self) -> u64 {
        run_log_path(&self.info.id)
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0)
    }
}

fn load_log_history(rec: &mut RunRecord) {
    let rotated = run_log_rotated_path(&rec.info.id);
    let live = run_log_path(&rec.info.id);
    let system = run_system_log_path(&rec.info.id);
    let mut lines: Vec<(i64, LogStream, String)> = Vec::new();
    // We don't store timestamps in the files; use 0 and let seq order them.
    for (path, stream) in [
        (rotated, LogStream::Stdout),
        (live, LogStream::Stdout),
        (system, LogStream::System),
    ] {
        if let Ok(data) = std::fs::read_to_string(&path) {
            for line in data.lines() {
                if !line.is_empty() {
                    lines.push((0, stream, line.to_string()));
                }
            }
        }
    }
    if lines.len() > LOG_BUFFER_CAP {
        lines = lines.split_off(lines.len() - LOG_BUFFER_CAP);
    }
    for (_, stream, text) in lines {
        // Avoid writing system lines back to disk while loading.
        let seq = rec.next_seq;
        rec.next_seq += 1;
        rec.buffer.push_back(LogLine {
            seq,
            stream,
            text,
            ts: rec.info.started_at,
        });
    }
}

fn prune_finished(inner: &mut Inner) {
    let mut finished: Vec<(i64, String)> = inner
        .runs
        .values()
        .filter(|rec| rec.info.status != RunStatus::Running)
        .map(|rec| {
            (
                rec.info.ended_at.unwrap_or(rec.info.started_at),
                rec.info.id.clone(),
            )
        })
        .collect();
    if finished.len() <= FINISHED_CAP {
        return;
    }
    finished.sort_by_key(|(ended, _)| *ended);
    let extra = finished.len() - FINISHED_CAP;
    for (_, id) in finished.into_iter().take(extra) {
        inner.runs.remove(&id);
    }
}

fn collect_cleanup_state(inner: &Inner) -> (Vec<String>, Vec<FinishedLog>) {
    let running: Vec<String> = inner
        .runs
        .values()
        .filter(|rec| rec.info.status == RunStatus::Running)
        .map(|rec| rec.info.id.clone())
        .collect();
    let finished = inner
        .runs
        .values()
        .filter(|rec| rec.info.status != RunStatus::Running)
        .map(|rec| FinishedLog {
            run_id: rec.info.id.clone(),
            part_key: format!("{}::{}", rec.info.part_id, rec.info.script),
            ended_at_ms: rec.info.ended_at.unwrap_or(rec.info.started_at),
        })
        .collect();
    (running, finished)
}

#[derive(Clone, Copy)]
enum SpawnKind {
    Script,
    Install,
}

#[derive(Clone)]
enum SpawnTarget {
    Npm,
    Ahk { exe: PathBuf, script: PathBuf },
}

fn make_app_command(
    pm: &str,
    script: &str,
    cwd: &Path,
    kind: SpawnKind,
    target: &SpawnTarget,
) -> tokio::process::Command {
    match (kind, target) {
        (SpawnKind::Install, _) => install_command(pm, cwd),
        (SpawnKind::Script, SpawnTarget::Ahk { exe, script }) => ahk_command(exe, script, cwd),
        (SpawnKind::Script, SpawnTarget::Npm) => script_command(pm, script, cwd),
    }
}

fn spawn_detached_app(
    pm: &str,
    script: &str,
    cwd: &Path,
    log_path: &Path,
    file_cap: u64,
    kind: SpawnKind,
    target: &SpawnTarget,
) -> Result<(tokio::process::Child, Option<u32>), String> {
    let (reader, writer) =
        os_pipe::pipe().map_err(|err| format!("log pipe: {err}"))?;
    let writer_err = writer
        .try_clone()
        .map_err(|err| format!("log pipe: {err}"))?;

    let mut pump_cmd = std::process::Command::new(
        crate::logpump::ensure_logpump_exe().map_err(|err| format!("logpump: {err}"))?,
    );
    pump_cmd
        .args([
            "--internal-log-pump",
            "--log",
            &log_path.to_string_lossy(),
            "--limit-file",
            &crate::logs::log_limit_path().to_string_lossy(),
            "--file-cap",
            &file_cap.to_string(),
        ])
        .stdin(std::process::Stdio::from(reader))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    apply_detach_flags(&mut pump_cmd);
    let mut pump = match pump_cmd.spawn() {
        Ok(child) => child,
        Err(_) => {
            #[cfg(windows)]
            {
                pump_cmd.creation_flags(detach_creation_flags_no_breakaway());
            }
            pump_cmd
                .spawn()
                .map_err(|err| format!("logpump: {err}"))?
        }
    };
    std::thread::spawn(move || {
        let _ = pump.wait();
    });

    let mut cmd = make_app_command(pm, script, cwd, kind, target);
    cmd.stdout(std::process::Stdio::from(writer))
        .stderr(std::process::Stdio::from(writer_err));
    match cmd.spawn() {
        Ok(child) => {
            let pid = child.id();
            Ok((child, pid))
        }
        Err(err) => {
            #[cfg(windows)]
            {
                let mut retry = make_app_command(pm, script, cwd, kind, target);
                retry.creation_flags(detach_creation_flags_no_breakaway());
                // The previous pipe writers were moved; we cannot retry with the same
                // handles. Surface the original error instead of a broken retry.
                let _ = detach_creation_flags();
                let _ = retry;
            }
            let command = match (kind, target) {
                (SpawnKind::Install, _) => format!("{pm} install"),
                (SpawnKind::Script, SpawnTarget::Ahk { exe, script }) => {
                    ahk_command_label(exe, script)
                }
                (SpawnKind::Script, SpawnTarget::Npm) => format!("{pm} run {script}"),
            };
            Err(format!("spawn {command}: {err}"))
        }
    }
}
