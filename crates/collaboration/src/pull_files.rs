//! Provider-independent pull-request file and selected-diff facts.
//!
//! Remote paths are bounded labels. Nothing in this module turns one into a
//! local path or accepts a renderer-selected provider URL as fetch authority.

use crate::{CollaborationError, is_canonical_commit_oid};
use serde::{Deserialize, Serialize};

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
    !value.split(['/', '\\']).any(|component| component == "..")
}

/// Exact Body-derived authority for one file generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PullFileContext {
    pub base_oid: String,
    pub head_oid: String,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
    pub body_metadata_facet_revision: String,
}

impl PullFileContext {
    pub fn validate(&self) -> Result<()> {
        if !is_canonical_pull_file_oid(&self.base_oid)
            || !is_canonical_pull_file_oid(&self.head_oid)
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

    /// Terminal publication checks the four freshly observed provider facts;
    /// the Body revision remains bound by this context itself.
    pub fn matches_range_validation(&self, validation: &PullFileRangeValidation) -> bool {
        self.base_oid == validation.base_oid
            && self.head_oid == validation.head_oid
            && self.base_repository_provider_id == validation.base_repository_provider_id
            && self.source_repository_provider_id == validation.source_repository_provider_id
    }
}

/// Fresh parent facts checked before a terminal generation is published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullFileRangeValidation {
    pub base_oid: String,
    pub head_oid: String,
    pub base_repository_provider_id: String,
    pub source_repository_provider_id: String,
}

impl PullFileRangeValidation {
    pub fn validate(&self) -> Result<()> {
        if !is_canonical_pull_file_oid(&self.base_oid)
            || !is_canonical_pull_file_oid(&self.head_oid)
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
    /// Supplemental adapter-owned evidence; the path tuple remains identity.
    pub provider_file_id: Option<String>,
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
        if let Some(provider_file_id) = &self.provider_file_id {
            validate_identifier(provider_file_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
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
    pub change_kind: PullFileChangeKind,
    pub provider_change_kind: String,
    pub additions: PullFileCount,
    pub deletions: PullFileCount,
    pub total_changes: PullFileCount,
    pub mode_changed: PullFileFlag,
    pub binary: PullFileFlag,
    pub generated: PullFileFlag,
    pub diff_hint: PullFileDiffHint,
}

impl ProviderPullFile {
    pub fn validate(&self) -> Result<()> {
        self.identity.validate()?;
        validate_label(&self.provider_change_kind, MAX_PULL_FILE_NATIVE_STATE_BYTES)?;
        self.additions.validate()?;
        self.deletions.validate()?;
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
            self.file.identity.provider_file_id.as_deref(),
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

/// Exact terminal cap evidence. `remote_has_more` may itself be unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileCapEvidence {
    pub reason: PullFileCapReason,
    pub remote_has_more: PullFileFlag,
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
        matches!(
            (self.state, self.cap),
            (PullFileCompletenessState::Capped, Some(_))
                | (
                    PullFileCompletenessState::Missing
                        | PullFileCompletenessState::Syncing
                        | PullFileCompletenessState::Complete
                        | PullFileCompletenessState::Partial,
                    None
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullFileProvenance {
    Provider,
    LocalExactRange,
}

/// Versioned adapter/strategy evidence, never a response payload or URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileSource {
    pub provenance: PullFileProvenance,
    pub source: String,
    pub adapter_version: u32,
}

impl PullFileSource {
    pub fn validate(&self) -> Result<()> {
        validate_label(&self.source, MAX_PULL_FILE_NATIVE_STATE_BYTES)?;
        if self.source.contains("://") || self.adapter_version == 0 {
            return Err(invalid_pull_file());
        }
        Ok(())
    }
}

/// Bounded opaque provider continuation plus its traversal depth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileProviderCursor {
    pub value: String,
    pub page: u32,
}

impl PullFileProviderCursor {
    pub fn validate(&self) -> Result<()> {
        if self.value.is_empty()
            || self.value.len() > MAX_PULL_FILE_PROVIDER_CURSOR_BYTES
            || self.value.chars().any(char::is_control)
            || self.page == 0
            || self.page > MAX_PULL_FILE_PROVIDER_PAGES
        {
            Err(invalid_pull_file())
        } else {
            Ok(())
        }
    }
}

/// Trusted collection request. It contains no endpoint, URL or local path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullFileCollectionRequest {
    pub account_id: String,
    pub authorization_epoch: String,
    pub subject_id: String,
    pub context: PullFileContext,
    pub cursor: Option<PullFileProviderCursor>,
    pub start_position: u32,
}

impl PullFileCollectionRequest {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_revision(&self.authorization_epoch)?;
        validate_identifier(&self.subject_id)?;
        self.context.validate()?;
        if self.start_position > MAX_PULL_FILES {
            return Err(invalid_pull_file());
        }
        if let Some(cursor) = &self.cursor {
            cursor.validate()?;
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
    pub next_cursor: Option<PullFileProviderCursor>,
    pub cap: Option<PullFileCapEvidence>,
    pub freshness_seconds: u32,
    pub cooldown_seconds: Option<u64>,
}

impl PullFileProviderPage {
    pub fn validate_for(&self, request: &PullFileCollectionRequest) -> Result<()> {
        request.validate()?;
        self.context.validate()?;
        if !self.context.is_exact(&request.context)
            || self.start_position != request.start_position
            || self.files.len() > MAX_PULL_FILES_PER_PROVIDER_PAGE
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
        self.source.validate()?;
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if request
                .cursor
                .as_ref()
                .is_some_and(|request_cursor| request_cursor.value == cursor.value)
            {
                return Err(invalid_pull_file());
            }
        }
        for file in &self.files {
            file.validate()?;
        }
        Ok(())
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

/// One bounded selected-diff artifact for an exact generation and file key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFileArtifact {
    pub account_id: String,
    pub subject_id: String,
    pub generation: String,
    pub file_key: String,
    pub context: PullFileContext,
    pub source: Option<PullFileSource>,
    pub content_state: PullFileContentState,
    pub unified_text: Option<String>,
    pub blob_references: PullFileBlobReferences,
    pub old_blob_oid: Option<String>,
    pub new_blob_oid: Option<String>,
    pub content_type: Option<String>,
    pub binary_hint: PullFileFlag,
    pub image_hint: PullFileFlag,
    pub provider_validated_at: Option<String>,
    pub last_access_revision: String,
    pub logical_bytes: String,
    pub on_disk_bytes: String,
}

impl PullFileArtifact {
    pub fn validate(&self) -> Result<()> {
        validate_identifier(&self.account_id)?;
        validate_identifier(&self.subject_id)?;
        validate_revision(&self.generation)?;
        validate_file_key(&self.file_key)?;
        self.context.validate()?;
        if let Some(source) = &self.source {
            source.validate()?;
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
        if let Some(validated_at) = &self.provider_validated_at {
            validate_label(validated_at, MAX_TIMESTAMP_BYTES)?;
        }
        validate_revision(&self.last_access_revision)?;
        let logical_bytes = validate_decimal(&self.logical_bytes)?;
        validate_decimal(&self.on_disk_bytes)?;

        match self.content_state {
            PullFileContentState::NotLoaded => {
                if self.source.is_some()
                    || self.unified_text.is_some()
                    || !self.blob_references.is_empty()
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Text => {
                let text = self.unified_text.as_ref().ok_or_else(invalid_pull_file)?;
                if self.source.is_none()
                    || !self.blob_references.is_empty()
                    || text.len() > MAX_PULL_FILE_TEXT_BYTES
                    || logical_bytes != text.len() as u64
                    || !valid_text_shape(text)
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Binary | PullFileContentState::Image => {
                if self.source.is_none()
                    || self.unified_text.is_some()
                    || self.blob_references.is_empty()
                {
                    return Err(invalid_pull_file());
                }
            }
            PullFileContentState::Omitted
            | PullFileContentState::Oversized
            | PullFileContentState::Unsupported
            | PullFileContentState::Unavailable => {
                if self.source.is_none()
                    || self.unified_text.is_some()
                    || !self.blob_references.is_empty()
                {
                    return Err(invalid_pull_file());
                }
            }
        }
        Ok(())
    }

    pub fn is_exact_for(&self, request: &PullFileDiffRequest) -> bool {
        self.account_id == request.account_id
            && self.subject_id == request.subject_id
            && self.generation == request.file_facet_revision
            && self.file_key == request.file_key
            && self.context.is_exact(&request.context)
    }
}

fn valid_text_shape(text: &str) -> bool {
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

    fn context() -> PullFileContext {
        PullFileContext {
            base_oid: BASE.into(),
            head_oid: HEAD.into(),
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
                provider_file_id: Some("provider-file-1".into()),
            },
            change_kind: PullFileChangeKind::Renamed,
            provider_change_kind: "renamed".into(),
            additions: PullFileCount::Known("2".into()),
            deletions: PullFileCount::Known("1".into()),
            total_changes: PullFileCount::Known("3".into()),
            mode_changed: PullFileFlag::Known(false),
            binary: PullFileFlag::Unknown,
            generated: PullFileFlag::Known(false),
            diff_hint: PullFileDiffHint::Candidate,
        }
    }

    fn source(provenance: PullFileProvenance) -> PullFileSource {
        PullFileSource {
            provenance,
            source: "fixture.pull_files.v1".into(),
            adapter_version: 1,
        }
    }

    fn collection_request() -> PullFileCollectionRequest {
        PullFileCollectionRequest {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            subject_id: "pull-1".into(),
            context: context(),
            cursor: None,
            start_position: 0,
        }
    }

    fn text_artifact(text: &str) -> PullFileArtifact {
        PullFileArtifact {
            account_id: "account".into(),
            subject_id: "pull-1".into(),
            generation: "19".into(),
            file_key: "file:001".into(),
            context: context(),
            source: Some(source(PullFileProvenance::Provider)),
            content_state: PullFileContentState::Text,
            unified_text: Some(text.into()),
            blob_references: PullFileBlobReferences::default(),
            old_blob_oid: Some(BASE.into()),
            new_blob_oid: Some(HEAD.into()),
            content_type: Some("text/x-diff".into()),
            binary_hint: PullFileFlag::Known(false),
            image_hint: PullFileFlag::Known(false),
            provider_validated_at: Some("2026-10-07T12:00:00Z".into()),
            last_access_revision: "20".into(),
            logical_bytes: text.len().to_string(),
            on_disk_bytes: text.len().to_string(),
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
                provider_file_id: Some("supplemental".into()),
            }
            .validate()
            .is_err()
        );
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
    fn provider_pages_are_range_bound_bounded_and_reject_repeated_cursors() {
        let request = collection_request();
        let page = PullFileProviderPage {
            context: context(),
            files: vec![provider_file(); MAX_PULL_FILES_PER_PROVIDER_PAGE],
            source: source(PullFileProvenance::Provider),
            start_position: 0,
            next_cursor: Some(PullFileProviderCursor {
                value: "page-2".into(),
                page: 1,
            }),
            cap: None,
            freshness_seconds: 30,
            cooldown_seconds: None,
        };
        assert!(page.validate_for(&request).is_ok());

        let mut oversized = page.clone();
        oversized.files.push(provider_file());
        assert!(oversized.validate_for(&request).is_err());

        let mut wrong_range = page.clone();
        wrong_range.context.head_oid = OTHER.into();
        assert!(wrong_range.validate_for(&request).is_err());

        let continued_request = PullFileCollectionRequest {
            cursor: Some(PullFileProviderCursor {
                value: "same".into(),
                page: 1,
            }),
            ..collection_request()
        };
        let repeated = PullFileProviderPage {
            next_cursor: Some(PullFileProviderCursor {
                value: "same".into(),
                page: 2,
            }),
            ..page
        };
        assert!(repeated.validate_for(&continued_request).is_err());

        let too_deep = PullFileProviderPage {
            next_cursor: Some(PullFileProviderCursor {
                value: "past-provider-bound".into(),
                page: MAX_PULL_FILE_PROVIDER_PAGES + 1,
            }),
            ..repeated
        };
        assert!(too_deep.validate_for(&collection_request()).is_err());
    }

    #[test]
    fn capped_completeness_retains_exact_reason_and_remote_more_evidence() {
        let cap = PullFileCapEvidence {
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
    fn selected_request_is_only_an_exact_generation_and_opaque_file_key() {
        let request = PullFileDiffRequest {
            account_id: "account".into(),
            authorization_epoch: "2".into(),
            subject_id: "pull-1".into(),
            file_facet_revision: "19".into(),
            context: context(),
            file_key: "file:001".into(),
        };
        assert!(request.validate().is_ok());
        assert!(text_artifact("").is_exact_for(&request));

        for file_key in ["../src/lib.rs", "https://provider.test/file", "file/key"] {
            let malformed = PullFileDiffRequest {
                file_key: file_key.into(),
                ..request.clone()
            };
            assert!(malformed.validate().is_err());
        }
    }

    #[test]
    fn artifacts_distinguish_empty_text_binary_and_no_content_states() {
        let empty = text_artifact("");
        assert!(empty.validate().is_ok());
        assert_eq!(empty.content_state, PullFileContentState::Text);
        assert_eq!(empty.unified_text.as_deref(), Some(""));

        let binary = PullFileArtifact {
            source: Some(source(PullFileProvenance::LocalExactRange)),
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
            source: Some(source(PullFileProvenance::LocalExactRange)),
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
    }
}
