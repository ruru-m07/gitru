//! Local collaboration data is independent of Tauri and local Git contexts.
#[cfg(test)]
extern crate self as collaboration;
pub mod checks;
pub mod command_recovery;
pub mod commands;
pub mod contextual_capabilities;
pub mod credentials;
pub(crate) mod delivery;
pub mod demand;
pub mod detail;
pub mod diagnostics;
pub mod domain;
pub mod effective;
pub mod error;
pub mod github_cli;
pub mod local_links;
pub mod notification_subjects;
pub mod participants;
pub mod providers;
pub mod pull_commits;
#[cfg(test)]
#[path = "../tests/support/pull_file_fixture.rs"]
pub(crate) mod pull_file_fixture;
pub mod pull_files;
pub mod recovery;
pub mod resource_metadata;
pub mod reviews;
pub mod runtime;
pub mod storage;
pub mod tasks;
#[cfg(feature = "test-harness")]
pub mod test_harness;
pub mod text_edits;

pub use checks::*;
pub use command_recovery::*;
pub use commands::*;
pub use contextual_capabilities::*;
pub use demand::*;
pub use detail::*;
pub use diagnostics::*;
pub use domain::*;
pub use effective::{IntentField, PendingCommandIntent, PendingItemIntent};
pub use error::{CollaborationError, ErrorCode};
pub use local_links::*;
pub use notification_subjects::*;
pub use participants::*;
pub use pull_commits::*;
pub use pull_files::*;
pub use resource_metadata::*;
pub use reviews::*;
pub use runtime::CollaborationRuntime;
pub use storage::Store;
pub use storage::notification_subjects::{NotificationDiscoveryIntent, NotificationDiscoveryLease};
pub use tasks::*;
pub use text_edits::*;
