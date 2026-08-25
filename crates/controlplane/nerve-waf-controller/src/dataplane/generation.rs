use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::watch;

#[derive(Clone)]
pub struct DataplaneGeneration {
    pub counter: Arc<AtomicU64>,
    pub notify: watch::Sender<u64>,
}

impl DataplaneGeneration {
    pub fn new() -> Self {
        let (notify, _) = watch::channel(0u64);
        Self {
            counter: Arc::new(AtomicU64::new(0)),
            notify,
        }
    }

    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    pub fn bump(&self) -> u64 {
        let next = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.notify.send(next);
        next
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.notify.subscribe()
    }
}

impl Default for DataplaneGeneration {
    fn default() -> Self {
        Self::new()
    }
}

pub fn bump_dataplane_generation(state: &crate::AppState) -> u64 {
    state.dataplane_gen.bump()
}

pub fn dataplane_generation(state: &crate::AppState) -> u64 {
    state.dataplane_gen.current()
}
