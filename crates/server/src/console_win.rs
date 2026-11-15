/// Attach or allocate a console so a GUI-subsystem exe can print logs.
pub fn enable() {
    #[cfg(windows)]
    unsafe {
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
        let attached =
            windows_sys::Win32::System::Console::AttachConsole(ATTACH_PARENT_PROCESS) != 0;
        if !attached {
            windows_sys::Win32::System::Console::AllocConsole();
        }
        reopen_stdio();
    }
}

#[cfg(all(windows, target_env = "msvc"))]
fn reopen_stdio() {
    use std::ffi::CString;

    extern "C" {
        fn freopen(
            filename: *const i8,
            mode: *const i8,
            stream: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn __acrt_iob_func(index: u32) -> *mut std::ffi::c_void;
    }

    unsafe {
        let conout = CString::new("CONOUT$").expect("CONOUT$");
        let conin = CString::new("CONIN$").expect("CONIN$");
        let write = CString::new("w").expect("w");
        let read = CString::new("r").expect("r");
        let _ = freopen(conin.as_ptr(), read.as_ptr(), __acrt_iob_func(0));
        let _ = freopen(conout.as_ptr(), write.as_ptr(), __acrt_iob_func(1));
        let _ = freopen(conout.as_ptr(), write.as_ptr(), __acrt_iob_func(2));
    }
}

#[cfg(all(windows, not(target_env = "msvc")))]
fn reopen_stdio() {}
