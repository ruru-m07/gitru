pub mod commands;
pub mod config;
pub mod error;
pub mod events;
pub mod filter;
pub mod watcher;

pub use commands::{
    get_watcher_state, rescan_repository, start_watching, stop_watching, WatcherState,
};
pub use config::WatcherConfig;
pub use error::{WatcherError, WatcherResult};
pub use events::{
    BatchUpdate, FileEvent, FileEventType, FileMetadata, WatcherState as WatcherLifecycleState,
};
pub use filter::GitignoreFilter;
pub use watcher::RepoWatcher;

// Re-export commonly used types
pub use chrono::{DateTime, Utc};
pub use uuid::Uuid;
