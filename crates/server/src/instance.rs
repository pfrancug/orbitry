use std::fs;
use std::path::PathBuf;

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
