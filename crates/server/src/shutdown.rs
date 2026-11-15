use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

#[derive(Clone, Default)]
pub struct Shutdown {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Shutdown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub async fn wait(&self) {
        loop {
            if self.flag.load(Ordering::SeqCst) {
                return;
            }
            self.notify.notified().await;
        }
    }
}
