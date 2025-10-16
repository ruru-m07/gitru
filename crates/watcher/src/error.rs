use thiserror::Error;

#[derive(Error, Debug)]
pub enum WatcherError {
    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Watcher already running")]
    AlreadyRunning,

    #[error("Watcher not initialized")]
    NotInitialized,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Gitignore parse error: {0}")]
    GitignoreError(String),

    #[error("File system error: {0}")]
    FsError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Notify error: {0}")]
    NotifyError(String),

    #[error("Channel send error")]
    ChannelError,

    #[error("Backpressure exceeded: queue size {0}")]
    BackpressureExceeded(usize),

    #[error("Internal error: {0}")]
    Internal(String),
}

// Convert notify errors
impl From<notify::Error> for WatcherError {
    fn from(err: notify::Error) -> Self {
        use notify::ErrorKind;
        match err.kind {
            ErrorKind::PathNotFound => WatcherError::PathNotFound(err.to_string()),
            ErrorKind::Io(io_err) => {
                if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                    WatcherError::PermissionDenied(io_err.to_string())
                } else {
                    WatcherError::IoError(io_err)
                }
            }
            ErrorKind::InvalidConfig(_) => WatcherError::ConfigError(err.to_string()),
            _ => WatcherError::NotifyError(err.to_string()),
        }
    }
}

// Convert ignore errors
impl From<ignore::Error> for WatcherError {
    fn from(err: ignore::Error) -> Self {
        if let Some(io_err) = err.io_error() {
            WatcherError::IoError(std::io::Error::new(io_err.kind(), io_err.to_string()))
        } else {
            WatcherError::GitignoreError(err.to_string())
        }
    }
}

pub type WatcherResult<T> = Result<T, WatcherError>;
