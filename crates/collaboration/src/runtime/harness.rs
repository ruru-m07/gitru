//! Feature-only injection into the actual runtime; no alternate scheduler.
use super::*;
use crate::test_harness::HarnessClock;

impl clock::Clock for HarnessClock {
    fn now(&self) -> Instant {
        self.instant()
    }
    fn utc(&self) -> DateTime<Utc> {
        self.utc_time()
    }
    fn jitter(&self) -> u64 {
        0
    }
}

impl CollaborationRuntime {
    pub(crate) fn with_harness_clock(mut self, clock: Arc<HarnessClock>) -> Self {
        self.clock = clock;
        self
    }

    pub(crate) async fn harness_lease_count(&self) -> u32 {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.demands.expire(self.now());
        scheduler.demands.leases.len() as u32
    }

    pub(crate) fn harness_wake(&self) {
        self.notify.notify_one();
    }

    pub(crate) async fn harness_publish_current(&self) -> Result<(), CollaborationError> {
        self.publish(self.store.revision().await?);
        self.notify.notify_one();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn harness_run_next(&self) -> bool {
        self.run_next().await
    }
}
