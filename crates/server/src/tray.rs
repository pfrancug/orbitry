use std::thread;
use std::time::{Duration, Instant};

use crate::browser::open_dashboard;
use crate::instance;
use crate::shutdown::{start_exit_watchdog, Shutdown};
use crate::update::{is_dev_build, UpdateService, VersionInfo};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

const TRAY_RGBA: &[u8] = include_bytes!("../assets/tray-32.rgba");
const TRAY_SIZE: u32 = 32;

pub fn run(port: u16, shutdown: Shutdown, update: UpdateService) -> Result<(), String> {
    let mut event_loop = EventLoopBuilder::new().build();

    let icon = tray_icon::Icon::from_rgba(TRAY_RGBA.to_vec(), TRAY_SIZE, TRAY_SIZE)
        .map_err(|err| format!("tray icon: {err}"))?;

    let menu = Menu::new();
    let open_item = MenuItem::new("Otwórz", true, None);
    let check_item = MenuItem::new("Sprawdź aktualizacje", true, None);
    let update_item = MenuItem::new("Aktualizuj", false, None);
    let quit_item = MenuItem::new("Wyjdź", true, None);
    menu.append_items(&[
        &open_item,
        &PredefinedMenuItem::separator(),
        &check_item,
        &update_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .map_err(|err| format!("tray menu: {err}"))?;

    let open_id = open_item.id().clone();
    let check_id = check_item.id().clone();
    let update_id = update_item.id().clone();
    let quit_id = quit_item.id().clone();

    let tray = TrayIconBuilder::new()
        .with_tooltip(&tooltip(&update.snapshot(true)))
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|err| format!("tray: {err}"))?;

    let menu_rx = MenuEvent::receiver();
    let tray_rx = TrayIconEvent::receiver();
    let url = instance::dashboard_url(port);
    let mut last_label = String::new();
    let mut last_enabled = false;
    let mut last_tooltip = tooltip(&update.snapshot(true));

    event_loop.run_return(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(1));
        hide_helper_windows();

        let info = update.snapshot(true);
        let (label, enabled) = update_item_state(&info);
        if label != last_label || enabled != last_enabled {
            update_item.set_text(&label);
            update_item.set_enabled(enabled);
            last_label = label;
            last_enabled = enabled;
        }
        let next_tooltip = tooltip(&info);
        if next_tooltip != last_tooltip {
            let _ = tray.set_tooltip(Some(&next_tooltip));
            last_tooltip = next_tooltip;
        }

        if let Ok(event) = menu_rx.try_recv() {
            if event.id == open_id {
                open_dashboard(&url);
            } else if event.id == check_id {
                start_manual_check(update.clone());
            } else if event.id == update_id {
                confirm_and_install(update.clone(), shutdown.clone(), port);
            } else if event.id == quit_id {
                shutdown.request_quit();
                start_exit_watchdog(5);
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

        if shutdown.is_requested() {
            start_exit_watchdog(5);
            *control_flow = ControlFlow::Exit;
        }
    });
    Ok(())
}

fn update_item_state(info: &VersionInfo) -> (String, bool) {
    if info.installing {
        return ("Aktualizowanie…".into(), false);
    }
    if info.update_available {
        if let Some(latest) = &info.latest {
            return (format!("Aktualizuj do {latest}"), true);
        }
    }
    ("Aktualizuj".into(), false)
}

fn tooltip(info: &VersionInfo) -> String {
    if let Some(text) = crate::update::phase_tooltip(info) {
        return text;
    }
    if info.update_available {
        if let Some(latest) = &info.latest {
            return format!("Orbitry · dostępna {latest}");
        }
    }
    format!("Orbitry · {}", info.current)
}

fn start_manual_check(update: UpdateService) {
    thread::spawn(move || {
        if is_dev_build() {
            show_message(
                "Orbitry",
                &format!(
                    "Wersja deweloperska: {}. Aktualizacje są wyłączone w buildach deweloperskich.",
                    crate::update::current_version_label()
                ),
                false,
            );
            return;
        }
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                show_message("Orbitry", &format!("Nie udało się sprawdzić aktualizacji: {err}"), true);
                return;
            }
        };
        match runtime.block_on(update.check()) {
            Ok(info) if info.update_available => {
                let latest = info.latest.unwrap_or_else(|| "nowa wersja".into());
                show_message(
                    "Orbitry",
                    &format!("Dostępna jest {latest}. Wybierz „Aktualizuj” w menu, aby ją zainstalować."),
                    false,
                );
            }
            Ok(_) => {
                show_message(
                    "Orbitry",
                    &format!("Masz najnowszą wersję ({}).", crate::update::current_version_label()),
                    false,
                );
            }
            Err(err) => {
                show_message("Orbitry", &format!("Nie udało się sprawdzić aktualizacji: {err}"), true);
            }
        }
    });
}

fn confirm_and_install(update: UpdateService, shutdown: Shutdown, port: u16) {
    let info = update.snapshot(true);
    if !info.update_available || info.installing {
        return;
    }
    let latest = info.latest.clone().unwrap_or_else(|| "nową wersję".into());
    let message = format!(
        "Zainstalować Orbitry {latest}? Orbitry uruchomi się ponownie. Uruchomione aplikacje pozostaną włączone."
    );
    if !confirm("Orbitry", &message) {
        return;
    }
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                show_message("Orbitry", &format!("Aktualizacja nie powiodła się: {err}"), true);
                return;
            }
        };
        if let Err(err) = runtime.block_on(update.install(shutdown, port)) {
            show_message("Orbitry", &format!("Aktualizacja nie powiodła się: {err}"), true);
        }
    });
}

fn confirm(title: &str, message: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::OkCancel)
        .set_level(rfd::MessageLevel::Warning)
        .show()
        == rfd::MessageDialogResult::Ok
}

fn show_message(title: &str, message: &str, error: bool) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .set_level(if error {
            rfd::MessageLevel::Error
        } else {
            rfd::MessageLevel::Info
        })
        .show();
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
