use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Type of file system event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// Metadata about a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes (None if unavailable or directory)
    pub size: Option<u64>,

    /// Whether this is a directory
    pub is_directory: bool,

    /// Old path for rename events
    pub old_path: Option<String>,
}

/// A single file system event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvent {
    /// Type of event
    pub event_type: FileEventType,

    /// Path relative to repository root
    pub path: String,

    /// Absolute filesystem path
    pub absolute_path: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FileMetadata>,
}

impl FileEvent {
    /// Create a new file event
    pub fn new(event_type: FileEventType, path: String, absolute_path: String) -> Self {
        Self {
            event_type,
            path,
            absolute_path,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create with metadata
    pub fn with_metadata(mut self, metadata: FileMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Create metadata from a path
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

    /// Set old path for rename events
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

/// Batch of file events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdate {
    /// Events in this batch
    pub events: Vec<FileEvent>,

    /// Monotonically increasing batch ID
    pub batch_id: u64,

    /// Timestamp when batch was created
    pub timestamp: DateTime<Utc>,

    /// Total number of events in batch
    pub total_events: usize,
}

impl BatchUpdate {
    /// Create a new batch
    pub fn new(events: Vec<FileEvent>, batch_id: u64) -> Self {
        let total_events = events.len();
        Self {
            events,
            batch_id,
            timestamp: Utc::now(),
            total_events,
        }
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get batch size
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

/// Watcher lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatcherState {
    Initializing,
    Active,
    Paused,
    Stopped,
    Error,
}

/// Watcher error types for IPC
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

/// Watcher statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherStats {
    /// Events processed per second (average over last interval)
    pub events_per_second: f64,

    /// Current queue depth
    pub events_queued: usize,

    /// Events dropped due to backpressure
    pub events_dropped: u64,

    /// Memory usage in MB (approximate)
    pub memory_usage_mb: f64,

    /// Uptime in seconds
    pub uptime_seconds: u64,

    /// Timestamp
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_event_creation() {
        let event = FileEvent::new(
            FileEventType::Created,
            "src/main.rs".to_string(),
            "/repo/src/main.rs".to_string(),
        );

        assert_eq!(event.event_type, FileEventType::Created);
        assert_eq!(event.path, "src/main.rs");
        assert!(event.metadata.is_none());
    }

    #[test]
    fn test_batch_update() {
        let events = vec![
            FileEvent::new(
                FileEventType::Created,
                "a.txt".to_string(),
                "/repo/a.txt".to_string(),
            ),
            FileEvent::new(
                FileEventType::Modified,
                "b.txt".to_string(),
                "/repo/b.txt".to_string(),
            ),
        ];

        let batch = BatchUpdate::new(events, 1);
        assert_eq!(batch.batch_id, 1);
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_event_serialization() {
        let event = FileEvent::new(
            FileEventType::Modified,
            "test.txt".to_string(),
            "/repo/test.txt".to_string(),
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: FileEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.path, deserialized.path);
        assert_eq!(event.event_type, deserialized.event_type);
    }
}
