use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size: Option<u64>,

    pub is_directory: bool,

    pub old_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvent {
    pub event_type: FileEventType,

    pub path: String,

    pub absolute_path: String,

    pub timestamp: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FileMetadata>,
}

impl FileEvent {
    pub fn new(event_type: FileEventType, path: String, absolute_path: String) -> Self {
        Self {
            event_type,
            path,
            absolute_path,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: FileMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn metadata_from_path(path: &Path) -> Option<FileMetadata> {
        std::fs::metadata(path).ok().map(|meta| FileMetadata {
            size: if meta.is_file() {
                Some(meta.len())
            } else {
                None
            },
            is_directory: meta.is_dir(),
            old_path: None,
        })
    }

    pub fn with_old_path(mut self, old_path: String) -> Self {
        if let Some(ref mut metadata) = self.metadata {
            metadata.old_path = Some(old_path);
        } else {
            self.metadata = Some(FileMetadata {
                size: None,
                is_directory: false,
                old_path: Some(old_path),
            });
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdate {
    pub events: Vec<FileEvent>,

    pub batch_id: u64,

    pub timestamp: DateTime<Utc>,

    pub total_events: usize,
}

impl BatchUpdate {
    pub fn new(events: Vec<FileEvent>, batch_id: u64) -> Self {
        let total_events = events.len();
        Self {
            events,
            batch_id,
            timestamp: Utc::now(),
            total_events,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatcherState {
    Initializing,
    Active,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatcherErrorType {
    PermissionDenied,
    PathNotFound,
    WatcherCrashed,
    GitignoreParseError,
    BackpressureExceeded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherStats {
    pub events_per_second: f64,

    pub events_queued: usize,

    pub events_dropped: u64,

    pub memory_usage_mb: f64,

    pub uptime_seconds: u64,

    pub timestamp: DateTime<Utc>,
}

impl Default for WatcherStats {
    fn default() -> Self {
        Self {
            events_per_second: 0.0,
            events_queued: 0,
            events_dropped: 0,
            memory_usage_mb: 0.0,
            uptime_seconds: 0,
            timestamp: Utc::now(),
        }
    }
}
