use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

const NONE: u8 = 0;
const QUIT: u8 = 1;
const UPDATE: u8 = 2;

#[derive(Clone, Default)]
pub struct Shutdown {
    reason: Arc<AtomicU8>,
    notify: Arc<Notify>,
}

impl Shutdown {
    pub fn new() -> Self {
        Self::default()
    }

    fn set_if_unset(&self, value: u8) -> bool {
        self.reason
            .compare_exchange(NONE, value, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Explicit quit (tray "Wyjdź" or OS session end). Managed apps are stopped.
    pub fn request_quit(&self) {
        if self.set_if_unset(QUIT) {
            self.notify.notify_waiters();
        } else if self.is_requested() {
            self.notify.notify_waiters();
        }
    }

    /// Updater is replacing this process. Managed apps must keep running.
    pub fn request_update(&self) {
        if self.set_if_unset(UPDATE) {
            self.notify.notify_waiters();
        } else if self.is_requested() {
            self.notify.notify_waiters();
        }
    }

    /// Backward-compatible alias for an explicit quit.
    pub fn request(&self) {
        self.request_quit();
    }

    pub fn is_requested(&self) -> bool {
        self.reason.load(Ordering::SeqCst) != NONE
    }

    pub fn stops_apps(&self) -> bool {
        self.reason.load(Ordering::SeqCst) == QUIT
    }

    pub fn is_update(&self) -> bool {
        self.reason.load(Ordering::SeqCst) == UPDATE
    }

    pub async fn wait(&self) {
        loop {
            if self.is_requested() {
                return;
            }
            self.notify.notified().await;
        }
    }
}

/// If graceful shutdown has not finished after `secs` seconds, force-exit.
pub fn start_exit_watchdog(secs: u64) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("orbitry-exit-watchdog".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            std::process::exit(0);
        })
        .ok();
}
