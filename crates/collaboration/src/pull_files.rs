//! Provider-independent pull-request file and selected-diff facts.
//!
//! Remote paths are bounded labels. Nothing in this module turns one into a
//! local path or accepts a renderer-selected provider URL as fetch authority.

use crate::{
    AccountState, CollaborationError, ProviderKind, RemoteAccount, RemoteItem, RemoteItemKind,
    RemoteRepository, is_canonical_commit_oid,
};
use chrono::{DateTime, SecondsFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub mod diff;

pub const MAX_PULL_FILES: u32 = 3_000;
pub const MAX_PULL_FILE_PROVIDER_PAGES: u32 = 30;
pub const MAX_PULL_FILES_PER_PROVIDER_PAGE: usize = 100;
pub const MAX_PULL_FILES_PER_LOCAL_PAGE: u32 = 100;
pub const MAX_PULL_FILE_LOCAL_PAGE_BYTES: usize = 1_048_576;
pub const MAX_PULL_FILE_PATH_BYTES: usize = 4_096;
pub const MAX_PULL_FILE_NATIVE_STATE_BYTES: usize = 128;
pub const MAX_PULL_FILE_PROVIDER_CURSOR_BYTES: usize = 8_192;
pub const MAX_PULL_FILE_LOCAL_CURSOR_BYTES: usize = 4_096;
pub const MAX_PULL_FILE_LOCAL_CURSOR_DEPTH: u32 = 100;
pub const MAX_PULL_FILE_KEY_BYTES: usize = 256;
pub const TARGET_PULL_FILE_TEXT_BYTES: usize = 2 * 1_048_576;
pub const MAX_PULL_FILE_TEXT_BYTES: usize = 4 * 1_048_576;
pub const MAX_PULL_FILE_TEXT_LINES: usize = 50_000;
pub const MAX_PULL_FILE_LINE_BYTES: usize = 256 * 1_024;
pub const MAX_PULL_FILE_WHOLE_DIFF_BYTES: usize = 16 * 1_048_576;

const MAX_IDENTIFIER_BYTES: usize = 1_024;
const MAX_CONTENT_TYPE_BYTES: usize = 256;
const MAX_BLOB_REFERENCE_BYTES: usize = 1_024;
const MAX_TIMESTAMP_BYTES: usize = 128;

type Result<T> = std::result::Result<T, CollaborationError>;

fn invalid_pull_file() -> CollaborationError {
    CollaborationError::invalid("Invalid or unbounded pull file data")
}

fn validate_label(value: &str, maximum: usize) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        Err(invalid_pull_file())
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str) -> Result<()> {
    validate_label(value, MAX_IDENTIFIER_BYTES)
}

fn validate_revision(value: &str) -> Result<()> {
    validate_positive_decimal(value).map(|_| ())
}

fn validate_generation(value: &str) -> Result<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| invalid_pull_file())?;
    if parsed.hyphenated().to_string() != value {
        return Err(invalid_pull_file());
    }
    Ok(())
}

fn validate_canonical_utc_timestamp(value: &str) -> Result<()> {
    validate_label(value, MAX_TIMESTAMP_BYTES)?;
    let parsed = DateTime::parse_from_rfc3339(value).map_err(|_| invalid_pull_file())?;
    if parsed.offset().local_minus_utc() != 0
        || parsed.to_rfc3339_opts(SecondsFormat::AutoSi, true) != value
    {
        return Err(invalid_pull_file());
    }
    Ok(())
}

fn validate_decimal(value: &str) -> Result<u64> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_pull_file());
    }
    let parsed = value.parse::<u64>().map_err(|_| invalid_pull_file())?;
    if parsed.to_string() != value {
        return Err(invalid_pull_file());
    }
    Ok(parsed)
}

fn validate_positive_decimal(value: &str) -> Result<u64> {
    let parsed = validate_decimal(value)?;
    if parsed == 0 || parsed > i64::MAX as u64 {
        Err(invalid_pull_file())
    } else {
        Ok(parsed)
    }
}

fn validate_file_key(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_PULL_FILE_KEY_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        Err(invalid_pull_file())
    } else {
        Ok(())
    }
}

fn validate_blob_reference(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_BLOB_REFERENCE_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        Err(invalid_pull_file())
    } else {
        Ok(())
    }
}

/// Provider-observed object IDs use the existing canonical SHA-1/SHA-256 rule.
pub fn is_canonical_pull_file_oid(value: &str) -> bool {
    is_canonical_commit_oid(value)
}

/// A remote repository path is validated as a label, without filesystem
/// normalization. This preserves provider identity while excluding values that
/// acquire absolute or parent-traversal meaning on common local filesystems.
pub fn is_valid_pull_file_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_PULL_FILE_PATH_BYTES
        || value.chars().any(char::is_control)
        || value.starts_with(['/', '\\'])
    {
        return false;
    }
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !value
        .split(['/', '\\'])
        .any(|component| component.is_empty() || component == "." || component == "..")
}

/// Exact Body-derived authority for one file generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PullFileContext {
    /// Provider-observed target tip, not necessarily the diff's merge base.
    pub base_oid: String,
    pub head_oid: String,
    /// Unknown never means `base_oid`. Current source strategies compare the
    /// merge base to the head, rather than the two branch tips directly.
    pub merge_base_oid: Option<String>,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
    pub body_metadata_facet_revision: String,
}

impl PullFileContext {
    pub fn validate(&self) -> Result<()> {
        if !is_canonical_pull_file_oid(&self.base_oid)
            || !is_canonical_pull_file_oid(&self.head_oid)
            || self
                .merge_base_oid
                .as_ref()
                .is_some_and(|oid| !is_canonical_pull_file_oid(oid))
        {
            return Err(invalid_pull_file());
        }
        validate_identifier(&self.base_repository_provider_id)?;
        validate_identifier(&self.source_repository_provider_id)?;
        validate_revision(&self.body_metadata_facet_revision)
    }

    /// Includes the Body metadata revision as well as every remote range fact.
    pub fn is_exact(&self, other: &Self) -> bool {
        self == other
    }

    /// Terminal publication checks every freshly observed comparison fact;
    /// the Body revision remains bound by this context itself.
    pub fn matches_range_validation(&self, validation: &PullFileRangeValidation) -> bool {
        self.validate().is_ok()
            && validation.validate().is_ok()
            && self.base_oid == validation.base_oid
            && self.head_oid == validation.head_oid
            && self.merge_base_oid == validation.merge_base_oid
            && self.base_repository_provider_id == validation.base_repository_provider_id
            && self.source_repository_provider_id == validation.source_repository_provider_id
    }
}

/// Fresh parent facts checked before a terminal generation is published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullFileRangeValidation {
    pub base_oid: String,
    pub head_oid: String,
    pub merge_base_oid: Option<String>,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
}

impl PullFileRangeValidation {
    pub fn validate(&self) -> Result<()> {
        if !is_canonical_pull_file_oid(&self.base_oid)
            || !is_canonical_pull_file_oid(&self.head_oid)
            || self
                .merge_base_oid
                .as_ref()
                .is_some_and(|oid| !is_canonical_pull_file_oid(oid))
        {
            return Err(invalid_pull_file());
        }
        validate_identifier(&self.base_repository_provider_id)?;
        validate_identifier(&self.source_repository_provider_id)
    }
}

/// Path identity never collapses a rename or copy to its destination.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PullFileIdentity {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
}

impl PullFileIdentity {
    pub fn validate(&self) -> Result<()> {
        if self.old_path.is_none() && self.new_path.is_none() {
            return Err(invalid_pull_file());
        }
        if self
            .old_path
            .as_deref()
            .is_some_and(|path| !is_valid_pull_file_path(path))
            || self
                .new_path
                .as_deref()
                .is_some_and(|path| !is_valid_pull_file_path(path))
        {
            return Err(invalid_pull_file());
        }
        Ok(())
    }

    fn validate_for(&self, change_kind: PullFileChangeKind) -> Result<()> {
        self.validate()?;
        let valid = match change_kind {
            PullFileChangeKind::Added => self.old_path.is_none() && self.new_path.is_some(),
            PullFileChangeKind::Deleted => self.old_path.is_some() && self.new_path.is_none(),
            PullFileChangeKind::Renamed | PullFileChangeKind::Copied => {
                self.old_path.is_some() && self.new_path.is_some()
            }
            // Some providers omit one side for in-place or unclassified
            // changes. These kinds preserve the observed pair without
            // inferring the missing path.
            PullFileChangeKind::Modified
            | PullFileChangeKind::TypeChanged
            | PullFileChangeKind::Unknown => true,
        };
        if valid {
            Ok(())
        } else {
            Err(invalid_pull_file())
        }
    }
}

/// The same helper can be applied to one page or to storage's accumulated
/// generation identities, so supplemental provider IDs never mask a duplicate
/// old/new path tuple across pages.
pub fn pull_file_identities_are_unique<'a>(
    identities: impl IntoIterator<Item = &'a PullFileIdentity>,
) -> bool {
    let mut unique = HashSet::new();
    identities
        .into_iter()
        .all(|identity| unique.insert(identity))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    /// A provider-observed type transition. One-sided paths remain explicit.
    TypeChanged,
    /// Native state was not recognized. Path presence is not inferred.
    Unknown,
}

/// Trustworthy numeric provider facts remain decimal strings at boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum PullFileCount {
    Unknown,
    Known(String),
}

impl PullFileCount {
    pub fn known(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_decimal(&value)?;
        Ok(Self::Known(value))
    }

    pub fn validate(&self) -> Result<()> {
        if let Self::Known(value) = self {
            validate_decimal(value)?;
        }
        Ok(())
    }
}

/// Unknown is distinct from a provider-observed `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum PullFileFlag {
    Unknown,
    Known(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileDiffHint {
    Unknown,
    Candidate,
    Omitted,
    Oversized,
    Unsupported,
    Unavailable,
}

/// Adapter-normalized row before the engine assigns a stable key and position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPullFile {
    pub identity: PullFileIdentity,
    /// Supplemental adapter-owned evidence; never part of path-pair identity.
    pub provider_file_id: Option<String>,
    pub change_kind: PullFileChangeKind,
    pub provider_change_kind: String,
    pub additions: PullFileCount,
    pub deletions: PullFileCount,
    pub total_changes: PullFileCount,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub mode_changed: PullFileFlag,
    pub binary: PullFileFlag,
    pub generated: PullFileFlag,
    /// Explicit endpoint omission signals. Unknown is retained for providers
    /// which do not report these facts; neither flag proves binary content.
    pub provider_collapsed: PullFileFlag,
    pub provider_too_large: PullFileFlag,
    pub diff_hint: PullFileDiffHint,
}

impl ProviderPullFile {
    pub fn validate(&self) -> Result<()> {
        self.identity.validate_for(self.change_kind)?;
        if let Some(provider_file_id) = &self.provider_file_id {
            validate_identifier(provider_file_id)?;
        }
        validate_label(&self.provider_change_kind, MAX_PULL_FILE_NATIVE_STATE_BYTES)?;
        self.additions.validate()?;
        self.deletions.validate()?;
        for mode in [&self.old_mode, &self.new_mode].into_iter().flatten() {
            if mode.len() != 6 || !mode.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
                return Err(invalid_pull_file());
            }
        }
        self.total_changes.validate()
    }
}

/// Published file summary bound to one exact context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFile {
    /// Opaque engine-owned key. It is not a path or provider URL.
    pub file_key: String,
    pub context: PullFileContext,
    pub provider_position: u32,
    pub file: ProviderPullFile,
}

impl PullFile {
    pub fn validate(&self) -> Result<()> {
        validate_file_key(&self.file_key)?;
        self.context.validate()?;
        if self.provider_position >= MAX_PULL_FILES {
            return Err(invalid_pull_file());
        }
        self.file.validate()
    }

    /// Stable storage ordering after the provider's declared position.
    pub fn deterministic_order_key(&self) -> (u32, Option<&str>, Option<&str>, Option<&str>) {
        (
            self.provider_position,
            self.file.identity.old_path.as_deref(),
            self.file.identity.new_path.as_deref(),
            self.file.provider_file_id.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileCapReason {
    ProviderFileLimit,
    ProviderPageLimit,
    ProviderOverflow,
    LocalFileLimit,
    LocalPageLimit,
    LocalByteLimit,
}

impl PullFileCapReason {
    const fn provenance(self) -> PullFileCapProvenance {
        match self {
            Self::ProviderFileLimit | Self::ProviderPageLimit | Self::ProviderOverflow => {
                PullFileCapProvenance::Provider
            }
            Self::LocalFileLimit | Self::LocalPageLimit | Self::LocalByteLimit => {
                PullFileCapProvenance::Local
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileCapProvenance {
    Provider,
    Local,
}

/// Exact terminal cap evidence. `remote_has_more` may itself be unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileCapEvidence {
    pub provenance: PullFileCapProvenance,
    pub reason: PullFileCapReason,
    pub remote_has_more: PullFileFlag,
}

impl PullFileCapEvidence {
    pub const fn is_valid(self) -> bool {
        matches!(
            (self.provenance, self.reason.provenance()),
            (
                PullFileCapProvenance::Provider,
                PullFileCapProvenance::Provider
            ) | (PullFileCapProvenance::Local, PullFileCapProvenance::Local)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileCompletenessState {
    Missing,
    Syncing,
    Complete,
    Capped,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileCompleteness {
    pub state: PullFileCompletenessState,
    pub cap: Option<PullFileCapEvidence>,
}

impl PullFileCompleteness {
    pub const fn missing() -> Self {
        Self {
            state: PullFileCompletenessState::Missing,
            cap: None,
        }
    }

    pub const fn syncing() -> Self {
        Self {
            state: PullFileCompletenessState::Syncing,
            cap: None,
        }
    }

    pub const fn complete() -> Self {
        Self {
            state: PullFileCompletenessState::Complete,
            cap: None,
        }
    }

    pub const fn partial() -> Self {
        Self {
            state: PullFileCompletenessState::Partial,
            cap: None,
        }
    }

    pub const fn capped(cap: PullFileCapEvidence) -> Self {
        Self {
            state: PullFileCompletenessState::Capped,
            cap: Some(cap),
        }
    }

    pub const fn is_valid(self) -> bool {
        match (self.state, self.cap) {
            (PullFileCompletenessState::Capped, Some(cap)) => cap.is_valid(),
            (
                PullFileCompletenessState::Missing
                | PullFileCompletenessState::Syncing
                | PullFileCompletenessState::Complete
                | PullFileCompletenessState::Partial,
                None,
            ) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileProvenance {
    Provider,
    LocalExactRange,
}

/// Closed transport/derivation strategies. Adding a provider path requires a
/// reviewed native variant; serialized observations cannot smuggle URLs or
/// provider response labels through this field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileSourceStrategy {
    GithubPullFiles,
    GithubPullDiff,
    GitlabMergeRequestDiffs,
    GitlabRawDiffs,
    BitbucketCloudDiffstat,
    BitbucketCloudDiff,
    LocalExactRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullFileComparisonSemantics {
    MergeBaseToHead,
}

impl PullFileSourceStrategy {
    /// Current provider endpoints all implement pull-request (three-dot)
    /// comparisons. LocalExactRange resolves a unique merge base first.
    pub const fn comparison_semantics(self) -> PullFileComparisonSemantics {
        PullFileComparisonSemantics::MergeBaseToHead
    }
    pub const fn provenance(self) -> PullFileProvenance {
        match self {
            Self::LocalExactRange => PullFileProvenance::LocalExactRange,
            Self::GithubPullFiles
            | Self::GithubPullDiff
            | Self::GitlabMergeRequestDiffs
            | Self::GitlabRawDiffs
            | Self::BitbucketCloudDiffstat
            | Self::BitbucketCloudDiff => PullFileProvenance::Provider,
        }
    }

    pub const fn provider(self) -> Option<ProviderKind> {
        match self {
            Self::GithubPullFiles | Self::GithubPullDiff => Some(ProviderKind::Github),
            Self::GitlabMergeRequestDiffs | Self::GitlabRawDiffs => Some(ProviderKind::Gitlab),
            Self::BitbucketCloudDiffstat | Self::BitbucketCloudDiff => {
                Some(ProviderKind::BitbucketCloud)
            }
            Self::LocalExactRange => None,
        }
    }

    const fn supports_collection(self) -> bool {
        matches!(
            self,
            Self::GithubPullFiles
                | Self::GitlabMergeRequestDiffs
                | Self::BitbucketCloudDiffstat
                | Self::LocalExactRange
        )
    }
}

/// Versioned closed strategy evidence, never a response payload or URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileSource {
    pub strategy: PullFileSourceStrategy,
    pub adapter_version: u32,
}

impl PullFileSource {
    pub fn validate(&self) -> Result<()> {
        if self.adapter_version == 0 {
            return Err(invalid_pull_file());
        }
        Ok(())
    }
}

fn validate_provider_cursor(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_PULL_FILE_PROVIDER_CURSOR_BYTES
        || value.chars().any(char::is_control)
    {
        Err(invalid_pull_file())
    } else {
        Ok(())
    }
}

/// Storage-issued authority for exactly one staging generation. Renderer-facing
/// query and hydration types never contain or construct this value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileLease {
    pub run_id: String,
    pub generation: String,
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub binding: PullFileBinding,
    pub source: PullFileSource,
    /// Number of provider pages durably accepted before the next request.
    pub provider_page_count: u32,
    /// Number of rows durably accepted; this is the next page's start position.
    pub accepted_row_count: u32,
    pub next_cursor: Option<String>,
    pub seen_cursors: Vec<String>,
}

impl PullFileLease {
    pub fn validate(&self) -> Result<()> {
        validate_generation(&self.run_id)?;
        validate_generation(&self.generation)?;
        validate_identifier(&self.account_id)?;
        validate_revision(&self.authorization_epoch)?;
        validate_revision(&self.authorization_view)?;
        self.binding.validate()?;
        self.source.validate()?;
        if !self.source.strategy.supports_collection()
            || self.provider_page_count >= MAX_PULL_FILE_PROVIDER_PAGES
            || self.accepted_row_count > MAX_PULL_FILES
            || self.accepted_row_count
                > self
                    .provider_page_count
                    .saturating_mul(MAX_PULL_FILES_PER_PROVIDER_PAGE as u32)
        {
            return Err(invalid_pull_file());
        }
        let continuation_shape_valid = if self.provider_page_count == 0 {
            self.accepted_row_count == 0
                && self.next_cursor.is_none()
                && self.seen_cursors.is_empty()
        } else {
            self.next_cursor.is_some()
                && self.seen_cursors.len() == self.provider_page_count as usize
                && self.seen_cursors.last() == self.next_cursor.as_ref()
        };
        if !continuation_shape_valid {
            return Err(invalid_pull_file());
        }
        let mut unique = HashSet::with_capacity(self.seen_cursors.len());
        for cursor in &self.seen_cursors {
            validate_provider_cursor(cursor)?;
            if !unique.insert(cursor) {
                return Err(invalid_pull_file());
            }
        }
        Ok(())
    }
}

/// Trusted native routing facts resolved before an adapter is called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileBinding {
    pub instance_id: String,
    pub repository_id: String,
    pub repository_provider_id: String,
    pub pull_id: String,
    pub pull_provider_id: String,
    pub number: Option<String>,
    pub context: PullFileContext,
}

impl PullFileBinding {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.instance_id)?;
        validate_identifier(&self.repository_id)?;
        validate_identifier(&self.repository_provider_id)?;
        validate_identifier(&self.pull_id)?;
        validate_identifier(&self.pull_provider_id)?;
        if let Some(number) = &self.number {
            validate_identifier(number)?;
        }
        self.context.validate()
    }
}

/// Trusted collection request. Provider routing comes from native account,
/// repository, pull and instance bindings; there is no caller-selected URL.
#[derive(Debug, Clone)]
pub struct PullFileCollectionRequest {
    pub account: RemoteAccount,
    pub authorization_view: String,
    pub repository: RemoteRepository,
    pub subject: RemoteItem,
    pub binding: PullFileBinding,
    pub source: PullFileSource,
    pub cursor: Option<String>,
    pub start_position: u32,
    pub lease: PullFileLease,
}

impl PullFileCollectionRequest {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account.id)?;
        validate_revision(&self.account.authorization_epoch)?;
        validate_revision(&self.authorization_view)?;
        validate_identifier(&self.repository.id)?;
        validate_identifier(&self.repository.provider_id)?;
        validate_identifier(&self.subject.id)?;
        validate_identifier(&self.subject.provider_id)?;
        self.binding.validate()?;
        self.lease.validate()?;
        if self.account.state != AccountState::Active
            || self.account.id != self.lease.account_id
            || self.account.authorization_epoch != self.lease.authorization_epoch
            || self.authorization_view != self.lease.authorization_view
            || self.repository.account_id != self.account.id
            || self.subject.account_id != self.account.id
            || self.subject.repository_id.as_deref() != Some(self.repository.id.as_str())
            || self.subject.kind != RemoteItemKind::PullRequest
            || self.binding.repository_id != self.repository.id
            || self.binding.repository_provider_id != self.repository.provider_id
            || self.binding.context.base_repository_provider_id
                != self.binding.repository_provider_id
            || self.binding.pull_id != self.subject.id
            || self.binding.pull_provider_id != self.subject.provider_id
            || self.binding.number != self.subject.number
            || self.binding != self.lease.binding
            || self.source != self.lease.source
            || self
                .source
                .strategy
                .provider()
                .is_some_and(|provider| provider != self.account.provider)
            || self.cursor != self.lease.next_cursor
            || self.start_position != self.lease.accepted_row_count
        {
            return Err(invalid_pull_file());
        }
        Ok(())
    }
}

/// One bounded adapter page. Exact context is echoed on every page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullFileProviderPage {
    pub context: PullFileContext,
    pub files: Vec<ProviderPullFile>,
    pub source: PullFileSource,
    pub start_position: u32,
    pub next_cursor: Option<String>,
    pub cap: Option<PullFileCapEvidence>,
    pub freshness_seconds: u32,
    pub cooldown_seconds: Option<u64>,
}

impl PullFileProviderPage {
    pub fn validate_for(&self, request: &PullFileCollectionRequest) -> Result<()> {
        request.validate()?;
        self.context.validate()?;
        if !self.context.is_exact(&request.lease.binding.context)
            || self.start_position != request.lease.accepted_row_count
            || self.files.len() > MAX_PULL_FILES_PER_PROVIDER_PAGE
            || self.source != request.lease.source
            || !self.source.strategy.supports_collection()
            || self
                .source
                .strategy
                .provider()
                .is_some_and(|provider| provider != request.account.provider)
            || self.cap.is_some_and(|cap| !cap.is_valid())
            || self.source.strategy.provenance() == PullFileProvenance::LocalExactRange
                && self
                    .cap
                    .is_some_and(|cap| cap.provenance == PullFileCapProvenance::Provider)
            || self.cap.is_some() && self.next_cursor.is_some()
        {
            return Err(invalid_pull_file());
        }
        let terminal_position = self
            .start_position
            .checked_add(u32::try_from(self.files.len()).map_err(|_| invalid_pull_file())?)
            .ok_or_else(invalid_pull_file)?;
        if terminal_position > MAX_PULL_FILES {
            return Err(invalid_pull_file());
        }
        if terminal_position == MAX_PULL_FILES
            && self.source.strategy == PullFileSourceStrategy::GithubPullFiles
            && !self.cap.is_some_and(|cap| {
                cap.provenance == PullFileCapProvenance::Provider
                    && cap.reason == PullFileCapReason::ProviderFileLimit
            })
        {
            return Err(invalid_pull_file());
        }
        self.source.validate()?;
        if let Some(cursor) = &self.next_cursor {
            validate_provider_cursor(cursor)?;
            let accepted_page_count = request
                .lease
                .provider_page_count
                .checked_add(1)
                .ok_or_else(invalid_pull_file)?;
            if accepted_page_count >= MAX_PULL_FILE_PROVIDER_PAGES
                || request.lease.seen_cursors.contains(cursor)
            {
                return Err(invalid_pull_file());
            }
        }
        for file in &self.files {
            file.validate()?;
        }
        if !pull_file_identities_are_unique(self.files.iter().map(|file| &file.identity)) {
            return Err(invalid_pull_file());
        }
        Ok(())
    }

    /// Advances storage authority only after this page passes validation. The
    /// first continuation records one accepted page; a thirtieth page must be
    /// terminal and therefore cannot produce another lease.
    pub fn next_lease(&self, request: &PullFileCollectionRequest) -> Result<Option<PullFileLease>> {
        self.validate_for(request)?;
        let Some(cursor) = &self.next_cursor else {
            return Ok(None);
        };
        let provider_page_count = request
            .lease
            .provider_page_count
            .checked_add(1)
            .ok_or_else(invalid_pull_file)?;
        let accepted_row_count = request
            .lease
            .accepted_row_count
            .checked_add(u32::try_from(self.files.len()).map_err(|_| invalid_pull_file())?)
            .ok_or_else(invalid_pull_file)?;
        let mut seen_cursors = request.lease.seen_cursors.clone();
        seen_cursors.push(cursor.clone());
        let lease = PullFileLease {
            run_id: request.lease.run_id.clone(),
            generation: request.lease.generation.clone(),
            account_id: request.lease.account_id.clone(),
            authorization_epoch: request.lease.authorization_epoch.clone(),
            authorization_view: request.lease.authorization_view.clone(),
            binding: request.lease.binding.clone(),
            source: request.lease.source.clone(),
            provider_page_count,
            accepted_row_count,
            next_cursor: Some(cursor.clone()),
            seen_cursors,
        };
        lease.validate()?;
        Ok(Some(lease))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileQuery {
    pub account_id: String,
    pub subject_id: String,
    pub cursor: Option<String>,
    pub limit: u32,
}

impl PullFileQuery {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_identifier(&self.subject_id)?;
        if self.limit == 0 || self.limit > MAX_PULL_FILES_PER_LOCAL_PAGE {
            return Err(invalid_pull_file());
        }
        if self.cursor.as_deref().is_some_and(|cursor| {
            cursor.is_empty()
                || cursor.len() > MAX_PULL_FILE_LOCAL_CURSOR_BYTES
                || cursor.chars().any(char::is_control)
        }) {
            return Err(invalid_pull_file());
        }
        Ok(())
    }
}

/// Renderer-safe selected-file identity. The store resolves this key against
/// the exact published generation before any provider or local work starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileDiffRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub subject_id: String,
    pub file_facet_revision: String,
    pub context: PullFileContext,
    pub file_key: String,
}

impl PullFileDiffRequest {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_revision(&self.authorization_epoch)?;
        validate_identifier(&self.subject_id)?;
        validate_revision(&self.file_facet_revision)?;
        self.context.validate()?;
        validate_file_key(&self.file_key)
    }
}

/// Store-resolved active generation evidence. Generation identity is an opaque
/// UUID and is intentionally distinct from the decimal collaboration revision
/// that published the facet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileGenerationReceipt {
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub subject_id: String,
    pub generation: String,
    pub file_facet_revision: String,
    pub context: PullFileContext,
}

impl PullFileGenerationReceipt {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_revision(&self.authorization_epoch)?;
        validate_revision(&self.authorization_view)?;
        validate_identifier(&self.subject_id)?;
        validate_generation(&self.generation)?;
        validate_revision(&self.file_facet_revision)?;
        self.context.validate()
    }

    pub fn is_exact_for(&self, request: &PullFileDiffRequest) -> bool {
        self.validate().is_ok()
            && request.validate().is_ok()
            && self.account_id == request.account_id
            && self.authorization_epoch == request.authorization_epoch
            && self.subject_id == request.subject_id
            && self.file_facet_revision == request.file_facet_revision
            && self.context.is_exact(&request.context)
    }
}

/// Store-resolved membership of one selected file in the active generation.
/// The renderer supplies only the request key; storage supplies this receipt
/// after resolving the exact row and current authorization fence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileMembershipReceipt {
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub subject_id: String,
    pub generation: String,
    pub file_facet_revision: String,
    pub context: PullFileContext,
    pub file_key: String,
    pub identity: PullFileIdentity,
}

impl PullFileMembershipReceipt {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_revision(&self.authorization_epoch)?;
        validate_revision(&self.authorization_view)?;
        validate_identifier(&self.subject_id)?;
        validate_generation(&self.generation)?;
        validate_revision(&self.file_facet_revision)?;
        self.context.validate()?;
        validate_file_key(&self.file_key)?;
        self.identity.validate()
    }

    pub fn is_exact_for(&self, request: &PullFileDiffRequest) -> bool {
        self.validate().is_ok()
            && request.validate().is_ok()
            && self.account_id == request.account_id
            && self.authorization_epoch == request.authorization_epoch
            && self.subject_id == request.subject_id
            && self.file_facet_revision == request.file_facet_revision
            && self.context.is_exact(&request.context)
            && self.file_key == request.file_key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileContentState {
    NotLoaded,
    Text,
    Binary,
    Image,
    Omitted,
    Oversized,
    Unsupported,
    Unavailable,
}

/// Native-owned blob references are opaque identifiers, never file paths or
/// remote URLs supplied by a renderer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileBlobReferences {
    pub old: Option<String>,
    pub new: Option<String>,
}

impl PullFileBlobReferences {
    fn validate(&self) -> Result<()> {
        if let Some(reference) = &self.old {
            validate_blob_reference(reference)?;
        }
        if let Some(reference) = &self.new {
            validate_blob_reference(reference)?;
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.old.is_none() && self.new.is_none()
    }
}

/// Source-specific exact-range validation evidence. Local Git object checks do
/// not fabricate a provider-validation timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PullFileArtifactValidation {
    Provider {
        provider_validated_at: String,
    },
    LocalExactRange {
        local_validated_at: String,
        resolved_merge_base_oid: String,
    },
}

impl PullFileArtifactValidation {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Provider {
                provider_validated_at,
            } => validate_canonical_utc_timestamp(provider_validated_at),
            Self::LocalExactRange {
                local_validated_at,
                resolved_merge_base_oid,
            } => {
                validate_canonical_utc_timestamp(local_validated_at)?;
                if !is_canonical_pull_file_oid(resolved_merge_base_oid) {
                    return Err(invalid_pull_file());
                }
                Ok(())
            }
        }
    }

    const fn provenance(&self) -> PullFileProvenance {
        match self {
            Self::Provider { .. } => PullFileProvenance::Provider,
            Self::LocalExactRange { .. } => PullFileProvenance::LocalExactRange,
        }
    }
}

/// One bounded selected-diff artifact for an exact generation and file key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileArtifact {
    pub account_id: String,
    pub authorization_epoch: String,
    pub authorization_view: String,
    pub subject_id: String,
    pub generation: String,
    pub file_key: String,
    pub identity: PullFileIdentity,
    pub context: PullFileContext,
    pub source: Option<PullFileSource>,
    pub validation: Option<PullFileArtifactValidation>,
    pub content_state: PullFileContentState,
    pub unified_text: Option<String>,
    pub blob_references: PullFileBlobReferences,
    pub old_blob_oid: Option<String>,
    pub new_blob_oid: Option<String>,
    pub content_type: Option<String>,
    pub binary_hint: PullFileFlag,
    pub image_hint: PullFileFlag,
    pub last_access_revision: String,
    /// Decoded/semantic content bytes computed by native admission, never an
    /// adapter- or renderer-supplied accounting claim.
    pub logical_bytes: String,
    /// Retained content bytes computed by storage after persistence.
    pub on_disk_bytes: String,
}

impl PullFileArtifact {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_revision(&self.authorization_epoch)?;
        validate_revision(&self.authorization_view)?;
        validate_identifier(&self.subject_id)?;
        validate_generation(&self.generation)?;
        validate_file_key(&self.file_key)?;
        self.identity.validate()?;
        self.context.validate()?;
        if let Some(source) = &self.source {
            source.validate()?;
        }
        if let Some(validation) = &self.validation {
            validation.validate()?;
            if let PullFileArtifactValidation::LocalExactRange {
                resolved_merge_base_oid,
                ..
            } = validation
                && self
                    .context
                    .merge_base_oid
                    .as_ref()
                    .is_some_and(|observed| observed != resolved_merge_base_oid)
            {
                return Err(invalid_pull_file());
            }
        }
        self.blob_references.validate()?;
        for oid in [&self.old_blob_oid, &self.new_blob_oid]
            .into_iter()
            .flatten()
        {
            if !is_canonical_pull_file_oid(oid) {
                return Err(invalid_pull_file());
            }
        }
        if let Some(content_type) = &self.content_type {
            validate_label(content_type, MAX_CONTENT_TYPE_BYTES)?;
        }
        validate_revision(&self.last_access_revision)?;
        let logical_bytes = validate_decimal(&self.logical_bytes)?;
        let on_disk_bytes = validate_decimal(&self.on_disk_bytes)?;

        let source_validation_matches = self
            .source
            .as_ref()
            .zip(self.validation.as_ref())
            .is_some_and(|(source, validation)| {
                source.strategy.provenance() == validation.provenance()
            });

        match self.content_state {
            PullFileContentState::NotLoaded => {
                if self.source.is_some()
                    || self.validation.is_some()
                    || self.unified_text.is_some()
                    || !self.blob_references.is_empty()
                    || logical_bytes != 0
                    || on_disk_bytes != 0
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Text => {
                let text = self.unified_text.as_ref().ok_or_else(invalid_pull_file)?;
                if !source_validation_matches
                    || !self.blob_references.is_empty()
                    || text.len() > MAX_PULL_FILE_TEXT_BYTES
                    || logical_bytes != text.len() as u64
                    || !valid_text_shape(text)
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Binary | PullFileContentState::Image => {
                if !source_validation_matches
                    || self.unified_text.is_some()
                    || self.blob_references.is_empty()
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Omitted
            | PullFileContentState::Unsupported
            | PullFileContentState::Unavailable => {
                if !source_validation_matches
                    || self.unified_text.is_some()
                    || !self.blob_references.is_empty()
                    || logical_bytes != 0
                    || on_disk_bytes != 0
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Oversized => {
                if !source_validation_matches
                    || self.unified_text.is_some()
                    || !self.blob_references.is_empty()
                    || on_disk_bytes != 0
                {
                    return Err(invalid_pull_file());
                }
            }
        }
        Ok(())
    }

    pub fn is_exact_for(
        &self,
        request: &PullFileDiffRequest,
        membership: &PullFileMembershipReceipt,
    ) -> bool {
        self.validate().is_ok()
            && request.validate().is_ok()
            && membership.validate().is_ok()
            && membership.is_exact_for(request)
            && self.account_id == membership.account_id
            && self.authorization_epoch == membership.authorization_epoch
            && self.authorization_view == membership.authorization_view
            && self.subject_id == membership.subject_id
            && self.generation == membership.generation
            && self.context.is_exact(&membership.context)
            && self.file_key == membership.file_key
            && self.identity == membership.identity
            && self.account_id == request.account_id
            && self.authorization_epoch == request.authorization_epoch
            && self.subject_id == request.subject_id
            && self.file_key == request.file_key
            && self.context.is_exact(&request.context)
    }
}

fn valid_text_shape(text: &str) -> bool {
    if text.contains('\0') {
        return false;
    }
    let mut lines = 0usize;
    for line in text.as_bytes().split(|byte| *byte == b'\n') {
        lines += 1;
        if lines > MAX_PULL_FILE_TEXT_LINES || line.len() > MAX_PULL_FILE_LINE_BYTES {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const OTHER: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const GENERATION: &str = "123e4567-e89b-12d3-a456-426614174000";
    const RUN_ID: &str = "123e4567-e89b-12d3-a456-426614174010";

    fn context() -> PullFileContext {
        PullFileContext {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
            merge_base_oid: None,
            base_repository_provider_id: "base-repository".into(),
            source_repository_provider_id: "source-repository".into(),
            body_metadata_facet_revision: "17".into(),
        }
    }

    fn provider_file() -> ProviderPullFile {
        ProviderPullFile {
            identity: PullFileIdentity {
                old_path: Some("src/ancien-雪.rs".into()),
                new_path: Some("src/new-雪.rs".into()),
            },
            provider_file_id: Some("provider-file-1".into()),
            change_kind: PullFileChangeKind::Renamed,
            provider_change_kind: "renamed".into(),
            additions: PullFileCount::Known("2".into()),
            deletions: PullFileCount::Known("1".into()),
            total_changes: PullFileCount::Known("3".into()),
            old_mode: Some("100644".into()),
            new_mode: Some("100755".into()),
            mode_changed: PullFileFlag::Known(false),
            binary: PullFileFlag::Unknown,
            generated: PullFileFlag::Known(false),
            provider_collapsed: PullFileFlag::Unknown,
            provider_too_large: PullFileFlag::Unknown,
            diff_hint: PullFileDiffHint::Candidate,
        }
    }

    fn provider_files(count: usize) -> Vec<ProviderPullFile> {
        provider_files_from(0, count)
    }

    fn provider_files_from(start: u32, count: usize) -> Vec<ProviderPullFile> {
        (0..count)
            .map(|offset| {
                let index = start + u32::try_from(offset).unwrap();
                let mut file = provider_file();
                file.identity.old_path = Some(format!("old/{index}.rs"));
                file.identity.new_path = Some(format!("new/{index}.rs"));
                file.provider_file_id = Some(format!("provider-file-{index}"));
                file
            })
            .collect()
    }

    fn source(strategy: PullFileSourceStrategy) -> PullFileSource {
        PullFileSource {
            strategy,
            adapter_version: 1,
        }
    }

    fn collection_request() -> PullFileCollectionRequest {
        let account = RemoteAccount {
            id: "account".into(),
            provider: ProviderKind::Github,
            host: "github.com".into(),
            actor_id: "actor".into(),
            login: "octocat".into(),
            display_name: None,
            authorization_epoch: "2".into(),
            state: AccountState::Active,
            notifications_supported: true,
        };
        let repository = RemoteRepository {
            id: "repository".into(),
            account_id: account.id.clone(),
            provider_id: "base-repository".into(),
            full_name: "owner/repository".into(),
            name: "repository".into(),
            web_url: "https://github.com/owner/repository".into(),
            description: None,
            default_branch: Some("main".into()),
            selected: true,
        };
        let subject = RemoteItem {
            id: "pull-1".into(),
            account_id: account.id.clone(),
            repository_id: Some(repository.id.clone()),
            provider_id: "provider-pull-1".into(),
            kind: RemoteItemKind::PullRequest,
            number: Some("1".into()),
            title: "Pull".into(),
            body: None,
            body_omitted: false,
            author: None,
            web_url: None,
            state: "open".into(),
            updated_at: "2026-10-07T12:00:00Z".into(),
            head_oid: Some(HEAD.into()),
            is_draft: Some(false),
            reason: None,
            unread: None,
        };
        let binding = PullFileBinding {
            instance_id: "github-instance".into(),
            repository_id: repository.id.clone(),
            repository_provider_id: repository.provider_id.clone(),
            pull_id: subject.id.clone(),
            pull_provider_id: subject.provider_id.clone(),
            number: subject.number.clone(),
            context: context(),
        };
        let authorization_view = "3".to_owned();
        let lease = PullFileLease {
            run_id: RUN_ID.into(),
            generation: GENERATION.into(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: authorization_view.clone(),
            binding: binding.clone(),
            source: source(PullFileSourceStrategy::GithubPullFiles),
            provider_page_count: 0,
            accepted_row_count: 0,
            next_cursor: None,
            seen_cursors: Vec::new(),
        };
        PullFileCollectionRequest {
            binding,
            account,
            authorization_view,
            repository,
            subject,
            source: lease.source.clone(),
            cursor: None,
            start_position: 0,
            lease,
        }
    }

    fn text_artifact(text: &str) -> PullFileArtifact {
        PullFileArtifact {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            authorization_view: "3".into(),
            subject_id: "pull-1".into(),
            generation: GENERATION.into(),
            file_key: "file:001".into(),
            identity: provider_file().identity,
            context: context(),
            source: Some(source(PullFileSourceStrategy::GithubPullFiles)),
            validation: Some(PullFileArtifactValidation::Provider {
                provider_validated_at: "2026-10-07T12:00:00Z".into(),
            }),
            content_state: PullFileContentState::Text,
            unified_text: Some(text.into()),
            blob_references: PullFileBlobReferences::default(),
            old_blob_oid: Some(BASE.into()),
            new_blob_oid: Some(HEAD.into()),
            content_type: Some("text/x-diff".into()),
            binary_hint: PullFileFlag::Known(false),
            image_hint: PullFileFlag::Known(false),
            last_access_revision: "20".into(),
            logical_bytes: text.len().to_string(),
            on_disk_bytes: text.len().to_string(),
        }
    }

    fn generation_receipt() -> PullFileGenerationReceipt {
        PullFileGenerationReceipt {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            authorization_view: "3".into(),
            subject_id: "pull-1".into(),
            generation: GENERATION.into(),
            file_facet_revision: "19".into(),
            context: context(),
        }
    }

    fn membership_receipt() -> PullFileMembershipReceipt {
        PullFileMembershipReceipt {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            authorization_view: "3".into(),
            subject_id: "pull-1".into(),
            generation: GENERATION.into(),
            file_facet_revision: "19".into(),
            context: context(),
            file_key: "file:001".into(),
            identity: provider_file().identity,
        }
    }

    #[test]
    fn exact_context_and_terminal_validation_compare_every_range_fact() {
        let exact = context();
        assert!(exact.validate().is_ok());
        assert!(exact.is_exact(&context()));

        for changed in [
            PullFileContext {
                base_oid: OTHER.into(),
                ..context()
            },
            PullFileContext {
                head_oid: OTHER.into(),
                ..context()
            },
            PullFileContext {
                merge_base_oid: Some(OTHER.into()),
                ..context()
            },
            PullFileContext {
                base_repository_provider_id: "other-base".into(),
                ..context()
            },
            PullFileContext {
                source_repository_provider_id: "other-source".into(),
                ..context()
            },
            PullFileContext {
                body_metadata_facet_revision: "18".into(),
                ..context()
            },
        ] {
            assert!(!exact.is_exact(&changed));
        }

        let validation = PullFileRangeValidation {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
            merge_base_oid: None,
            base_repository_provider_id: "base-repository".into(),
            source_repository_provider_id: "source-repository".into(),
        };
        assert!(validation.validate().is_ok());
        assert!(exact.matches_range_validation(&validation));
        let mismatched = PullFileRangeValidation {
            head_oid: OTHER.into(),
            ..validation
        };
        assert!(!exact.matches_range_validation(&mismatched));

        let divergent = PullFileContext {
            merge_base_oid: Some(OTHER.into()),
            ..exact.clone()
        };
        assert!(divergent.validate().is_ok());
        let observed = PullFileRangeValidation {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
            merge_base_oid: Some(OTHER.into()),
            base_repository_provider_id: "base-repository".into(),
            source_repository_provider_id: "source-repository".into(),
        };
        assert!(divergent.matches_range_validation(&observed));
        assert!(!exact.matches_range_validation(&observed));

        let mut uppercase = context();
        uppercase.head_oid = HEAD.to_uppercase();
        assert_eq!(
            uppercase.validate().unwrap_err().code,
            ErrorCode::InvalidInput
        );
        let mut noncanonical_revision = context();
        noncanonical_revision.body_metadata_facet_revision = "017".into();
        assert!(noncanonical_revision.validate().is_err());
    }

    #[test]
    fn path_tuple_preserves_remote_identity_without_filesystem_authority() {
        let identity = provider_file().identity;
        assert!(identity.validate().is_ok());
        assert_eq!(identity.old_path.as_deref(), Some("src/ancien-雪.rs"));
        assert_eq!(identity.new_path.as_deref(), Some("src/new-雪.rs"));

        for path in [
            "",
            "/etc/passwd",
            "C:\\secret.txt",
            "\\\\server\\share",
            "../secret",
            "src/../secret",
            "src\\..\\secret",
            "./src.rs",
            "src/./file.rs",
            "src//file.rs",
            "src/",
            "src\\.\\file.rs",
            "src/control\n.rs",
        ] {
            assert!(!is_valid_pull_file_path(path), "accepted {path:?}");
        }
        assert!(is_valid_pull_file_path("src/literal\\name.rs"));
        assert!(is_valid_pull_file_path(
            &"x".repeat(MAX_PULL_FILE_PATH_BYTES)
        ));
        assert!(!is_valid_pull_file_path(
            &"x".repeat(MAX_PULL_FILE_PATH_BYTES + 1)
        ));
        assert!(
            PullFileIdentity {
                old_path: None,
                new_path: None,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn change_kinds_enforce_path_pair_cardinality_without_inference() {
        let cases = [
            (PullFileChangeKind::Added, None, Some("new.rs"), true),
            (
                PullFileChangeKind::Added,
                Some("old.rs"),
                Some("new.rs"),
                false,
            ),
            (PullFileChangeKind::Deleted, Some("old.rs"), None, true),
            (
                PullFileChangeKind::Deleted,
                Some("old.rs"),
                Some("new.rs"),
                false,
            ),
            (
                PullFileChangeKind::Renamed,
                Some("old.rs"),
                Some("new.rs"),
                true,
            ),
            (PullFileChangeKind::Renamed, None, Some("new.rs"), false),
            (
                PullFileChangeKind::Copied,
                Some("old.rs"),
                Some("copy.rs"),
                true,
            ),
            (PullFileChangeKind::Copied, Some("old.rs"), None, false),
            (PullFileChangeKind::Modified, None, Some("same.rs"), true),
            (PullFileChangeKind::TypeChanged, Some("same.rs"), None, true),
            (PullFileChangeKind::Unknown, None, Some("observed.rs"), true),
            (PullFileChangeKind::Unknown, None, None, false),
        ];
        for (kind, old_path, new_path, valid) in cases {
            let mut file = provider_file();
            file.change_kind = kind;
            file.identity = PullFileIdentity {
                old_path: old_path.map(str::to_owned),
                new_path: new_path.map(str::to_owned),
            };
            assert_eq!(file.validate().is_ok(), valid, "{kind:?}");
        }
    }

    #[test]
    fn counts_flags_and_native_states_preserve_unknown_without_inference() {
        assert!(PullFileCount::Unknown.validate().is_ok());
        assert_eq!(
            PullFileCount::known("0").unwrap(),
            PullFileCount::Known("0".into())
        );
        for malformed in ["", "01", "-1", "18446744073709551616"] {
            assert!(PullFileCount::Known(malformed.into()).validate().is_err());
        }
        assert_ne!(PullFileFlag::Unknown, PullFileFlag::Known(false));

        let mut file = provider_file();
        file.provider_change_kind = "x".repeat(MAX_PULL_FILE_NATIVE_STATE_BYTES);
        assert!(file.validate().is_ok());
        file.provider_change_kind.push('x');
        assert!(file.validate().is_err());
    }

    #[test]
    fn provider_pages_are_range_bound_bounded_and_reject_duplicate_path_tuples() {
        let request = collection_request();
        let page = PullFileProviderPage {
            context: context(),
            files: provider_files(MAX_PULL_FILES_PER_PROVIDER_PAGE),
            source: source(PullFileSourceStrategy::GithubPullFiles),
            start_position: 0,
            next_cursor: Some("page-2".into()),
            cap: None,
            freshness_seconds: 30,
            cooldown_seconds: None,
        };
        assert!(page.validate_for(&request).is_ok());

        let mut oversized = page.clone();
        oversized.files.push(provider_files(101).pop().unwrap());
        assert!(oversized.validate_for(&request).is_err());

        let mut wrong_range = page.clone();
        wrong_range.context.head_oid = OTHER.into();
        assert!(wrong_range.validate_for(&request).is_err());

        let mut duplicate_tuple = page.clone();
        let mut duplicate = duplicate_tuple.files[0].clone();
        duplicate.provider_file_id = Some("different-supplemental-id".into());
        duplicate_tuple.files[1] = duplicate;
        assert!(duplicate_tuple.validate_for(&request).is_err());
        assert_eq!(
            duplicate_tuple.files[0].identity,
            duplicate_tuple.files[1].identity
        );
        assert_ne!(
            duplicate_tuple.files[0].provider_file_id,
            duplicate_tuple.files[1].provider_file_id
        );
    }

    #[test]
    fn github_exact_three_thousand_boundary_requires_provider_limit_evidence() {
        let mut request = collection_request();
        for page_number in 1..MAX_PULL_FILE_PROVIDER_PAGES {
            let start_position = request.lease.accepted_row_count;
            let page = PullFileProviderPage {
                context: context(),
                files: provider_files_from(start_position, MAX_PULL_FILES_PER_PROVIDER_PAGE),
                source: request.lease.source.clone(),
                start_position,
                next_cursor: Some(format!("cursor-{page_number}")),
                cap: None,
                freshness_seconds: 30,
                cooldown_seconds: None,
            };
            let next = page.next_lease(&request).unwrap().unwrap();
            request.source = next.source.clone();
            request.cursor = next.next_cursor.clone();
            request.start_position = next.accepted_row_count;
            request.lease = next;
        }
        assert_eq!(
            request.lease.provider_page_count,
            MAX_PULL_FILE_PROVIDER_PAGES - 1
        );
        assert_eq!(
            request.lease.accepted_row_count,
            MAX_PULL_FILES - MAX_PULL_FILES_PER_PROVIDER_PAGE as u32
        );

        let boundary = PullFileProviderPage {
            context: context(),
            files: provider_files_from(
                request.lease.accepted_row_count,
                MAX_PULL_FILES_PER_PROVIDER_PAGE,
            ),
            source: request.lease.source.clone(),
            start_position: request.lease.accepted_row_count,
            next_cursor: None,
            cap: None,
            freshness_seconds: 30,
            cooldown_seconds: None,
        };
        assert!(boundary.validate_for(&request).is_err());

        let local_cap = PullFileProviderPage {
            cap: Some(PullFileCapEvidence {
                provenance: PullFileCapProvenance::Local,
                reason: PullFileCapReason::LocalFileLimit,
                remote_has_more: PullFileFlag::Unknown,
            }),
            ..boundary.clone()
        };
        assert!(local_cap.validate_for(&request).is_err());

        let provider_cap = PullFileProviderPage {
            cap: Some(PullFileCapEvidence {
                provenance: PullFileCapProvenance::Provider,
                reason: PullFileCapReason::ProviderFileLimit,
                remote_has_more: PullFileFlag::Unknown,
            }),
            ..boundary
        };
        assert!(provider_cap.validate_for(&request).is_ok());
    }

    #[test]
    fn lease_counts_pages_and_rows_and_rejects_cursor_cycles_and_position_jumps() {
        let first_request = collection_request();
        let first_page = PullFileProviderPage {
            context: context(),
            files: provider_files(1),
            source: source(PullFileSourceStrategy::GithubPullFiles),
            start_position: 0,
            next_cursor: Some("cursor-a".into()),
            cap: None,
            freshness_seconds: 30,
            cooldown_seconds: None,
        };
        let first = first_page.next_lease(&first_request).unwrap().unwrap();
        assert_eq!(first.provider_page_count, 1);
        assert_eq!(first.accepted_row_count, 1);
        assert_eq!(first.seen_cursors, ["cursor-a"]);

        let mut second_request = collection_request();
        second_request.lease = first;
        second_request.cursor = second_request.lease.next_cursor.clone();
        second_request.start_position = second_request.lease.accepted_row_count;
        let second_page = PullFileProviderPage {
            start_position: 1,
            next_cursor: Some("cursor-b".into()),
            ..first_page.clone()
        };
        let switched_source = PullFileProviderPage {
            source: PullFileSource {
                strategy: PullFileSourceStrategy::GithubPullFiles,
                adapter_version: 2,
            },
            ..second_page.clone()
        };
        assert!(switched_source.validate_for(&second_request).is_err());
        let second = second_page.next_lease(&second_request).unwrap().unwrap();
        assert_eq!(second.provider_page_count, 2);
        assert_eq!(second.accepted_row_count, 2);
        assert_eq!(second.seen_cursors, ["cursor-a", "cursor-b"]);

        let mut cyclic_request = collection_request();
        cyclic_request.lease = second.clone();
        cyclic_request.cursor = cyclic_request.lease.next_cursor.clone();
        cyclic_request.start_position = cyclic_request.lease.accepted_row_count;
        let cyclic_page = PullFileProviderPage {
            start_position: 2,
            next_cursor: Some("cursor-a".into()),
            ..first_page.clone()
        };
        assert!(cyclic_page.validate_for(&cyclic_request).is_err());

        let position_jump = PullFileProviderPage {
            start_position: MAX_PULL_FILES - 1,
            next_cursor: None,
            ..second_page.clone()
        };
        assert!(position_jump.validate_for(&cyclic_request).is_err());

        let forged_jump = PullFileLease {
            provider_page_count: 1,
            accepted_row_count: MAX_PULL_FILES - 1,
            next_cursor: Some("cursor-a".into()),
            seen_cursors: vec!["cursor-a".into()],
            ..first_request.lease.clone()
        };
        assert!(forged_jump.validate().is_err());

        let forged_cycle = PullFileLease {
            provider_page_count: 2,
            accepted_row_count: 2,
            next_cursor: Some("cursor-b".into()),
            seen_cursors: vec!["cursor-b".into(), "cursor-b".into()],
            ..first_request.lease
        };
        assert!(forged_cycle.validate().is_err());
    }

    #[test]
    fn native_routing_binding_and_closed_source_must_match_provider_identity() {
        let request = collection_request();
        assert!(request.validate().is_ok());

        let mut wrong_repository = collection_request();
        wrong_repository.binding.repository_provider_id = "other-repository".into();
        assert!(wrong_repository.validate().is_err());

        let mut wrong_pull = collection_request();
        wrong_pull.binding.pull_provider_id = "other-pull".into();
        assert!(wrong_pull.validate().is_err());

        let mut wrong_base_context = collection_request();
        wrong_base_context
            .binding
            .context
            .base_repository_provider_id = "other-repository".into();
        assert!(wrong_base_context.validate().is_err());

        let mut changed_range = collection_request();
        changed_range.binding.context.head_oid = OTHER.into();
        assert!(changed_range.validate().is_err());

        let mut changed_epoch = collection_request();
        changed_epoch.account.authorization_epoch = "3".into();
        assert!(changed_epoch.validate().is_err());

        let mut changed_authorization_view = collection_request();
        changed_authorization_view.authorization_view = "4".into();
        assert!(changed_authorization_view.validate().is_err());

        let mut changed_source_version = collection_request();
        changed_source_version.source.adapter_version = 2;
        assert!(changed_source_version.validate().is_err());

        let mut changed_cursor = collection_request();
        changed_cursor.cursor = Some("unleased-cursor".into());
        assert!(changed_cursor.validate().is_err());

        let mut changed_start_position = collection_request();
        changed_start_position.start_position = 1;
        assert!(changed_start_position.validate().is_err());

        let gitlab_page = PullFileProviderPage {
            context: context(),
            files: provider_files(1),
            source: source(PullFileSourceStrategy::GitlabMergeRequestDiffs),
            start_position: 0,
            next_cursor: None,
            cap: None,
            freshness_seconds: 30,
            cooldown_seconds: None,
        };
        assert!(gitlab_page.validate_for(&request).is_err());

        let mut wrong_provider_request = collection_request();
        wrong_provider_request.account.provider = ProviderKind::Gitlab;
        assert!(wrong_provider_request.validate().is_err());

        let mut local_request = collection_request();
        local_request.source = source(PullFileSourceStrategy::LocalExactRange);
        local_request.lease.source = local_request.source.clone();
        assert!(local_request.validate().is_ok());

        let serialized =
            serde_json::to_string(&source(PullFileSourceStrategy::GithubPullFiles)).unwrap();
        assert_eq!(
            serialized,
            r#"{"strategy":"github_pull_files","adapter_version":1}"#
        );
        assert!(!serialized.contains("http"));
    }

    #[test]
    fn cap_reason_must_match_provenance_and_is_always_terminal() {
        let request = collection_request();
        let provider_cap = PullFileCapEvidence {
            provenance: PullFileCapProvenance::Provider,
            reason: PullFileCapReason::ProviderFileLimit,
            remote_has_more: PullFileFlag::Unknown,
        };
        let provider_page = PullFileProviderPage {
            context: context(),
            files: provider_files(1),
            source: source(PullFileSourceStrategy::GithubPullFiles),
            start_position: 0,
            next_cursor: None,
            cap: Some(provider_cap),
            freshness_seconds: 30,
            cooldown_seconds: None,
        };
        assert!(provider_page.validate_for(&request).is_ok());

        let nonterminal_cap = PullFileProviderPage {
            next_cursor: Some("forbidden-next".into()),
            ..provider_page.clone()
        };
        assert!(nonterminal_cap.validate_for(&request).is_err());

        let local_reason_with_provider_provenance = PullFileProviderPage {
            cap: Some(PullFileCapEvidence {
                provenance: PullFileCapProvenance::Provider,
                reason: PullFileCapReason::LocalByteLimit,
                remote_has_more: PullFileFlag::Known(true),
            }),
            ..provider_page.clone()
        };
        assert!(
            local_reason_with_provider_provenance
                .validate_for(&request)
                .is_err()
        );

        let provider_reason_with_local_provenance = PullFileProviderPage {
            cap: Some(PullFileCapEvidence {
                provenance: PullFileCapProvenance::Local,
                reason: PullFileCapReason::ProviderFileLimit,
                remote_has_more: PullFileFlag::Unknown,
            }),
            ..provider_page.clone()
        };
        assert!(
            provider_reason_with_local_provenance
                .validate_for(&request)
                .is_err()
        );

        // A local safety bound can terminate provider-sourced rows without
        // rewriting their content provenance.
        let local_cap = PullFileProviderPage {
            cap: Some(PullFileCapEvidence {
                provenance: PullFileCapProvenance::Local,
                reason: PullFileCapReason::LocalByteLimit,
                remote_has_more: PullFileFlag::Unknown,
            }),
            ..provider_page
        };
        assert!(local_cap.validate_for(&request).is_ok());

        let local_source = source(PullFileSourceStrategy::LocalExactRange);
        let mut local_request = collection_request();
        local_request.source = local_source.clone();
        local_request.lease.source = local_source.clone();
        let provider_cap_from_local_source = PullFileProviderPage {
            source: local_source.clone(),
            cap: Some(provider_cap),
            ..local_cap.clone()
        };
        assert!(
            provider_cap_from_local_source
                .validate_for(&local_request)
                .is_err()
        );

        let local_cap_from_local_source = PullFileProviderPage {
            source: local_source,
            ..local_cap
        };
        assert!(
            local_cap_from_local_source
                .validate_for(&local_request)
                .is_ok()
        );
    }

    #[test]
    fn capped_completeness_retains_exact_reason_and_remote_more_evidence() {
        let cap = PullFileCapEvidence {
            provenance: PullFileCapProvenance::Provider,
            reason: PullFileCapReason::ProviderFileLimit,
            remote_has_more: PullFileFlag::Unknown,
        };
        let capped = PullFileCompleteness::capped(cap);
        assert!(capped.is_valid());
        assert_eq!(capped.cap, Some(cap));
        assert!(
            !PullFileCompleteness {
                state: PullFileCompletenessState::Complete,
                cap: Some(cap),
            }
            .is_valid()
        );
        assert!(
            !PullFileCompleteness {
                state: PullFileCompletenessState::Capped,
                cap: None,
            }
            .is_valid()
        );
    }

    #[test]
    fn selected_artifact_requires_active_uuid_generation_epoch_and_facet_receipt() {
        let request = PullFileDiffRequest {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            subject_id: "pull-1".into(),
            file_facet_revision: "19".into(),
            context: context(),
            file_key: "file:001".into(),
        };
        assert!(request.validate().is_ok());
        let active = generation_receipt();
        assert!(active.validate().is_ok());
        let membership = membership_receipt();
        assert!(membership.validate().is_ok());
        assert!(text_artifact("").is_exact_for(&request, &membership));

        let mut stale_epoch = text_artifact("");
        stale_epoch.authorization_epoch = "1".into();
        assert!(!stale_epoch.is_exact_for(&request, &membership));

        let stale_generation = PullFileMembershipReceipt {
            generation: "123e4567-e89b-12d3-a456-426614174099".into(),
            ..membership.clone()
        };
        assert!(!text_artifact("").is_exact_for(&request, &stale_generation));

        let stale_facet = PullFileMembershipReceipt {
            file_facet_revision: "18".into(),
            ..membership.clone()
        };
        assert!(!text_artifact("").is_exact_for(&request, &stale_facet));

        let stale_view = PullFileMembershipReceipt {
            authorization_view: "4".into(),
            ..membership.clone()
        };
        assert!(!text_artifact("").is_exact_for(&request, &stale_view));

        let stale_identity = PullFileMembershipReceipt {
            identity: PullFileIdentity {
                old_path: Some("other-old.rs".into()),
                new_path: Some("other-new.rs".into()),
            },
            ..membership.clone()
        };
        assert!(!text_artifact("").is_exact_for(&request, &stale_identity));

        let invalid_membership = PullFileMembershipReceipt {
            authorization_view: "0".into(),
            ..membership.clone()
        };
        assert!(!text_artifact("").is_exact_for(&request, &invalid_membership));

        let mut invalid_artifact = text_artifact("text");
        invalid_artifact.logical_bytes = "3".into();
        assert!(!invalid_artifact.is_exact_for(&request, &membership));

        let decimal_generation = PullFileGenerationReceipt {
            generation: "19".into(),
            ..active.clone()
        };
        assert!(decimal_generation.validate().is_err());
        let uppercase_generation = PullFileGenerationReceipt {
            generation: GENERATION.to_uppercase(),
            ..active
        };
        assert!(uppercase_generation.validate().is_err());

        for file_key in ["../src/lib.rs", "https://provider.test/file", "file/key"] {
            let malformed = PullFileDiffRequest {
                file_key: file_key.into(),
                ..request.clone()
            };
            assert!(malformed.validate().is_err());
            assert!(!text_artifact("").is_exact_for(&malformed, &membership));
        }
    }

    #[test]
    fn artifacts_distinguish_empty_text_binary_and_no_content_states() {
        let empty = text_artifact("");
        assert!(empty.validate().is_ok());
        assert_eq!(empty.content_state, PullFileContentState::Text);
        assert_eq!(empty.unified_text.as_deref(), Some(""));

        let binary = PullFileArtifact {
            source: Some(source(PullFileSourceStrategy::LocalExactRange)),
            validation: Some(PullFileArtifactValidation::LocalExactRange {
                local_validated_at: "2026-10-07T12:00:00Z".into(),
                resolved_merge_base_oid: BASE.into(),
            }),
            content_state: PullFileContentState::Binary,
            unified_text: None,
            blob_references: PullFileBlobReferences {
                old: Some("blob:old".into()),
                new: Some("blob:new".into()),
            },
            logical_bytes: "12".into(),
            on_disk_bytes: "12".into(),
            ..empty.clone()
        };
        assert!(binary.validate().is_ok());

        for state in [
            PullFileContentState::Omitted,
            PullFileContentState::Oversized,
            PullFileContentState::Unsupported,
            PullFileContentState::Unavailable,
        ] {
            let artifact = PullFileArtifact {
                content_state: state,
                unified_text: None,
                logical_bytes: "0".into(),
                on_disk_bytes: "0".into(),
                ..empty.clone()
            };
            assert!(artifact.validate().is_ok());
        }

        let not_loaded = PullFileArtifact {
            source: None,
            validation: None,
            content_state: PullFileContentState::NotLoaded,
            unified_text: None,
            logical_bytes: "0".into(),
            on_disk_bytes: "0".into(),
            ..empty
        };
        assert!(not_loaded.validate().is_ok());
    }

    #[test]
    fn artifact_text_limits_and_blob_oids_fail_closed() {
        let mut mismatched_size = text_artifact("abc");
        mismatched_size.logical_bytes = "2".into();
        assert!(mismatched_size.validate().is_err());

        let long_line = "x".repeat(MAX_PULL_FILE_LINE_BYTES + 1);
        assert!(text_artifact(&long_line).validate().is_err());

        let too_many_lines = "\n".repeat(MAX_PULL_FILE_TEXT_LINES);
        assert!(text_artifact(&too_many_lines).validate().is_err());

        let mut uppercase_oid = text_artifact("text");
        uppercase_oid.new_blob_oid = Some(HEAD.to_uppercase());
        assert!(uppercase_oid.validate().is_err());

        let mut remote_blob = PullFileArtifact {
            source: Some(source(PullFileSourceStrategy::LocalExactRange)),
            validation: Some(PullFileArtifactValidation::LocalExactRange {
                local_validated_at: "2026-10-07T12:00:00Z".into(),
                resolved_merge_base_oid: BASE.into(),
            }),
            content_state: PullFileContentState::Image,
            unified_text: None,
            blob_references: PullFileBlobReferences {
                old: None,
                new: Some("https://provider.test/image".into()),
            },
            logical_bytes: "1".into(),
            on_disk_bytes: "1".into(),
            ..text_artifact("x")
        };
        assert!(remote_blob.validate().is_err());
        remote_blob.blob_references.new = Some("blob:image".into());
        assert!(remote_blob.validate().is_ok());

        let mut non_utc = text_artifact("text");
        non_utc.validation = Some(PullFileArtifactValidation::Provider {
            provider_validated_at: "2026-10-07T17:30:00+05:30".into(),
        });
        assert!(non_utc.validate().is_err());
        non_utc.validation = Some(PullFileArtifactValidation::Provider {
            provider_validated_at: "2026-10-07T12:00:00+00:00".into(),
        });
        assert!(non_utc.validate().is_err());
        let mut missing_validation = text_artifact("text");
        missing_validation.validation = None;
        assert!(missing_validation.validate().is_err());

        let mut false_provider_evidence = text_artifact("text");
        false_provider_evidence.source = Some(source(PullFileSourceStrategy::LocalExactRange));
        assert!(false_provider_evidence.validate().is_err());
        false_provider_evidence.validation = Some(PullFileArtifactValidation::LocalExactRange {
            local_validated_at: "2026-10-07T12:00:00Z".into(),
            resolved_merge_base_oid: BASE.into(),
        });
        assert!(false_provider_evidence.validate().is_ok());

        let retained_oversized_size = PullFileArtifact {
            content_state: PullFileContentState::Oversized,
            unified_text: None,
            logical_bytes: (MAX_PULL_FILE_TEXT_BYTES + 1).to_string(),
            on_disk_bytes: "0".into(),
            ..text_artifact("x")
        };
        assert!(retained_oversized_size.validate().is_ok());

        let retained_omitted_bytes = PullFileArtifact {
            content_state: PullFileContentState::Omitted,
            unified_text: None,
            logical_bytes: "1".into(),
            on_disk_bytes: "1".into(),
            ..text_artifact("x")
        };
        assert!(retained_omitted_bytes.validate().is_err());
    }
}
