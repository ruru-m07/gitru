//! Local collaboration data is independent of Tauri and local Git contexts.
#[cfg(test)]
extern crate self as collaboration;
pub mod contextual_capabilities;
pub mod credentials;
pub mod demand;
pub mod detail;
pub mod domain;
pub mod error;
pub mod github_cli;
pub mod local_links;
pub mod notification_subjects;
pub mod providers;
pub mod resource_metadata;
pub mod runtime;
pub mod storage;

pub use contextual_capabilities::*;
pub use demand::*;
pub use detail::*;
pub use domain::*;
pub use error::{CollaborationError, ErrorCode};
pub use local_links::*;
pub use notification_subjects::*;
pub use resource_metadata::*;
pub use runtime::CollaborationRuntime;
pub use storage::Store;
pub use storage::notification_subjects::{NotificationDiscoveryIntent, NotificationDiscoveryLease};
