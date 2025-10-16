# Watcher Crate

A high-performance, .gitignore-aware file watching library for Rust, designed for integration with Tauri applications.

## Features

- 🚀 **Fast & Efficient**: Built on `notify` with debouncing and batching
- 🎯 **Gitignore Support**: Respects `.gitignore` files using the `ignore` crate
- 📦 **Batched Events**: Reduces noise with intelligent event coalescing
- 🔄 **Real-time**: Low-latency event delivery (< 300ms)
- 🛡️ **Cross-platform**: Works on Linux, macOS, and Windows
- ⚡ **Async-ready**: Built with Tokio for async operations
- 🔌 **Tauri Integration**: Optional feature for Tauri commands

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
watcher = { path = "../../crates/watcher" }

# Or with Tauri commands feature
watcher = { path = "../../crates/watcher", features = ["tauri-commands"] }
```

## Quick Start

### Basic Usage

```rust
use watcher::{RepoWatcher, WatcherConfig};

#[tokio::main]
async fn main() {
    // Create configuration
    let config = WatcherConfig {
        debounce_ms: 100,
        batch_size: 50,
        ..Default::default()
    };

    // Create watcher
    let watcher = RepoWatcher::new("/path/to/repo", config).unwrap();

    // Set event callback
    watcher.set_callback(|batch| {
        println!("Received {} events", batch.events.len());
        for event in batch.events {
            println!("  {} - {}", event.event_type, event.path);
        }
    });

    // Start watching
    watcher.start().unwrap();

    // Keep running
    tokio::signal::ctrl_c().await.unwrap();

    // Stop watching
    watcher.stop().unwrap();
}
```

### With Tauri

```rust
use tauri::Builder;
use watcher::{start_watching, stop_watching, get_watcher_state, WatcherState};

fn main() {
    Builder::default()
        .manage(WatcherState::new())
        .invoke_handler(tauri::generate_handler![
            start_watching,
            stop_watching,
            get_watcher_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Configuration

```rust
pub struct WatcherConfig {
    /// Debounce window in milliseconds (default: 100)
    pub debounce_ms: u64,

    /// Maximum events per batch (default: 50)
    pub batch_size: usize,

    /// Maximum batch wait time in milliseconds (default: 150)
    pub batch_timeout_ms: u64,

    /// Maximum pending events before backpressure (default: 10,000)
    pub max_queue_size: usize,

    /// Whether to follow symlinks (default: false)
    pub follow_symlinks: bool,

    /// Additional ignore patterns beyond .gitignore
    pub extra_ignores: Vec<String>,

    /// Enable statistics emission (default: false)
    pub emit_stats: bool,

    /// Statistics emission interval in seconds (default: 10)
    pub stats_interval_secs: u64,
}
```

## Event Types

```rust
pub enum FileEventType {
    Created,   // File was created
    Modified,  // File was modified
    Deleted,   // File was deleted
    Renamed,   // File was renamed
}

pub struct FileEvent {
    pub event_type: FileEventType,
    pub path: String,           // Relative to repo root
    pub absolute_path: String,  // Full filesystem path
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<FileMetadata>,
}

pub struct BatchUpdate {
    pub events: Vec<FileEvent>,
    pub batch_id: u64,
    pub timestamp: DateTime<Utc>,
    pub total_events: usize,
}
```

## Gitignore Filtering

The watcher automatically respects `.gitignore` files in your repository:

```bash
# .gitignore
*.log
target/
node_modules/
```

Files matching these patterns will **not** trigger events.

### Custom Ignore Patterns

```rust
let config = WatcherConfig {
    extra_ignores: vec![
        "*.tmp".to_string(),
        "cache/".to_string(),
    ],
    ..Default::default()
};
```

## Performance

- **Latency**: < 300ms from file change to event emission
- **Throughput**: Handles 1000+ events/second
- **Memory**: < 100MB for typical repositories
- **CPU**: < 5% during idle

### Optimization Tips

1. **Adjust debounce window**: Higher values reduce events but increase latency
2. **Increase batch size**: Fewer IPC calls, but larger messages
3. **Use extra_ignores**: Skip known unimportant files early

## Error Handling

```rust
use watcher::WatcherError;

match watcher.start() {
    Ok(_) => println!("Started successfully"),
    Err(WatcherError::PathNotFound(path)) => {
        eprintln!("Path not found: {}", path);
    }
    Err(WatcherError::PermissionDenied(path)) => {
        eprintln!("Permission denied: {}", path);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Testing

```bash
# Run all tests
cargo test

# Run specific test module
cargo test filter

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo test --release benchmark -- --nocapture
```

## Examples

See the `examples/` directory for more usage examples:

- `basic.rs` - Simple file watching
- `gitignore.rs` - Gitignore filtering
- `performance.rs` - Performance benchmarks

## Architecture

```
RepoWatcher
    ├── GitignoreFilter (respects .gitignore)
    ├── notify::Watcher (file system events)
    ├── Debouncer (reduces noise)
    └── BatchProcessor (collects events)
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## License

See the main project LICENSE file.

## Credits

Built with:

- [`notify`](https://github.com/notify-rs/notify) - Cross-platform file system notifications
- [`ignore`](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore) - Gitignore parsing from ripgrep
- [`tokio`](https://tokio.rs/) - Async runtime

## See Also

- [File Watcher Design Doc](../../../docs/file-watcher-design.md)
- [IPC Contract](../../../docs/ipc-contract.md)
- [Testing Plan](../../../docs/testing-plan.md)
