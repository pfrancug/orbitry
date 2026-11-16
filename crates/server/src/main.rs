#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod browser;
mod console_win;
mod instance;
mod notify;
mod open;
mod server;
mod shutdown;
mod tray;
mod ui;
mod update;

use std::path::PathBuf;

use clap::Parser;
use orbitry_core::{config_path, load_config, save_config};
use tokio::sync::mpsc;

use crate::browser::{open_dashboard, show_error};
use crate::shutdown::{start_exit_watchdog, Shutdown};
use crate::update::UpdateService;

#[derive(Parser, Debug)]
#[command(
    name = "orbitry",
    version,
    about = "A local command center for every app in orbit."
)]
struct Args {
    /// Project root to scan (repeatable). Added to the persisted user config.
    #[arg(long)]
    root: Vec<PathBuf>,

    /// HTTP port for the dashboard and API.
    #[arg(long, default_value_t = 4280)]
    port: u16,

    /// Open the dashboard in the browser after start.
    #[arg(long)]
    open: bool,

    /// Kept for compatibility. The browser is not opened unless --open is set.
    #[arg(long, hide = true)]
    no_open: bool,

    /// Attach a console window so a release build can print logs.
    #[arg(long)]
    console: bool,

    /// Wait for a previous Orbitry process to exit, then take over. Used by the updater.
    #[arg(long, hide = true)]
    replace_wait: Option<u32>,

    /// Start silently in the tray (used by OS login autostart). Does not open the browser.
    #[arg(long, hide = true)]
    autostart: bool,
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    if raw.get(1).map(String::as_str) == Some("--internal-log-pump") {
        std::process::exit(orbitry_core::run_log_pump(&raw[2..]));
    }
    let args = Args::parse();
    if args.console {
        console_win::enable();
    }
    if let Err(err) = run(args) {
        eprintln!("orbitry: {err}");
        show_error(&err);
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    if let Some(pid) = args.replace_wait {
        match crate::update::complete_replace(pid, args.port)? {
            crate::update::ReplaceOutcome::LaunchedSuccessor => return Ok(()),
            crate::update::ReplaceOutcome::Continue => {}
        }
    }

    crate::update::cleanup_stale_exe();
    let _ = orbitry_core::refresh_autostart_path();

    let path = config_path();
    let mut config = load_config(&path)?;
    for root in &args.root {
        let displayed = orbitry_core::display_path(&orbitry_core::canonical(root));
        config = config.with_root(displayed);
    }
    if !args.root.is_empty() {
        save_config(&path, &config)?;
    }

    let lock = match if args.replace_wait.is_some() {
        instance::acquire_with_retry(std::time::Duration::from_secs(10))?
    } else {
        instance::acquire()?
    } {
        Some(lock) => lock,
        None => {
            if !args.autostart {
                open_dashboard(&instance::running_url(args.port));
            }
            return Ok(());
        }
    };

    let shutdown = Shutdown::new();
    let update = UpdateService::new(config.auto_check_updates);
    let (ready_tx, mut ready_rx) = mpsc::unbounded_channel();
    let server_path = path.clone();
    let server_port = args.port;
    let server_shutdown = shutdown.clone();
    let server_update = update.clone();
    let server_thread = std::thread::Builder::new()
        .name("orbitry-server".into())
        .spawn(move || {
            server::run_blocking(server_path, server_port, server_shutdown, server_update, ready_tx);
        })
        .map_err(|err| format!("spawn server thread: {err}"))?;

    let bound_port = match ready_rx.blocking_recv() {
        Some(Ok(port)) => port,
        Some(Err(err)) if err == "already-bound" => {
            drop(lock);
            if !args.autostart {
                open_dashboard(&instance::running_url(args.port));
            }
            let _ = server_thread.join();
            return Ok(());
        }
        Some(Err(err)) => return Err(err),
        None => return Err("server thread exited before becoming ready".into()),
    };

    if args.open && !args.no_open && !args.autostart {
        open_dashboard(&instance::dashboard_url(bound_port));
    }

    if let Err(err) = tray::run(bound_port, shutdown.clone(), update.clone()) {
        eprintln!("orbitry: {err}");
        // Keep serving if the tray cannot start (for example a headless session).
        // Ctrl+C or closing the process still stops child runs.
        let _ = server_thread.join();
        drop(lock);
        return Err(err);
    }

    // Do not overwrite an in-flight update shutdown (apps must keep running).
    if !shutdown.is_requested() {
        shutdown.request_quit();
    }
    start_exit_watchdog(5);
    let _ = server_thread.join();
    drop(lock);
    Ok(())
}
