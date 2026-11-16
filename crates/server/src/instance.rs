use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use orbitry_core::instance_path;
use serde::{Deserialize, Serialize};
use single_instance::SingleInstance;

const MUTEX_NAME: &str = "com.orbitry.app.single-instance";

#[derive(Serialize, Deserialize)]
struct InstanceInfo {
    port: u16,
    pid: u32,
}

pub struct InstanceLock {
    _guard: SingleInstance,
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        clear();
    }
}

pub fn acquire() -> Result<Option<InstanceLock>, String> {
    let guard =
        SingleInstance::new(MUTEX_NAME).map_err(|err| format!("single-instance lock: {err}"))?;
    if guard.is_single() {
        Ok(Some(InstanceLock { _guard: guard }))
    } else {
        Ok(None)
    }
}

pub fn dashboard_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub fn running_url(fallback_port: u16) -> String {
    dashboard_url(read_port().unwrap_or(fallback_port))
}

pub fn write(port: u16) -> Result<(), String> {
    let path = instance_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    }
    let info = InstanceInfo {
        port,
        pid: std::process::id(),
    };
    let data = serde_json::to_vec_pretty(&info).map_err(|err| format!("instance json: {err}"))?;
    fs::write(&path, data).map_err(|err| format!("write {}: {err}", path.display()))
}

pub fn read_port() -> Option<u16> {
    let path = instance_path();
    let data = fs::read(&path).ok()?;
    serde_json::from_slice::<InstanceInfo>(&data)
        .ok()
        .map(|info| info.port)
}

pub fn clear() {
    let path: PathBuf = instance_path();
    let _ = fs::remove_file(path);
}


pub fn acquire_with_retry(timeout: Duration) -> Result<Option<InstanceLock>, String> {
    let started = Instant::now();
    loop {
        match acquire()? {
            Some(lock) => return Ok(Some(lock)),
            None if started.elapsed() >= timeout => return Ok(None),
            None => thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Wait until `pid` has exited, or return an error if `timeout` elapses.
pub fn wait_for_pid(pid: u32, timeout: Duration) -> Result<(), String> {
    #[cfg(windows)]
    {
        wait_for_pid_windows(pid, timeout)
    }
    #[cfg(unix)]
    {
        wait_for_pid_unix(pid, timeout)
    }
}

#[cfg(windows)]
fn wait_for_pid_windows(pid: u32, timeout: Duration) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::OpenProcess;

    const PROCESS_SYNCHRONIZE: u32 = 0x00100000;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    unsafe {
        let handle = OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        if handle.is_null() {
            return Ok(());
        }
        let millis = timeout.as_millis().min(u32::MAX as u128) as u32;
        let result = windows_sys::Win32::System::Threading::WaitForSingleObject(handle, millis);
        CloseHandle(handle);
        if result == WAIT_OBJECT_0 {
            Ok(())
        } else if result == WAIT_TIMEOUT {
            Err(format!("timed out waiting for process {pid} to exit"))
        } else {
            Err(format!("wait for process {pid} failed"))
        }
    }
}

#[cfg(unix)]
fn wait_for_pid_unix(pid: u32, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    loop {
        let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
        if !alive {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            return Err(format!("timed out waiting for process {pid} to exit"));
        }
        thread::sleep(Duration::from_millis(200));
    }
}
