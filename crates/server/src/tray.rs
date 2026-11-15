use crate::browser::open_dashboard;
use crate::instance;
use crate::shutdown::Shutdown;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

const TRAY_RGBA: &[u8] = include_bytes!("../assets/tray-32.rgba");
const TRAY_SIZE: u32 = 32;

pub fn run(port: u16, shutdown: Shutdown) -> Result<(), String> {
    let mut event_loop = EventLoopBuilder::new().build();

    let icon = tray_icon::Icon::from_rgba(TRAY_RGBA.to_vec(), TRAY_SIZE, TRAY_SIZE)
        .map_err(|err| format!("tray icon: {err}"))?;

    let menu = Menu::new();
    let open_item = MenuItem::new("Otwórz", true, None);
    let quit_item = MenuItem::new("Wyjdź", true, None);
    menu.append_items(&[&open_item, &PredefinedMenuItem::separator(), &quit_item])
        .map_err(|err| format!("tray menu: {err}"))?;

    let open_id = open_item.id().clone();
    let quit_id = quit_item.id().clone();

    let _tray = TrayIconBuilder::new()
        .with_tooltip("Orbitry")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|err| format!("tray: {err}"))?;

    let menu_rx = MenuEvent::receiver();
    let tray_rx = TrayIconEvent::receiver();
    let url = instance::dashboard_url(port);

    event_loop.run_return(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        hide_helper_windows();

        if let Ok(event) = menu_rx.try_recv() {
            if event.id == open_id {
                open_dashboard(&url);
            } else if event.id == quit_id {
                shutdown.request();
                *control_flow = ControlFlow::Exit;
            }
        }

        if let Ok(event) = tray_rx.try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_dashboard(&url);
            }
        }
    });
    Ok(())
}

/// Hide tao's tiny "Thread Event Target" popup so launching the exe shows no window.
fn hide_helper_windows() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows(Some(hide_helper_cb), 0);
    }
}

#[cfg(windows)]
unsafe extern "system" fn hide_helper_cb(
    hwnd: windows_sys::Win32::Foundation::HWND,
    _: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowThreadProcessId, IsWindowVisible, ShowWindow, SW_HIDE,
    };

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != std::process::id() {
        return 1;
    }
    let mut class = [0u16; 256];
    let count = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
    if count <= 0 {
        return 1;
    }
    let class = String::from_utf16_lossy(&class[..count as usize]);
    if class == "Tao Thread Event Target" && IsWindowVisible(hwnd) != 0 {
        ShowWindow(hwnd, SW_HIDE);
    }
    1
}
