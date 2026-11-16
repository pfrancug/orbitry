//! Background system and per-run resource samples (CPU and RSS).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::process::now_ms;

const SAMPLE_EVERY: Duration = Duration::from_secs(2);
const HISTORY: usize = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetrics {
    pub cpu_pct: f32,
    pub mem_used: u64,
    pub mem_total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMetrics {
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    pub pid_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsHistory {
    pub cpu: Vec<f32>,
    pub mem_pct: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub sampled_at: i64,
    pub system: SystemMetrics,
    pub runs: HashMap<String, RunMetrics>,
    pub history: MetricsHistory,
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self {
            sampled_at: 0,
            system: SystemMetrics {
                cpu_pct: 0.0,
                mem_used: 0,
                mem_total: 0,
            },
            runs: HashMap::new(),
            history: MetricsHistory {
                cpu: Vec::new(),
                mem_pct: Vec::new(),
            },
        }
    }
}

/// Live PID list shared with the sampler thread.
#[derive(Clone, Default)]
pub struct RunPidIndex {
    inner: Arc<Mutex<Vec<(String, u32)>>>,
}

impl RunPidIndex {
    pub fn set(&self, pids: Vec<(String, u32)>) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = pids;
        }
    }

    pub fn get(&self) -> Vec<(String, u32)> {
        self.inner
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct MetricsHub {
    cache: Arc<Mutex<MetricsSnapshot>>,
    pids: RunPidIndex,
}

impl MetricsHub {
    pub fn start(pids: RunPidIndex) -> Self {
        let cache = Arc::new(Mutex::new(MetricsSnapshot::default()));
        let cache_thread = Arc::clone(&cache);
        let pids_thread = pids.clone();
        thread::Builder::new()
            .name("orbitry-metrics".into())
            .spawn(move || sampler_loop(cache_thread, pids_thread))
            .expect("spawn metrics thread");
        Self { cache, pids }
    }

    pub fn pids(&self) -> &RunPidIndex {
        &self.pids
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        self.cache
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

fn sampler_loop(cache: Arc<Mutex<MetricsSnapshot>>, pids: RunPidIndex) {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    thread::sleep(Duration::from_millis(200));

    let mut cpu_hist = VecDeque::with_capacity(HISTORY);
    let mut mem_hist = VecDeque::with_capacity(HISTORY);

    loop {
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let cpu_pct = sys.global_cpu_usage();
        let mem_used = sys.used_memory();
        let mem_total = sys.total_memory();
        let mem_pct = if mem_total > 0 {
            (mem_used as f64 / mem_total as f64) * 100.0
        } else {
            0.0
        } as f32;

        let children = build_children(&sys);
        let mut runs = HashMap::new();
        for (run_id, pid) in pids.get() {
            let tree = walk_tree(pid, &children);
            let mut cpu = 0.0f32;
            let mut mem = 0u64;
            let mut count = 0u32;
            for child in tree {
                if let Some(proc) = sys.process(Pid::from_u32(child)) {
                    cpu += proc.cpu_usage();
                    mem += proc.memory();
                    count += 1;
                }
            }
            runs.insert(
                run_id,
                RunMetrics {
                    cpu_pct: cpu,
                    mem_bytes: mem,
                    pid_count: count,
                },
            );
        }

        push_hist(&mut cpu_hist, cpu_pct);
        push_hist(&mut mem_hist, mem_pct);

        let snapshot = MetricsSnapshot {
            sampled_at: now_ms(),
            system: SystemMetrics {
                cpu_pct,
                mem_used,
                mem_total,
            },
            runs,
            history: MetricsHistory {
                cpu: cpu_hist.iter().copied().collect(),
                mem_pct: mem_hist.iter().copied().collect(),
            },
        };
        if let Ok(mut guard) = cache.lock() {
            *guard = snapshot;
        }
        thread::sleep(SAMPLE_EVERY);
    }
}

fn push_hist(hist: &mut VecDeque<f32>, value: f32) {
    if hist.len() == HISTORY {
        hist.pop_front();
    }
    hist.push_back(value);
}

fn build_children(sys: &System) -> HashMap<u32, Vec<u32>> {
    let mut map: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, proc) in sys.processes() {
        let pid = pid.as_u32();
        if let Some(parent) = proc.parent() {
            map.entry(parent.as_u32()).or_default().push(pid);
        }
    }
    map
}

fn walk_tree(root: u32, children: &HashMap<u32, Vec<u32>>) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    let mut seen = std::collections::HashSet::new();
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        out.push(pid);
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids);
        }
    }
    out
}

/// Process start time in seconds since the Unix epoch, if the PID is still alive.
pub fn process_start_time(pid: u32) -> Option<u64> {
    let mut sys = System::new();
    let handle = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[handle]), true);
    sys.process(handle).map(|proc| proc.start_time())
}

pub fn pid_still_same(pid: u32, start: u64) -> bool {
    process_start_time(pid) == Some(start)
}
