//! OS login autostart. The real state is the OS entry, not config.json.

#[cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{display_path, paths_equal};

#[cfg(windows)]
const RUN_VALUE: &str = "Orbitry";
#[cfg(all(unix, not(target_os = "macos")))]
const LINUX_DESKTOP: &str = "orbitry.desktop";
#[cfg(target_os = "macos")]
const MAC_LABEL: &str = "com.orbitry.app";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub stale: bool,
}

impl AutostartStatus {
    fn off() -> Self {
        Self {
            enabled: false,
            command: None,
            stale: false,
        }
    }
}

pub fn autostart_status() -> AutostartStatus {
    match registered_command() {
        Ok(Some(command)) => AutostartStatus {
            enabled: true,
            stale: command_is_stale(&command),
            command: Some(command),
        },
        Ok(None) => AutostartStatus::off(),
        Err(_) => AutostartStatus::off(),
    }
}

pub fn set_autostart_enabled(enabled: bool) -> Result<AutostartStatus, String> {
    if enabled {
        install_autostart()?;
    } else {
        remove_autostart()?;
    }
    Ok(autostart_status())
}

/// If autostart is on and the registered command no longer points at this exe
/// (or is missing `--autostart`), rewrite it. No-op when autostart is off.
pub fn refresh_autostart_path() -> Result<AutostartStatus, String> {
    let status = autostart_status();
    if status.enabled && status.stale {
        install_autostart()?;
    }
    Ok(autostart_status())
}

fn current_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|err| format!("current exe: {err}"))?;
    Ok(PathBuf::from(display_path(&exe)))
}

#[cfg(windows)]
fn desired_command() -> Result<String, String> {
    Ok(command_for(&current_exe()?))
}

fn command_for(exe: &Path) -> String {
    let path = display_path(exe);
    if needs_quotes(&path) {
        format!("\"{path}\" --autostart")
    } else {
        format!("{path} --autostart")
    }
}

fn needs_quotes(path: &str) -> bool {
    path.chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\''))
}

fn command_is_stale(command: &str) -> bool {
    if !command.split_whitespace().any(|part| part == "--autostart") {
        return true;
    }
    let Some(registered) = exe_from_command(command) else {
        return true;
    };
    let Ok(current) = current_exe() else {
        return true;
    };
    if !registered.exists() {
        return true;
    }
    !paths_equal(&display_path(&registered), &display_path(&current))
}

fn exe_from_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    let path = if let Some(rest) = command.strip_prefix('"') {
        let end = rest.find('"')?;
        rest[..end].to_string()
    } else {
        command
            .split_whitespace()
            .next()
            .unwrap_or(command)
            .to_string()
    };
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn registered_command() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        windows_run::get(RUN_VALUE)
    }
    #[cfg(target_os = "macos")]
    {
        macos_command()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux_command()
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(None)
    }
}

fn install_autostart() -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_run::set(RUN_VALUE, &desired_command()?)
    }
    #[cfg(target_os = "macos")]
    {
        macos_install()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux_install()
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err("autostart is not supported on this system".into())
    }
}

fn remove_autostart() -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_run::delete(RUN_VALUE)
    }
    #[cfg(target_os = "macos")]
    {
        macos_remove()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux_remove()
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(())
    }
}

#[cfg(windows)]
mod windows_run {
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ,
    };

    const SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    fn open(write: bool) -> Result<Key, String> {
        let subkey = wide(SUBKEY);
        let access = if write {
            KEY_READ | KEY_SET_VALUE
        } else {
            KEY_READ
        };
        let mut hkey: HKEY = ptr::null_mut();
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, access, &mut hkey) };
        if status != ERROR_SUCCESS {
            return Err(format!("open Run key: {status}"));
        }
        Ok(Key(hkey))
    }

    pub fn get(name: &str) -> Result<Option<String>, String> {
        let key = open(false)?;
        let name = wide(name);
        let mut ty = 0u32;
        let mut size = 0u32;
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                ptr::null_mut(),
                &mut ty,
                ptr::null_mut(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(format!("read Run value: {status}"));
        }
        if size == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; size as usize];
        let mut size = size;
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("read Run value: {status}"));
        }
        if ty != REG_SZ && ty != REG_EXPAND_SZ {
            return Err(format!("unexpected Run value type {ty}"));
        }
        let units = (size as usize) / 2;
        let chars = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u16>(), units) };
        let end = chars.iter().position(|&unit| unit == 0).unwrap_or(chars.len());
        let value = String::from_utf16_lossy(&chars[..end]);
        if value.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    pub fn set(name: &str, value: &str) -> Result<(), String> {
        let key = open(true)?;
        let name = wide(name);
        let data = wide(value);
        let bytes = (data.len() * 2) as u32;
        let status = unsafe {
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("write Run value: {status}"));
        }
        Ok(())
    }

    pub fn delete(name: &str) -> Result<(), String> {
        let key = open(true)?;
        let name = wide(name);
        let status = unsafe { RegDeleteValueW(key.0, name.as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("delete Run value: {status}"))
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_plist_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "home directory not found".to_string())?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{MAC_LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn macos_command() -> Result<Option<String>, String> {
    let path = macos_plist_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    if plist_disabled(&data) {
        return Ok(None);
    }
    if let Some(exe) = plist_program(&data) {
        return Ok(Some(command_for(Path::new(&exe))));
    }
    Ok(Some(data))
}

#[cfg(target_os = "macos")]
fn plist_disabled(data: &str) -> bool {
    data.contains("<key>Disabled</key>")
        && data
            .split("<key>Disabled</key>")
            .nth(1)
            .is_some_and(|rest| rest.trim_start().starts_with("<true/>"))
}

#[cfg(target_os = "macos")]
fn plist_program(data: &str) -> Option<String> {
    let start = data.find("<key>ProgramArguments</key>")?;
    let array = &data[start..];
    let first = array.find("<string>")?;
    let rest = &array[first + "<string>".len()..];
    let end = rest.find("</string>")?;
    Some(rest[..end].to_string())
}

#[cfg(target_os = "macos")]
fn macos_install() -> Result<(), String> {
    let exe = current_exe()?;
    let path = macos_plist_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    let exe_xml = xml_escape(&display_path(&exe));
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{MAC_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_xml}</string>
        <string>--autostart</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#
    );
    fs::write(&path, plist).map_err(|err| format!("write {}: {err}", path.display()))
}

#[cfg(target_os = "macos")]
fn macos_remove() -> Result<(), String> {
    let path = macos_plist_path()?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("delete {}: {err}", path.display())),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_desktop_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .ok_or_else(|| "config directory not found".to_string())?;
    Ok(dir.join("autostart").join(LINUX_DESKTOP))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_command() -> Result<Option<String>, String> {
    let path = linux_desktop_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    if desktop_hidden(&data) {
        return Ok(None);
    }
    Ok(desktop_exec(&data))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_hidden(data: &str) -> bool {
    data.lines().any(|line| {
        let line = line.trim();
        line.eq_ignore_ascii_case("Hidden=true")
            || line.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_exec(data: &str) -> Option<String> {
    data.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Exec=")
            .map(|value| value.trim().to_string())
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_install() -> Result<(), String> {
    let exe = current_exe()?;
    let path = linux_desktop_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    let exec = command_for(&exe);
    let desktop = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=Orbitry\n\
         Comment=A local command center for every app in orbit.\n\
         Exec={exec}\n\
         Terminal=false\n\
         Categories=Development;Utility;\n\
         X-GNOME-Autostart-enabled=true\n"
    );
    fs::write(&path, desktop).map_err(|err| format!("write {}: {err}", path.display()))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_remove() -> Result<(), String> {
    let path = linux_desktop_path()?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("delete {}: {err}", path.display())),
    }
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

