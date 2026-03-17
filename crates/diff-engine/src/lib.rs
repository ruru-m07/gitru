use std::sync::Arc;

use syntect::parsing::SyntaxSet;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    cache::SyntaxCache,
    language_registry::LanguageRegistry,
    models::{DiffJob, DiffRequest},
    queue::JobQueue,
    theme_manager::ThemeManager,
};

pub mod cache;
pub mod highlighter;
pub mod hunk;
pub mod language_registry;
pub mod models;
pub mod queue;
pub mod reader;
pub mod semantic;
pub mod theme_manager;
pub mod worker;

/// Global managed state — holds the live engine behind an RwLock so
/// Tauri commands can share it safely across threads.
pub struct DiffEngineState {
    pub services: RwLock<Option<Arc<DiffEngine>>>,
}

/// Long-lived context shared by all workers. Expanding this struct is
/// the right place to add shared resources in future phases (file
/// cache, watcher, LRU diff cache, etc.).
pub struct DiffEngineContext {
    pub repo_path: String,
    pub syntax_set: Arc<SyntaxSet>,
    pub theme_manager: ThemeManager,
    pub syntax_cache: SyntaxCache,
    pub language_registry: Arc<LanguageRegistry>,
}

impl DiffEngineContext {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        let syntax_set = Arc::new(SyntaxSet::load_defaults_newlines());
        let theme_manager = ThemeManager::new(theme_manager::ThemeName::Base16OceanDark)?;
        let syntax_cache = SyntaxCache::new();
        let language_registry = Arc::new(LanguageRegistry::new());

        Ok(Self {
            repo_path: repo_path.to_string(),
            syntax_set,
            theme_manager,
            syntax_cache,
            language_registry,
        })
    }

    /// Change the theme for this engine context
    pub fn set_theme(&mut self, theme_name: theme_manager::ThemeName) -> Result<(), String> {
        self.theme_manager.set_theme(theme_name)
    }

    /// Get information about the current context state
    pub fn cache_stats(&self) -> (usize, usize) {
        self.syntax_cache.stats()
    }
}

pub struct DiffEngine {
    pub ctx: Arc<DiffEngineContext>,
    queue: Arc<JobQueue>,
}

impl DiffEngine {
    /// Create the engine and immediately spawn the worker pool.
    /// Worker count = max(1, num_cpus / 2) — leaves headroom for the
    /// main thread and git-service workers.
    pub fn new(repo_path: &str, app: tauri::AppHandle) -> Result<Self, String> {
        let ctx = Arc::new(DiffEngineContext::new(repo_path)?);
        let worker_count = (num_cpus::get() / 2).max(1);
        let (queue, receiver) = JobQueue::new();
        let queue = Arc::new(queue);
        let receiver = Arc::new(receiver);

        for id in 0..worker_count {
            let rx = receiver.clone();
            let ctx_clone = ctx.clone();
            let app_clone = app.clone();
            let queue_clone = queue.clone();
            tauri::async_runtime::spawn(async move {
                worker::run_worker(id, rx, ctx_clone, app_clone, queue_clone).await;
            });
        }

        log::info!(
            "diff-engine started: {} worker(s) for repo {}",
            worker_count,
            repo_path
        );

        Ok(Self { ctx, queue })
    }

    /// Enqueue a diff job and return its unique ID. The caller should
    /// listen to `diff_event` on the Tauri event bus and filter by
    /// `jobId` to receive the result.
    pub async fn enqueue_job(&self, request: DiffRequest) -> Result<String, String> {
        let job = DiffJob {
            id: Uuid::new_v4(),
            request,
        };
        let id = job.id.to_string();
        self.queue.enqueue(job).await?;
        Ok(id)
    }

    /// Cancel the current running task (if any)
    pub async fn cancel_current_job(&self) {
        self.queue.cancel_current().await;
    }

    /// Clear the queue (prevents new jobs from being processed)
    pub async fn clear_queue(&self) {
        self.queue.clear().await;
    }

    /// Get the ID of the currently running job (if any)
    pub async fn current_job_id(&self) -> Option<String> {
        self.queue.current_job().await.map(|id| id.to_string())
    }
}
