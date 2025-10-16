use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the file watcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    /// Debounce window in milliseconds (default: 100)
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,

    /// Maximum events per batch (default: 50)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Maximum batch wait time in milliseconds (default: 150)
    #[serde(default = "default_batch_timeout_ms")]
    pub batch_timeout_ms: u64,

    /// Maximum pending events before backpressure kicks in (default: 10,000)
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,

    /// Whether to follow symlinks (default: false)
    #[serde(default)]
    pub follow_symlinks: bool,

    /// Additional ignore patterns beyond .gitignore
    #[serde(default)]
    pub extra_ignores: Vec<String>,

    /// Enable statistics emission (default: false)
    #[serde(default)]
    pub emit_stats: bool,

    /// Statistics emission interval in seconds (default: 10)
    #[serde(default = "default_stats_interval")]
    pub stats_interval_secs: u64,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce_ms(),
            batch_size: default_batch_size(),
            batch_timeout_ms: default_batch_timeout_ms(),
            max_queue_size: default_max_queue_size(),
            follow_symlinks: false,
            extra_ignores: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".DS_Store".to_string(),
                "*.swp".to_string(),
                "*.swo".to_string(),
                "*~".to_string(),
            ],
            emit_stats: false,
            stats_interval_secs: default_stats_interval(),
        }
    }
}

impl WatcherConfig {
    /// Get debounce duration
    pub fn debounce_duration(&self) -> Duration {
        Duration::from_millis(self.debounce_ms)
    }

    /// Get batch timeout duration
    pub fn batch_timeout(&self) -> Duration {
        Duration::from_millis(self.batch_timeout_ms)
    }

    /// Get stats interval duration
    pub fn stats_interval(&self) -> Duration {
        Duration::from_secs(self.stats_interval_secs)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.debounce_ms == 0 {
            return Err("debounce_ms must be greater than 0".to_string());
        }
        if self.batch_size == 0 {
            return Err("batch_size must be greater than 0".to_string());
        }
        if self.batch_timeout_ms == 0 {
            return Err("batch_timeout_ms must be greater than 0".to_string());
        }
        if self.max_queue_size == 0 {
            return Err("max_queue_size must be greater than 0".to_string());
        }
        Ok(())
    }
}

// Default value functions for serde
fn default_debounce_ms() -> u64 {
    100
}

fn default_batch_size() -> usize {
    50
}

fn default_batch_timeout_ms() -> u64 {
    150
}

fn default_max_queue_size() -> usize {
    10_000
}

fn default_stats_interval() -> u64 {
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_ms, 100);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.batch_timeout_ms, 150);
        assert_eq!(config.max_queue_size, 10_000);
        assert!(!config.follow_symlinks);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_config() {
        let mut config = WatcherConfig::default();
        config.debounce_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_durations() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_duration(), Duration::from_millis(100));
        assert_eq!(config.batch_timeout(), Duration::from_millis(150));
    }
}
