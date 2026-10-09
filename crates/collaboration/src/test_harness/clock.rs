use chrono::{DateTime, Utc};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

pub(crate) struct HarnessClock {
    base: Instant,
    utc_base: DateTime<Utc>,
    elapsed: AtomicU32,
}

impl HarnessClock {
    pub(crate) fn new(utc_base: DateTime<Utc>, elapsed: u32) -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            utc_base,
            elapsed: AtomicU32::new(elapsed),
        })
    }
    pub(crate) fn set_elapsed(&self, elapsed: u32) {
        self.elapsed.store(elapsed, Ordering::SeqCst);
    }
    pub(crate) fn instant(&self) -> Instant {
        self.base + Duration::from_secs(u64::from(self.elapsed.load(Ordering::SeqCst)))
    }
    pub(crate) fn utc_time(&self) -> DateTime<Utc> {
        self.utc_base + chrono::Duration::seconds(i64::from(self.elapsed.load(Ordering::SeqCst)))
    }
}
