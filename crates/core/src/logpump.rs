//! Detached stdin-to-file pump with size-based rotation.
//!
//! Spawned as `orbitry --internal-log-pump` from a copy of the executable under
//! the config directory, so it does not lock the official binary during updates.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::config_path;
use crate::logs::{file_cap_bytes, open_append, rotate_log, write_log_limit, DEFAULT_LOG_LIMIT_MB};

const COPY_NAME: &str = if cfg!(windows) { "logpump.exe" } else { "logpump" };

pub fn run_log_pump(args: &[String]) -> i32 {
    let mut log_path = None;
    let mut limit_file = None;
    let mut cap = file_cap_bytes(DEFAULT_LOG_LIMIT_MB);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--log" => {
                if let Some(value) = args.get(i + 1) {
                    log_path = Some(PathBuf::from(value));
                    i += 2;
                    continue;
                }
            }
            "--limit-file" => {
                if let Some(value) = args.get(i + 1) {
                    limit_file = Some(PathBuf::from(value));
                    i += 2;
                    continue;
                }
            }
            "--file-cap" => {
                if let Some(value) = args.get(i + 1) {
                    if let Ok(parsed) = value.parse::<u64>() {
                        if parsed > 0 {
                            cap = parsed;
                        }
                    }
                    i += 2;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let Some(log_path) = log_path else {
        eprintln!("logpump: missing --log");
        return 2;
    };
    if let Err(err) = pump_loop(&log_path, limit_file.as_deref(), cap) {
        eprintln!("logpump: {err}");
        return 1;
    }
    0
}

fn pump_loop(log_path: &Path, limit_file: Option<&Path>, mut cap: u64) -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut file = open_append(log_path)?;
    let mut written = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buf = [0u8; 16 * 1024];
    let mut last_check = Instant::now();
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if last_check.elapsed() >= Duration::from_secs(2) {
            if let Some(path) = limit_file {
                if let Ok(data) = fs::read_to_string(path) {
                    if let Ok(value) = data.trim().parse::<u64>() {
                        if value > 0 {
                            cap = value;
                        }
                    }
                }
            }
            last_check = Instant::now();
        }
        if written > 0 && written.saturating_add(n as u64) > cap {
            drop(file);
            rotate_log(log_path)?;
            file = open_append(log_path)?;
            written = 0;
        }
        file.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
    }
    file.flush()?;
    Ok(())
}

/// Ensure a reusable logpump copy exists in the config directory and return its path.
pub fn ensure_logpump_exe() -> io::Result<PathBuf> {
    let dir = config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&dir)?;
    let dest = dir.join(COPY_NAME);
    let src = std::env::current_exe()?;
    let need_copy = match (dest.metadata(), src.metadata()) {
        (Ok(dst), Ok(src_meta)) => {
            dst.len() != src_meta.len()
                || dst.modified().ok() != src_meta.modified().ok() && dst.len() != src_meta.len()
        }
        (Err(_), _) => true,
        _ => false,
    };
    // Also copy when the stamp disagrees with this binary's size.
    let stamp = dir.join("logpump.stamp");
    let src_len = src.metadata().map(|m| m.len()).unwrap_or(0);
    let stamped = fs::read_to_string(&stamp)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());
    if need_copy || stamped != Some(src_len) || !dest.exists() {
        let tmp = dir.join(format!("{COPY_NAME}.new"));
        if fs::copy(&src, &tmp).is_ok() {
            if fs::rename(&tmp, &dest).is_err() {
                // The running copy may be locked; leave the existing dest in place
                // and drop the temp file. New runs keep using the previous pump.
                let _ = fs::remove_file(&tmp);
            } else {
                let _ = fs::write(&stamp, format!("{src_len}\n"));
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
                }
            }
        }
    }
    if dest.exists() {
        Ok(dest)
    } else {
        Ok(src)
    }
}

pub fn spawn_logpump(log_path: &Path, file_cap: u64) -> io::Result<std::process::Child> {
    let _ = write_log_limit(DEFAULT_LOG_LIMIT_MB);
    let exe = ensure_logpump_exe()?;
    let limit = crate::logs::log_limit_path();
    let mut cmd = Command::new(exe);
    cmd.args([
        "--internal-log-pump",
        "--log",
        &log_path.to_string_lossy(),
        "--limit-file",
        &limit.to_string_lossy(),
        "--file-cap",
        &file_cap.to_string(),
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    apply_detach_flags(&mut cmd);
    cmd.spawn()
}

pub fn apply_detach_flags(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

#[cfg(windows)]
pub fn create_no_window_flags() -> u32 {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    CREATE_NO_WINDOW
}

pub fn detach_creation_flags() -> u32 {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB
}

#[cfg(windows)]
pub fn detach_creation_flags_no_breakaway() -> u32 {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
}

pub fn apply_detach_flags_tokio(cmd: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = cmd;
    }
}
