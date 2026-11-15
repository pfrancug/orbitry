//! In-process manager for running detected scripts and streaming their logs.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, Mutex};

use crate::display_path;
use crate::health::{check_health, HealthInfo, HealthStatus};
use crate::config::PortOverride;
use crate::ports::{apply_port_overrides, detect_static_ports, ports_from_log, ListeningSnapshot};
use crate::process::{
    detect_package_manager, kill_process_tree, kill_process_tree_force, now_ms, run_command_label,
    script_command, LogLine, LogStream, Run, RunEvent, RunStatus,
};

#[cfg(windows)]
use crate::process::WinJob;

pub const LOG_BUFFER_CAP: usize = 2000;
const FINISHED_CAP: usize = 50;
const BROADCAST_CAP: usize = 256;

pub struct ProcessManager {
    inner: Mutex<Inner>,
    #[cfg(windows)]
    job: Option<WinJob>,
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
            #[cfg(windows)]
            job: WinJob::new(),
        })
    }

    pub async fn start(
        self: &Arc<Self>,
        part_id: String,
        part_name: String,
        part_path: String,
        script: String,
    ) -> Result<Run, StartError> {
        let cwd = PathBuf::from(&part_path);
        if !cwd.is_dir() {
            return Err(StartError::Message(format!(
                "project folder not found: {part_path}"
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
        let pm = detect_package_manager(&cwd);
        let command = run_command_label(pm, &script);
        let id = uuid::Uuid::new_v4().to_string();
        let mut child = script_command(pm, &script, &cwd)
            .spawn()
            .map_err(|err| StartError::Message(format!("spawn {command}: {err}")))?;
        let pid = child.id();

        #[cfg(windows)]
        if let (Some(job), Some(pid)) = (&self.job, pid) {
            let _ = job.assign(pid);
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let mut expected_ports = detect_static_ports(&cwd, &[], Some(&script), None);
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
            status: RunStatus::Running,
            exit_code: None,
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
            };
            let pid_label = pid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".into());
            rec.push_system(&format!("started {command} (pid {pid_label})"));
            inner.runs.insert(id.clone(), rec);
        }

        if let Some(pipe) = stdout {
            tokio::spawn(pump_lines(
                Arc::clone(self),
                id.clone(),
                LogStream::Stdout,
                pipe,
            ));
        }
        if let Some(pipe) = stderr {
            tokio::spawn(pump_lines(
                Arc::clone(self),
                id.clone(),
                LogStream::Stderr,
                pipe,
            ));
        }

        let waiter = Arc::clone(self);
        let wait_id = id.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            let code = status.ok().and_then(|s| s.code());
            waiter.finish(&wait_id, code).await;
        });

        let inner = self.inner.lock().await;
        Ok(inner
            .runs
            .get(&id)
            .map(|rec| rec.info.clone())
            .unwrap_or(info))
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
        let mut inner = self.inner.lock().await;
        let Some(rec) = inner.runs.get_mut(id) else {
            return;
        };
        if rec.info.status != RunStatus::Running {
            return;
        }
        rec.info.exit_code = exit_code;
        rec.info.ended_at = Some(now_ms());
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

    async fn refresh_live(&self) {
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
}

impl RunRecord {
    fn push(&mut self, stream: LogStream, text: String) {
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

async fn pump_lines<R>(manager: Arc<ProcessManager>, id: String, stream: LogStream, pipe: R)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match read_line_cr(&mut reader, &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buf)
                    .trim_end_matches(['\r', '\n'])
                    .to_string();
                manager.push_line(&id, stream, text).await;
            }
            Err(_) => break,
        }
    }
}

async fn read_line_cr<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> std::io::Result<usize> {
    let mut read = 0usize;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(read);
        }
        let mut consumed = 0usize;
        let mut done = false;
        let mut take = 0usize;
        for (index, byte) in available.iter().enumerate() {
            if *byte == b'\n' {
                take = index + 1;
                consumed = take;
                done = true;
                break;
            }
            if *byte == b'\r' {
                take = index + 1;
                consumed = take;
                if index + 1 < available.len() && available[index + 1] == b'\n' {
                    take = index + 2;
                    consumed = take;
                }
                done = true;
                break;
            }
        }
        if !done {
            take = available.len();
            consumed = take;
        }
        buf.extend_from_slice(&available[..take]);
        read += consumed;
        reader.consume(consumed);
        if done {
            return Ok(read);
        }
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        if let Ok(inner) = self.inner.try_lock() {
            for rec in inner.runs.values() {
                if rec.info.status == RunStatus::Running {
                    if let Some(pid) = rec.info.pid {
                        let _ = kill_process_tree(pid);
                    }
                }
            }
        }
    }
}
