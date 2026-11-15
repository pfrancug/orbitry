pub fn open_dashboard(url: &str) {
    #[cfg(windows)]
    {
        open_windows(url);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

#[cfg(windows)]
fn open_windows(url: &str) {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let operation = wide("open");
    let target = wide(url);
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

#[cfg(windows)]
pub fn show_error(message: &str) {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let title = wide("Orbitry");
    let body = wide(message);
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
                | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR
                | windows_sys::Win32::UI::WindowsAndMessaging::MB_SETFOREGROUND,
        );
    }
}

#[cfg(not(windows))]
pub fn show_error(_message: &str) {}
