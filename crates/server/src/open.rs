//! User-triggered helpers: native folder picker, terminal, VS Code, Cursor, and the file manager.
//!
//! These spawn visible UI only in response to an explicit API call from the
//! dashboard. Verification must never invoke them.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use orbitry_core::display_path;

pub fn pick_folder() -> Result<Option<PathBuf>, String> {
    let picked = rfd::FileDialog::new()
        .set_title("Choose a projects folder")
        .pick_folder();
    Ok(picked.map(|path| {
        let rendered = display_path(&path);
        PathBuf::from(rendered)
    }))
}

pub fn open_in_terminal(path: &Path) -> Result<(), String> {
    let path = require_dir(path)?;
    #[cfg(windows)]
    {
        return open_terminal_windows(path);
    }
    #[cfg(target_os = "macos")]
    {
        return spawn_visible("open", &["-a", "Terminal", &display_path(path)], Some(path));
    }
    #[cfg(target_os = "linux")]
    {
        return open_terminal_linux(path);
    }
    #[allow(unreachable_code)]
    Err("opening a terminal is not supported on this platform".into())
}

pub fn vscode_installed() -> bool {
    #[cfg(windows)]
    {
        return windows_vscode_exe().is_some();
    }
    #[cfg(target_os = "macos")]
    {
        if look_on_path(&["code", "code-insiders", "codium"]).is_some() {
            return true;
        }
        const APPS: &[&str] = &[
            "/Applications/Visual Studio Code.app",
            "/Applications/Visual Studio Code - Insiders.app",
            "/Applications/VSCodium.app",
        ];
        return APPS.iter().any(|path| Path::new(path).exists());
    }
    #[cfg(target_os = "linux")]
    {
        return look_on_path(&["code", "code-insiders", "codium", "vscodium"]).is_some();
    }
    #[allow(unreachable_code)]
    false
}

pub fn open_in_vscode(path: &Path) -> Result<(), String> {
    let path = require_dir(path)?;
    #[cfg(windows)]
    {
        return open_vscode_windows(path);
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(exe) = look_on_path(&["code", "code-insiders", "codium"]) {
            return spawn_detached(&exe, &[&display_path(path)], Some(path), false);
        }
        return spawn_visible(
            "open",
            &["-a", "Visual Studio Code", &display_path(path)],
            Some(path),
        );
    }
    #[cfg(target_os = "linux")]
    {
        let exe = look_on_path(&["code", "code-insiders", "codium", "vscodium"])
            .ok_or_else(|| "VS Code was not found on PATH".to_string())?;
        return spawn_detached(&exe, &[&display_path(path)], Some(path), false);
    }
    #[allow(unreachable_code)]
    Err("opening VS Code is not supported on this platform".into())
}

pub fn cursor_installed() -> bool {
    #[cfg(windows)]
    {
        return windows_cursor_exe().is_some();
    }
    #[cfg(target_os = "macos")]
    {
        if look_on_path(&["cursor"]).is_some() {
            return true;
        }
        return Path::new("/Applications/Cursor.app").exists();
    }
    #[cfg(target_os = "linux")]
    {
        return look_on_path(&["cursor"]).is_some()
            || Path::new("/usr/share/cursor/cursor").exists()
            || Path::new("/opt/Cursor/cursor").exists()
            || Path::new("/opt/cursor/cursor").exists();
    }
    #[allow(unreachable_code)]
    false
}

/// Open a folder in the Cursor IDE editor window (not the Agents window).
///
/// Uses `--classic`, which Cursor documents as always opening the classic
/// editor and bypassing Agents Window routing.
pub fn open_in_cursor(path: &Path) -> Result<(), String> {
    let path = require_dir(path)?;
    let dir = display_path(path);
    #[cfg(windows)]
    {
        return open_cursor_windows(path, &dir);
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(exe) = look_on_path(&["cursor"]) {
            return spawn_detached(&exe, &["--classic", &dir], Some(path), false);
        }
        if Path::new("/Applications/Cursor.app").exists() {
            return spawn_visible(
                "open",
                &["-a", "Cursor", "--args", "--classic", &dir],
                Some(path),
            );
        }
        return Err("Cursor was not found on this machine".into());
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(exe) = look_on_path(&["cursor"]) {
            return spawn_detached(&exe, &["--classic", &dir], Some(path), false);
        }
        for candidate in [
            "/usr/share/cursor/cursor",
            "/opt/Cursor/cursor",
            "/opt/cursor/cursor",
        ] {
            let exe = Path::new(candidate);
            if exe.is_file() {
                return spawn_detached(exe, &["--classic", &dir], Some(path), false);
            }
        }
        return Err("Cursor was not found on PATH".into());
    }
    #[allow(unreachable_code)]
    Err("opening Cursor is not supported on this platform".into())
}

#[cfg(windows)]
fn windows_cursor_exe() -> Option<PathBuf> {
    look_on_path(&["Cursor.exe", "cursor.exe"])
        .or_else(|| windows_home_bin(r"Programs\cursor\Cursor.exe"))
        .or_else(|| windows_home_bin(r"Programs\Cursor\Cursor.exe"))
        .or_else(|| program_files(r"Cursor\Cursor.exe"))
        .or_else(|| look_on_path(&["cursor.cmd", "cursor"]))
}

#[cfg(windows)]
fn open_cursor_windows(path: &Path, dir: &str) -> Result<(), String> {
    let exe = windows_cursor_exe().ok_or_else(|| "Cursor was not found on this machine".to_string())?;
    let hide_shim = exe
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
    spawn_detached(&exe, &["--classic", dir], Some(path), hide_shim)
}

pub fn open_in_explorer(path: &Path) -> Result<(), String> {
    let path = require_dir(path)?;
    let rendered = display_path(path);
    #[cfg(windows)]
    {
        return spawn_detached(Path::new("explorer.exe"), &[&rendered], None, true);
    }
    #[cfg(target_os = "macos")]
    {
        return spawn_visible("open", &[&rendered], None);
    }
    #[cfg(target_os = "linux")]
    {
        return spawn_visible("xdg-open", &[&rendered], None);
    }
    #[allow(unreachable_code)]
    Err("opening the file manager is not supported on this platform".into())
}

fn require_dir(path: &Path) -> Result<&Path, String> {
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!("folder not found: {}", path.display()))
    }
}

#[cfg(windows)]
fn open_terminal_windows(path: &Path) -> Result<(), String> {
    let dir = display_path(path);
    if let Some(wt) = look_on_path(&["wt.exe", "wt"])
        .or_else(|| windows_home_bin(r"Microsoft\WindowsApps\wt.exe"))
    {
        return spawn_detached(&wt, &["-d", &dir], Some(path), true);
    }
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/K", "cd", "/d", &dir]);
    cmd.current_dir(path);
    cmd.stdin(Stdio::null());
    visible_console(&mut cmd);
    cmd.spawn()
        .map(|_| ())
        .map_err(|err| format!("open terminal: {err}"))
}

#[cfg(windows)]
fn windows_vscode_exe() -> Option<PathBuf> {
    look_on_path(&["Code.exe", "Code - Insiders.exe", "VSCodium.exe", "code.exe"])
        .or_else(|| windows_home_bin(r"Programs\Microsoft VS Code\Code.exe"))
        .or_else(|| windows_home_bin(r"Programs\Microsoft VS Code Insiders\Code - Insiders.exe"))
        .or_else(|| program_files(r"Microsoft VS Code\Code.exe"))
        .or_else(|| program_files(r"Microsoft VS Code Insiders\Code - Insiders.exe"))
        .or_else(|| look_on_path(&["code.cmd", "code-insiders.cmd", "codium.cmd", "code"]))
}

#[cfg(windows)]
fn open_vscode_windows(path: &Path) -> Result<(), String> {
    let dir = display_path(path);
    let exe = windows_vscode_exe().ok_or_else(|| "VS Code was not found on this machine".to_string())?;
    let hide_shim = exe
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
    spawn_detached(&exe, &[&dir], Some(path), hide_shim)
}

#[cfg(windows)]
fn windows_home_bin(relative: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let path = PathBuf::from(local).join(relative);
    path.is_file().then_some(path)
}

#[cfg(windows)]
fn program_files(relative: &str) -> Option<PathBuf> {
    for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(root) = std::env::var_os(key) {
            let path = PathBuf::from(root).join(relative);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn open_terminal_linux(path: &Path) -> Result<(), String> {
    let dir = display_path(path);
    let candidates: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["--working-directory"]),
        ("gnome-terminal", &["--working-directory"]),
        ("konsole", &["--workdir"]),
        ("xfce4-terminal", &["--working-directory"]),
        ("kitty", &["--directory"]),
        ("alacritty", &["--working-directory"]),
        ("wezterm", &["start", "--cwd"]),
        ("tilix", &["--working-directory"]),
    ];
    for (bin, prefix) in candidates {
        if let Some(exe) = look_on_path(&[bin]) {
            let mut args: Vec<&str> = prefix.to_vec();
            args.push(&dir);
            if spawn_detached(&exe, &args, Some(path), false).is_ok() {
                return Ok(());
            }
        }
    }
    Err("no terminal emulator was found on PATH".into())
}

fn look_on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg_attr(windows, allow(dead_code))]
fn spawn_visible(bin: &str, args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null());
    cmd.spawn()
        .map(|_| ())
        .map_err(|err| format!("open {bin}: {err}"))
}

fn spawn_detached(
    bin: &Path,
    args: &[&str],
    cwd: Option<&Path>,
    hide_console: bool,
) -> Result<(), String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if hide_console {
        hide_only(&mut cmd);
    } else {
        detach(&mut cmd);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|err| format!("open {}: {err}", bin.display()))
}

fn hide_only(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let _ = cmd;
}

fn detach(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let _ = cmd;
}

#[cfg(windows)]
fn visible_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    cmd.creation_flags(CREATE_NEW_CONSOLE);
}
