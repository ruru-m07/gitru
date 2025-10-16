// File watcher event types (matching Rust backend)

export enum FileEventType {
  Created = 'created',
  Modified = 'modified',
  Deleted = 'deleted',
  Renamed = 'renamed',
}

export interface FileMetadata {
  size?: number;
  is_directory: boolean;
  old_path?: string;
}

export interface FileEvent {
  event_type: FileEventType;
  path: string;
  absolute_path: string;
  timestamp: string;
  metadata?: FileMetadata;
}

export interface BatchUpdate {
  events: FileEvent[];
  batch_id: number;
  timestamp: string;
  total_events: number;
}

export enum WatcherState {
  Initializing = 'initializing',
  Active = 'active',
  Paused = 'paused',
  Stopped = 'stopped',
  Error = 'error',
}

export enum WatcherErrorType {
  PermissionDenied = 'permission_denied',
  PathNotFound = 'path_not_found',
  WatcherCrashed = 'watcher_crashed',
  GitignoreParseError = 'gitignore_parse_error',
  BackpressureExceeded = 'backpressure_exceeded',
  Unknown = 'unknown',
}

export interface WatcherError {
  watcher_id: string;
  error_type: WatcherErrorType;
  message: string;
  path?: string;
  recoverable: boolean;
  timestamp: string;
}

export interface WatcherStateChanged {
  watcher_id: string;
  old_state: WatcherState;
  new_state: WatcherState;
  reason?: string;
  timestamp: string;
}

export interface WatcherStats {
  events_per_second: number;
  events_queued: number;
  events_dropped: number;
  memory_usage_mb: number;
  uptime_seconds: number;
  timestamp: string;
}

export interface WatcherConfig {
  debounce_ms?: number;
  batch_size?: number;
  batch_timeout_ms?: number;
  follow_symlinks?: boolean;
  extra_ignores?: string[];
  emit_stats?: boolean;
  stats_interval_secs?: number;
}

export interface StartWatchingResponse {
  success: boolean;
  watcher_id: string;
  error?: string;
}

export interface StopWatchingResponse {
  success: boolean;
  error?: string;
}
