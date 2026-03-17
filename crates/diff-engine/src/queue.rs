use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::DiffJob;

/// Maximum number of tasks in queue (hardcoded to 1)
pub const MAX_QUEUE_SIZE: usize = 1;

/// Latest-job-priority queue. Discards older pending jobs and always
/// processes the newest one. This prevents latency from old requests.
///
/// Example flow:
/// - Frontend sends A, B, C, D, E rapidly
/// - A starts processing
/// - B arrives → queued as pending
/// - C arrives → replaces B (B is discarded)
/// - D arrives → replaces C (C is discarded)
/// - E arrives → replaces D (D is discarded)
/// - When A finishes, E is processed immediately!
pub struct JobQueue {
    /// The single pending job that will be processed next.
    /// When a new job arrives, it replaces any existing pending job.
    pending_job: Arc<Mutex<Option<DiffJob>>>,
    /// Notification for when a new job is available
    notify: Arc<tokio::sync::Notify>,
    /// Tracks the current job being processed
    current_job_id: Arc<Mutex<Option<Uuid>>>,
}

/// Receiver side of the queue. Workers use this to fetch pending jobs.
pub struct JobReceiver {
    pub pending_job: Arc<Mutex<Option<DiffJob>>>,
    pub notify: Arc<tokio::sync::Notify>,
}

impl JobQueue {
    /// Create a new latest-job-priority queue.
    pub fn new() -> (Self, JobReceiver) {
        let pending_job = Arc::new(Mutex::new(None));
        let notify = Arc::new(tokio::sync::Notify::new());

        (
            Self {
                pending_job: pending_job.clone(),
                notify: notify.clone(),
                current_job_id: Arc::new(Mutex::new(None)),
            },
            JobReceiver {
                pending_job,
                notify,
            },
        )
    }

    /// Enqueue a job, replacing any existing pending job.
    /// This never blocks - new jobs always replace old pending jobs.
    /// Only the currently running job cannot be interrupted.
    pub async fn enqueue(&self, job: DiffJob) -> Result<(), String> {
        let job_id = job.id;

        // Store the new job (replaces any pending job)
        {
            let mut pending = self.pending_job.lock().await;
            if let Some(old_job) = pending.take() {
                log::debug!(
                    "Discarding pending job {} in favor of {}",
                    old_job.id,
                    job_id
                );
            }
            *pending = Some(job);
        }

        // Update tracking
        *self.current_job_id.lock().await = Some(job_id);

        // Notify workers that a new job is available
        self.notify.notify_one();

        log::debug!("Job {} enqueued (latest-job-priority)", job_id);
        Ok(())
    }

    /// Clear the pending queue (but not the current running job).
    pub async fn clear(&self) {
        let mut pending = self.pending_job.lock().await;
        if pending.take().is_some() {
            log::warn!("Queue cleared - pending job discarded");
        }
    }

    /// Cancel (kill) the current running task by removing it from tracking.
    pub async fn cancel_current(&self) {
        let mut current = self.current_job_id.lock().await;
        if let Some(job_id) = current.take() {
            log::warn!("Cancelling current job: {}", job_id);
        }
    }

    /// Check if a job has been superseded by a newer one.
    /// Returns true if the job should be abandoned:
    ///   - a newer job was enqueued after this one (current_job_id != job_id)
    ///   - or the job was explicitly cancelled (current_job_id is None)
    pub async fn is_job_cancelled(&self, job_id: Uuid) -> bool {
        let current = self.current_job_id.lock().await;
        match *current {
            // This job is still the latest enqueued — proceed
            Some(current_id) => current_id != job_id,
            // Explicitly cancelled via cancel_current()
            None => true,
        }
    }

    /// Get the current job ID being processed
    pub async fn current_job(&self) -> Option<Uuid> {
        self.current_job_id.lock().await.clone()
    }
}
