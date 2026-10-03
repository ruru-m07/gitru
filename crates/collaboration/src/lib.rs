//! Local collaboration data is independent of Tauri and local Git contexts.
pub mod credentials;
pub mod domain;
pub mod error;
pub mod github_cli;
pub mod providers;
pub mod runtime;
pub mod storage;

pub use domain::*;
pub use error::{CollaborationError, ErrorCode};
pub use runtime::CollaborationRuntime;
pub use storage::Store;
