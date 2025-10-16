use crate::config::WatcherConfig;
use crate::error::{WatcherError, WatcherResult};
use crate::events::{BatchUpdate, FileEvent, FileEventType, WatcherState};
use crate::filter::GitignoreFilter;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Callback type for file events
pub type EventCallback = Arc<dyn Fn(BatchUpdate) + Send + Sync>;

/// Repository file watcher
pub struct RepoWatcher {
    repo_path: PathBuf,
    config: WatcherConfig,
    filter: Arc<Mutex<GitignoreFilter>>,
    state: Arc<Mutex<WatcherState>>,
    running: Arc<AtomicBool>,
    batch_id: Arc<AtomicU64>,
    event_callback: Arc<Mutex<Option<EventCallback>>>,
    debouncer: Arc<Mutex<Option<Debouncer<RecommendedWatcher, FileIdMap>>>>,
    start_time: Instant,
}

impl RepoWatcher {
    /// Create a new repository watcher
    pub fn new<P: AsRef<Path>>(repo_path: P, config: WatcherConfig) -> WatcherResult<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();

        // Validate config
        config.validate().map_err(WatcherError::ConfigError)?;

        // Validate path
        if !repo_path.exists() {
            return Err(WatcherError::PathNotFound(repo_path.display().to_string()));
        }

        if !repo_path.is_dir() {
            return Err(WatcherError::InvalidPath(
                "Path must be a directory".to_string(),
            ));
        }

        // Create gitignore filter
        let filter = GitignoreFilter::new(&repo_path, config.extra_ignores.clone())?;

        info!("Created RepoWatcher for {:?}", repo_path);

        Ok(Self {
            repo_path,
            config,
            filter: Arc::new(Mutex::new(filter)),
            state: Arc::new(Mutex::new(WatcherState::Initializing)),
            running: Arc::new(AtomicBool::new(false)),
            batch_id: Arc::new(AtomicU64::new(0)),
            event_callback: Arc::new(Mutex::new(None)),
            debouncer: Arc::new(Mutex::new(None)),
            start_time: Instant::now(),
        })
    }

    /// Set event callback
    pub fn set_callback<F>(&self, callback: F)
    where
        F: Fn(BatchUpdate) + Send + Sync + 'static,
    {
        let mut cb = self.event_callback.lock().unwrap();
        *cb = Some(Arc::new(callback));
    }

    /// Start watching the repository
    pub fn start(&self) -> WatcherResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(WatcherError::AlreadyRunning);
        }

        info!("Starting watcher for {:?}", self.repo_path);

        // Create event channel for batching
        let (tx, rx) = mpsc::unbounded_channel::<FileEvent>();

        // Clone Arc references for the debouncer callback
        let tx_clone = tx.clone();
        let filter_clone = Arc::clone(&self.filter);
        let repo_path_clone = self.repo_path.clone();

        // Create debouncer
        let mut debouncer = new_debouncer(
            self.config.debounce_duration(),
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for event in events {
                        if let Some(file_event) =
                            Self::process_event(&event.event, &filter_clone, &repo_path_clone)
                        {
                            if let Err(e) = tx_clone.send(file_event) {
                                error!("Failed to send event: {}", e);
                            }
                        }
                    }
                }
                Err(errors) => {
                    for error in errors {
                        error!("Watch error: {:?}", error);
                    }
                }
            },
        )
        .map_err(|e| WatcherError::NotifyError(e.to_string()))?;

        // Start watching
        debouncer
            .watcher()
            .watch(&self.repo_path, RecursiveMode::Recursive)?;

        // Store debouncer
        *self.debouncer.lock().unwrap() = Some(debouncer);

        // Update state
        *self.state.lock().unwrap() = WatcherState::Active;
        self.running.store(true, Ordering::SeqCst);

        // Start batch processor
        self.start_batch_processor(rx);

        info!("Watcher started successfully");

        Ok(())
    }

    /// Stop watching
    pub fn stop(&self) -> WatcherResult<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        info!("Stopping watcher");

        self.running.store(false, Ordering::SeqCst);
        *self.state.lock().unwrap() = WatcherState::Stopped;

        // Drop the debouncer to stop watching
        *self.debouncer.lock().unwrap() = None;

        info!("Watcher stopped");

        Ok(())
    }

    /// Process a notify event into a FileEvent
    fn process_event(
        event: &Event,
        filter: &Arc<Mutex<GitignoreFilter>>,
        repo_path: &Path,
    ) -> Option<FileEvent> {
        let filter = filter.lock().unwrap();

        // Get the main path from the event
        let path = event.paths.first()?;

        // Check if path should be ignored
        if filter.is_ignored(path) {
            debug!("Ignoring event for {:?}", path);
            return None;
        }

        // Get relative path
        let relative_path = filter.make_relative(path)?;
        let relative_str = relative_path.to_string_lossy().to_string();
        let absolute_str = path.to_string_lossy().to_string();

        // Determine event type
        let event_type = match event.kind {
            EventKind::Create(_) => FileEventType::Created,
            EventKind::Modify(_) => FileEventType::Modified,
            EventKind::Remove(_) => FileEventType::Deleted,
            // EventKind::Rename(_, _) => FileEventType::Renamed,
            _ => return None, // Ignore other event types
        };

        debug!("Processing {:?} event for {:?}", event_type, relative_str);

        // Create file event
        let mut file_event = FileEvent::new(event_type, relative_str, absolute_str);

        // Add metadata if file still exists
        if let Some(metadata) = FileEvent::metadata_from_path(path) {
            file_event = file_event.with_metadata(metadata);
        }

        // Handle rename events
        if event_type == FileEventType::Renamed && event.paths.len() > 1 {
            if let Some(old_path) = filter.make_relative(&event.paths[1]) {
                file_event = file_event.with_old_path(old_path.to_string_lossy().to_string());
            }
        }

        Some(file_event)
    }

    /// Start the batch processor task
    fn start_batch_processor(&self, mut rx: mpsc::UnboundedReceiver<FileEvent>) {
        let callback = Arc::clone(&self.event_callback);
        let batch_id = Arc::clone(&self.batch_id);
        let batch_size = self.config.batch_size;
        let batch_timeout = self.config.batch_timeout();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut pending_events = Vec::new();
            let mut last_flush = Instant::now();

            while running.load(Ordering::SeqCst) {
                // Try to receive events with timeout
                match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
                    Ok(Some(event)) => {
                        pending_events.push(event);

                        // Flush if batch is full
                        if pending_events.len() >= batch_size {
                            Self::flush_batch(&mut pending_events, &callback, &batch_id);
                            last_flush = Instant::now();
                        }
                    }
                    Ok(None) => {
                        // Channel closed
                        break;
                    }
                    Err(_) => {
                        // Timeout - check if we should flush based on time
                        if !pending_events.is_empty() && last_flush.elapsed() >= batch_timeout {
                            Self::flush_batch(&mut pending_events, &callback, &batch_id);
                            last_flush = Instant::now();
                        }
                    }
                }
            }

            // Flush remaining events
            if !pending_events.is_empty() {
                Self::flush_batch(&mut pending_events, &callback, &batch_id);
            }

            info!("Batch processor stopped");
        });
    }

    /// Flush pending events as a batch
    fn flush_batch(
        events: &mut Vec<FileEvent>,
        callback: &Arc<Mutex<Option<EventCallback>>>,
        batch_id: &Arc<AtomicU64>,
    ) {
        if events.is_empty() {
            return;
        }

        let id = batch_id.fetch_add(1, Ordering::SeqCst);
        let batch = BatchUpdate::new(std::mem::take(events), id);

        debug!("Flushing batch {} with {} events", id, batch.len());

        if let Some(cb) = callback.lock().unwrap().as_ref() {
            cb(batch);
        }
    }

    /// Get current state
    pub fn get_state(&self) -> WatcherState {
        *self.state.lock().unwrap()
    }

    /// Get repository path
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Get uptime in seconds
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Reload gitignore rules
    pub fn reload_gitignore(&self) -> WatcherResult<()> {
        let mut filter = self.filter.lock().unwrap();
        filter.reload()?;
        info!("Reloaded gitignore rules");
        Ok(())
    }
}

impl Drop for RepoWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicUsize;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        fs::create_dir_all(repo_path.join("src")).unwrap();
        fs::write(repo_path.join(".gitignore"), "*.log\n").unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_watcher_creation() {
        let (_temp, repo_path) = create_test_repo();
        let config = WatcherConfig::default();
        let watcher = RepoWatcher::new(&repo_path, config).unwrap();

        assert_eq!(watcher.repo_path(), repo_path);
        assert_eq!(watcher.get_state(), WatcherState::Initializing);
    }

    #[test]
    fn test_invalid_path() {
        let config = WatcherConfig::default();
        let result = RepoWatcher::new("/nonexistent/path", config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_stop() {
        let (_temp, repo_path) = create_test_repo();
        let config = WatcherConfig::default();
        let watcher = RepoWatcher::new(&repo_path, config).unwrap();

        watcher.start().unwrap();
        assert!(watcher.is_running());
        assert_eq!(watcher.get_state(), WatcherState::Active);

        watcher.stop().unwrap();
        assert!(!watcher.is_running());
        assert_eq!(watcher.get_state(), WatcherState::Stopped);
    }

    #[tokio::test]
    async fn test_event_callback() {
        let (_temp, repo_path) = create_test_repo();
        let config = WatcherConfig::default();
        let watcher = RepoWatcher::new(&repo_path, config).unwrap();

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = Arc::clone(&event_count);

        watcher.set_callback(move |batch| {
            event_count_clone.fetch_add(batch.len(), Ordering::SeqCst);
        });

        watcher.start().unwrap();

        // Create a test file
        tokio::time::sleep(Duration::from_millis(100)).await;
        fs::write(repo_path.join("test.txt"), "content").unwrap();

        // Wait for event processing
        tokio::time::sleep(Duration::from_millis(500)).await;

        watcher.stop().unwrap();

        // Should have received at least one event
        assert!(event_count.load(Ordering::SeqCst) > 0);
    }
}
