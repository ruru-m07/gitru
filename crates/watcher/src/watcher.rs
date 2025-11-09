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

pub type EventCallback = Arc<dyn Fn(BatchUpdate) + Send + Sync>;

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
    pub fn new<P: AsRef<Path>>(repo_path: P, config: WatcherConfig) -> WatcherResult<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();

        config.validate().map_err(WatcherError::ConfigError)?;

        if !repo_path.exists() {
            return Err(WatcherError::PathNotFound(repo_path.display().to_string()));
        }

        if !repo_path.is_dir() {
            return Err(WatcherError::InvalidPath(
                "Path must be a directory".to_string(),
            ));
        }

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

    pub fn set_callback<F>(&self, callback: F)
    where
        F: Fn(BatchUpdate) + Send + Sync + 'static,
    {
        let mut cb = self.event_callback.lock().unwrap();
        *cb = Some(Arc::new(callback));
    }

    pub fn start(&self) -> WatcherResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(WatcherError::AlreadyRunning);
        }

        info!("Starting watcher for {:?}", self.repo_path);

        let (tx, rx) = mpsc::unbounded_channel::<FileEvent>();

        let tx_clone = tx.clone();
        let filter_clone = Arc::clone(&self.filter);

        let mut debouncer = new_debouncer(
            self.config.debounce_duration(),
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for event in events {
                        if let Some(file_event) = Self::process_event(&event.event, &filter_clone) {
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

        debouncer
            .watcher()
            .watch(&self.repo_path, RecursiveMode::Recursive)?;

        *self.debouncer.lock().unwrap() = Some(debouncer);

        *self.state.lock().unwrap() = WatcherState::Active;
        self.running.store(true, Ordering::SeqCst);

        self.start_batch_processor(rx);

        info!("Watcher started successfully");

        Ok(())
    }

    pub fn stop(&self) -> WatcherResult<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        info!("Stopping watcher");

        self.running.store(false, Ordering::SeqCst);
        *self.state.lock().unwrap() = WatcherState::Stopped;

        *self.debouncer.lock().unwrap() = None;

        info!("Watcher stopped");

        Ok(())
    }

    fn process_event(event: &Event, filter: &Arc<Mutex<GitignoreFilter>>) -> Option<FileEvent> {
        let filter = filter.lock().unwrap();

        let path = event.paths.first()?;

        if filter.is_ignored(path) {
            debug!("Ignoring event for {:?}", path);
            return None;
        }

        let relative_path = filter.make_relative(path)?;
        let relative_str = relative_path.to_string_lossy().to_string();
        let absolute_str = path.to_string_lossy().to_string();

        let event_type = match event.kind {
            EventKind::Create(_) => FileEventType::Created,
            EventKind::Modify(_) => FileEventType::Modified,
            EventKind::Remove(_) => FileEventType::Deleted,
            // EventKind::Rename(_, _) => FileEventType::Renamed,
            _ => return None,
        };

        debug!("Processing {:?} event for {:?}", event_type, relative_str);

        let mut file_event = FileEvent::new(event_type, relative_str, absolute_str);

        if let Some(metadata) = FileEvent::metadata_from_path(path) {
            file_event = file_event.with_metadata(metadata);
        }

        if event_type == FileEventType::Renamed && event.paths.len() > 1 {
            if let Some(old_path) = filter.make_relative(&event.paths[1]) {
                file_event = file_event.with_old_path(old_path.to_string_lossy().to_string());
            }
        }

        Some(file_event)
    }

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
                match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
                    Ok(Some(event)) => {
                        pending_events.push(event);

                        if pending_events.len() >= batch_size {
                            Self::flush_batch(&mut pending_events, &callback, &batch_id);
                            last_flush = Instant::now();
                        }
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(_) => {
                        if !pending_events.is_empty() && last_flush.elapsed() >= batch_timeout {
                            Self::flush_batch(&mut pending_events, &callback, &batch_id);
                            last_flush = Instant::now();
                        }
                    }
                }
            }

            if !pending_events.is_empty() {
                Self::flush_batch(&mut pending_events, &callback, &batch_id);
            }

            info!("Batch processor stopped");
        });
    }

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

    pub fn get_state(&self) -> WatcherState {
        *self.state.lock().unwrap()
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

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
