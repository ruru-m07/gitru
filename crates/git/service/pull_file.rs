//! Optional local-Git selected-file acceleration for provider pull requests.
//!
//! The caller supplies provider-validated exact tips and path identity. This
//! service performs no fetch and never treats the target tip as the comparison
//! base: it resolves one unique merge base, verifies the selected change, then
//! reads only that literal path pair.

use crate::{
    context::RepoContext,
    models::{
        pull_file::{
            LocalPullFileComparison, LocalPullFileDiff, LocalPullFileDiffProvenance,
            LocalPullFileDiffRequest, LocalPullFileDiffState,
            LocalPullFileDiffUnavailableReason as Unavailable,
            LocalPullFileDiffUnsupportedReason as Unsupported,
            MAX_LOCAL_PULL_FILE_COMBINED_BLOB_BYTES, MAX_LOCAL_PULL_FILE_INPUT_BLOB_BYTES,
            MAX_LOCAL_PULL_FILE_LINE_BYTES, MAX_LOCAL_PULL_FILE_PATH_BYTES,
            MAX_LOCAL_PULL_FILE_TEXT_BYTES, MAX_LOCAL_PULL_FILE_TEXT_LINES,
        },
        remotes::RemoteObservationError,
    },
    runner::GitCommandTransaction,
};
use std::{collections::HashSet, sync::Arc, time::Duration};

const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_OBJECT_TYPE_BYTES: usize = 16;
const MAX_OBJECT_SIZE_BYTES: usize = 32;
const MAX_MERGE_BASE_BYTES: usize = 8 * 1_024;
const MAX_CHANGE_METADATA_BYTES: usize = 64 * 1_024;
const MAX_SELECTED_ENTRY_BYTES: usize = MAX_LOCAL_PULL_FILE_PATH_BYTES + 256;

pub struct PullFileService {
    ctx: Arc<RepoContext>,
}

impl PullFileService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    /// Read one provider-selected file diff from already-present local objects.
    /// Invalid authority input is rejected before Git runs; operational refusal
    /// and bounded-content outcomes are returned as typed states.
    pub async fn selected_diff(
        &self,
        request: LocalPullFileDiffRequest,
    ) -> Result<LocalPullFileDiff, String> {
        validate_request(&request)?;
        match tokio::time::timeout(OPERATION_TIMEOUT, self.selected_diff_inner(&request)).await {
            Ok(result) => Ok(result),
            Err(_) => Ok(unavailable(Unavailable::TimedOut, None)),
        }
    }

    async fn selected_diff_inner(&self, request: &LocalPullFileDiffRequest) -> LocalPullFileDiff {
        let Ok(mut transaction) = self.ctx.runner.transaction().await else {
            return unavailable(Unavailable::GitUnavailable, None);
        };

        match has_exact_commit(&mut transaction, &request.base_oid).await {
            Ok(true) => {}
            Ok(false) => return unavailable(Unavailable::BaseObjectMissing, None),
            Err(reason) => return unavailable(reason, None),
        }
        match has_exact_commit(&mut transaction, &request.head_oid).await {
            Ok(true) => {}
            Ok(false) => return unavailable(Unavailable::HeadObjectMissing, None),
            Err(reason) => return unavailable(reason, None),
        }

        let resolved_merge_base = match resolve_merge_base(
            &mut transaction,
            &request.base_oid,
            &request.head_oid,
        )
        .await
        {
            Ok(ResolvedMergeBase::Unique(oid)) => oid,
            Ok(ResolvedMergeBase::None) => {
                return unsupported(Unsupported::NoCommonAncestor, None);
            }
            Ok(ResolvedMergeBase::Ambiguous) => {
                return unavailable(Unavailable::AmbiguousMergeBase, None);
            }
            Err(reason) => return unavailable(reason, None),
        };
        let provenance = LocalPullFileDiffProvenance {
            comparison: LocalPullFileComparison::MergeBaseToHead,
            base_oid: request.base_oid.clone(),
            head_oid: request.head_oid.clone(),
            resolved_merge_base_oid: resolved_merge_base.clone(),
            known_merge_base_oid: request.merge_base_oid.clone(),
        };
        if request
            .merge_base_oid
            .as_deref()
            .is_some_and(|known| known != resolved_merge_base)
        {
            return unavailable(Unavailable::KnownMergeBaseMismatch, Some(provenance));
        }

        match preflight_selected_blobs(
            &mut transaction,
            &resolved_merge_base,
            &request.head_oid,
            request,
        )
        .await
        {
            Ok(Preflight::Ready) => {}
            Ok(Preflight::Oversized) => {
                return LocalPullFileDiff {
                    provenance: Some(provenance),
                    state: LocalPullFileDiffState::Oversized,
                };
            }
            Ok(Preflight::Unsupported) => {
                return unsupported(Unsupported::UnsupportedChangeKind, Some(provenance));
            }
            Err(reason) => return unavailable(reason, Some(provenance)),
        }

        match verify_change_identity(
            &mut transaction,
            &resolved_merge_base,
            &request.head_oid,
            request,
        )
        .await
        {
            Ok(()) => {}
            Err(ChangeIdentityError::Unsupported) => {
                return unsupported(Unsupported::UnsupportedChangeKind, Some(provenance));
            }
            Err(ChangeIdentityError::Unavailable(reason)) => {
                return unavailable(reason, Some(provenance));
            }
        }

        match selected_change_is_binary(
            &mut transaction,
            &resolved_merge_base,
            &request.head_oid,
            request,
        )
        .await
        {
            Ok(true) => {
                return LocalPullFileDiff {
                    provenance: Some(provenance),
                    state: LocalPullFileDiffState::Binary,
                };
            }
            Ok(false) => {}
            Err(reason) => return unavailable(reason, Some(provenance)),
        }

        let args = diff_args("--patch", &resolved_merge_base, &request.head_oid, request);
        let (bytes, status) =
            match local_read(&mut transaction, &args, MAX_LOCAL_PULL_FILE_TEXT_BYTES).await {
                Ok(output) => output,
                Err(ReadFailure::LimitExceeded) => {
                    return LocalPullFileDiff {
                        provenance: Some(provenance),
                        state: LocalPullFileDiffState::Oversized,
                    };
                }
                Err(ReadFailure::Unavailable) => {
                    return unavailable(Unavailable::GitUnavailable, Some(provenance));
                }
            };
        if status != 0 {
            return unavailable(Unavailable::GitUnavailable, Some(provenance));
        }
        if !text_within_bounds(&bytes) {
            return LocalPullFileDiff {
                provenance: Some(provenance),
                state: LocalPullFileDiffState::Oversized,
            };
        }
        if bytes.contains(&0) {
            return LocalPullFileDiff {
                provenance: Some(provenance),
                state: LocalPullFileDiffState::Binary,
            };
        }
        let Ok(unified_diff) = String::from_utf8(bytes) else {
            return unsupported(Unsupported::NonUtf8Text, Some(provenance));
        };
        LocalPullFileDiff {
            provenance: Some(provenance),
            state: LocalPullFileDiffState::Text { unified_diff },
        }
    }
}

fn validate_request(request: &LocalPullFileDiffRequest) -> Result<(), String> {
    let valid_oid = |value: &str| {
        matches!(value.len(), 40 | 64)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    };
    if !valid_oid(&request.base_oid)
        || !valid_oid(&request.head_oid)
        || request.base_oid.len() != request.head_oid.len()
        || request
            .merge_base_oid
            .as_ref()
            .is_some_and(|oid| !valid_oid(oid) || oid.len() != request.base_oid.len())
        || (request.old_path.is_none() && request.new_path.is_none())
        || request
            .old_path
            .as_deref()
            .is_some_and(|path| !is_valid_path(path))
        || request
            .new_path
            .as_deref()
            .is_some_and(|path| !is_valid_path(path))
    {
        return Err("Invalid local pull file diff request".into());
    }
    Ok(())
}

fn is_valid_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_LOCAL_PULL_FILE_PATH_BYTES
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

async fn has_exact_commit(
    transaction: &mut GitCommandTransaction,
    oid: &str,
) -> Result<bool, Unavailable> {
    let args = local_command(["cat-file", "-t", oid]);
    let (bytes, status) = local_read(transaction, &args, MAX_OBJECT_TYPE_BYTES)
        .await
        .map_err(read_metadata_failure)?;
    if status == 0 {
        Ok(bytes == b"commit\n")
    } else if status == 128 {
        Ok(false)
    } else {
        Err(Unavailable::GitUnavailable)
    }
}

enum ResolvedMergeBase {
    Unique(String),
    None,
    Ambiguous,
}

async fn resolve_merge_base(
    transaction: &mut GitCommandTransaction,
    base_oid: &str,
    head_oid: &str,
) -> Result<ResolvedMergeBase, Unavailable> {
    let args = local_command(["merge-base", "--all", base_oid, head_oid]);
    let (bytes, status) = local_read(transaction, &args, MAX_MERGE_BASE_BYTES)
        .await
        .map_err(read_metadata_failure)?;
    if status == 1 && bytes.is_empty() {
        return Ok(ResolvedMergeBase::None);
    }
    if status != 0 {
        return Err(Unavailable::GitUnavailable);
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(Unavailable::MalformedGitOutput);
    }
    let mut bases = Vec::new();
    for line in bytes[..bytes.len() - 1].split(|byte| *byte == b'\n') {
        let oid = std::str::from_utf8(line).map_err(|_| Unavailable::MalformedGitOutput)?;
        if oid.len() != base_oid.len()
            || !matches!(oid.len(), 40 | 64)
            || !oid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(Unavailable::MalformedGitOutput);
        }
        bases.push(oid.to_owned());
    }
    match bases.len() {
        1 => Ok(ResolvedMergeBase::Unique(bases.remove(0))),
        2.. => Ok(ResolvedMergeBase::Ambiguous),
        _ => Err(Unavailable::MalformedGitOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preflight {
    Ready,
    Oversized,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedEntry {
    Blob {
        size: u64,
    },
    /// A gitlink is metadata-only for `--submodule=short`; its referenced
    /// commit does not need to exist in the selected repository.
    Gitlink,
    Unsupported,
}

/// Inspect exact tree entries before any rename, numstat, or patch workload.
/// `ls-tree` resolves the literal path without reading blob content, then
/// `cat-file -t/-s` verifies each blob is present and within the native budget.
async fn preflight_selected_blobs(
    transaction: &mut GitCommandTransaction,
    merge_base_oid: &str,
    head_oid: &str,
    request: &LocalPullFileDiffRequest,
) -> Result<Preflight, Unavailable> {
    let mut combined = 0_u64;
    for (commit_oid, path) in [
        (merge_base_oid, request.old_path.as_deref()),
        (head_oid, request.new_path.as_deref()),
    ] {
        let Some(path) = path else { continue };
        match selected_entry(transaction, commit_oid, path).await? {
            SelectedEntry::Blob { size } => {
                if size > MAX_LOCAL_PULL_FILE_INPUT_BLOB_BYTES {
                    return Ok(Preflight::Oversized);
                }
                combined = combined
                    .checked_add(size)
                    .ok_or(Unavailable::MetadataLimitExceeded)?;
                if combined > MAX_LOCAL_PULL_FILE_COMBINED_BLOB_BYTES {
                    return Ok(Preflight::Oversized);
                }
            }
            SelectedEntry::Gitlink => {}
            SelectedEntry::Unsupported => return Ok(Preflight::Unsupported),
        }
    }
    Ok(Preflight::Ready)
}

async fn selected_entry(
    transaction: &mut GitCommandTransaction,
    commit_oid: &str,
    path: &str,
) -> Result<SelectedEntry, Unavailable> {
    let args = local_command(["ls-tree", "--full-tree", "-z", commit_oid, "--", path]);
    let (bytes, status) = local_read(transaction, &args, MAX_SELECTED_ENTRY_BYTES)
        .await
        .map_err(read_metadata_failure)?;
    if status != 0 {
        return Err(Unavailable::GitUnavailable);
    }
    let entry = parse_selected_entry(&bytes, commit_oid.len(), path.as_bytes())?;
    match entry {
        TreeEntry::Blob { oid } => {
            let kind = local_command(["cat-file", "-t", oid.as_str()]);
            let (bytes, status) = local_read(transaction, &kind, MAX_OBJECT_TYPE_BYTES)
                .await
                .map_err(read_metadata_failure)?;
            if status == 128 {
                return Err(Unavailable::SelectedObjectMissing);
            }
            if status != 0 {
                return Err(Unavailable::GitUnavailable);
            }
            if bytes != b"blob\n" {
                return Err(Unavailable::MalformedGitOutput);
            }

            let size = local_command(["cat-file", "-s", oid.as_str()]);
            let (bytes, status) = local_read(transaction, &size, MAX_OBJECT_SIZE_BYTES)
                .await
                .map_err(read_metadata_failure)?;
            if status == 128 {
                return Err(Unavailable::SelectedObjectMissing);
            }
            if status != 0 {
                return Err(Unavailable::GitUnavailable);
            }
            Ok(SelectedEntry::Blob {
                size: parse_object_size(&bytes)?,
            })
        }
        TreeEntry::Gitlink => Ok(SelectedEntry::Gitlink),
        TreeEntry::Unsupported => Ok(SelectedEntry::Unsupported),
    }
}

enum TreeEntry {
    Blob { oid: String },
    Gitlink,
    Unsupported,
}

fn parse_selected_entry(
    bytes: &[u8],
    oid_length: usize,
    expected_path: &[u8],
) -> Result<TreeEntry, Unavailable> {
    if bytes.is_empty() {
        return Err(Unavailable::ChangeIdentityMismatch);
    }
    if !bytes.ends_with(b"\0") || bytes[..bytes.len() - 1].contains(&0) {
        return Err(Unavailable::MalformedGitOutput);
    }
    let record = &bytes[..bytes.len() - 1];
    let separator = record
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or(Unavailable::MalformedGitOutput)?;
    let (metadata, path_with_separator) = record.split_at(separator);
    let path = &path_with_separator[1..];
    if path != expected_path {
        return Err(Unavailable::ChangeIdentityMismatch);
    }
    let mut fields = metadata.split(|byte| *byte == b' ');
    let mode = fields.next().ok_or(Unavailable::MalformedGitOutput)?;
    let kind = fields.next().ok_or(Unavailable::MalformedGitOutput)?;
    let oid = fields.next().ok_or(Unavailable::MalformedGitOutput)?;
    if fields.next().is_some()
        || oid.len() != oid_length
        || !matches!(oid.len(), 40 | 64)
        || !oid
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(Unavailable::MalformedGitOutput);
    }
    match (mode, kind) {
        (b"100644" | b"100755" | b"120000", b"blob") => Ok(TreeEntry::Blob {
            oid: std::str::from_utf8(oid)
                .map_err(|_| Unavailable::MalformedGitOutput)?
                .to_owned(),
        }),
        (b"160000", b"commit") => Ok(TreeEntry::Gitlink),
        _ => Ok(TreeEntry::Unsupported),
    }
}

fn parse_object_size(bytes: &[u8]) -> Result<u64, Unavailable> {
    let digits = bytes
        .strip_suffix(b"\n")
        .ok_or(Unavailable::MalformedGitOutput)?;
    if digits.is_empty()
        || !digits.iter().all(u8::is_ascii_digit)
        || digits.len() > 20
        || (digits.len() > 1 && digits[0] == b'0')
    {
        return Err(Unavailable::MalformedGitOutput);
    }
    std::str::from_utf8(digits)
        .map_err(|_| Unavailable::MalformedGitOutput)?
        .parse()
        .map_err(|_| Unavailable::MalformedGitOutput)
}

#[derive(Debug)]
enum ChangeIdentityError {
    Unsupported,
    Unavailable(Unavailable),
}

async fn verify_change_identity(
    transaction: &mut GitCommandTransaction,
    merge_base_oid: &str,
    head_oid: &str,
    request: &LocalPullFileDiffRequest,
) -> Result<(), ChangeIdentityError> {
    let args = diff_args("--name-status", merge_base_oid, head_oid, request);
    let (bytes, status) = local_read(transaction, &args, MAX_CHANGE_METADATA_BYTES)
        .await
        .map_err(|error| ChangeIdentityError::Unavailable(read_metadata_failure(error)))?;
    if status != 0 {
        return Err(ChangeIdentityError::Unavailable(
            Unavailable::GitUnavailable,
        ));
    }
    let records = parse_name_status(&bytes)?;
    if records.len() != 1 {
        return Err(ChangeIdentityError::Unavailable(
            Unavailable::ChangeIdentityMismatch,
        ));
    }
    let expected = (
        request.old_path.as_deref().map(str::as_bytes),
        request.new_path.as_deref().map(str::as_bytes),
    );
    let actual = records[0].as_ref_pair();
    if actual != expected {
        return Err(ChangeIdentityError::Unavailable(
            Unavailable::ChangeIdentityMismatch,
        ));
    }
    Ok(())
}

struct ParsedChange {
    old_path: Option<Vec<u8>>,
    new_path: Option<Vec<u8>>,
}

impl ParsedChange {
    fn as_ref_pair(&self) -> (Option<&[u8]>, Option<&[u8]>) {
        (self.old_path.as_deref(), self.new_path.as_deref())
    }
}

fn parse_name_status(bytes: &[u8]) -> Result<Vec<ParsedChange>, ChangeIdentityError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.ends_with(b"\0") {
        return Err(ChangeIdentityError::Unavailable(
            Unavailable::MalformedGitOutput,
        ));
    }
    let tokens: Vec<&[u8]> = bytes[..bytes.len() - 1].split(|byte| *byte == 0).collect();
    if tokens.iter().any(|token| token.is_empty()) {
        return Err(ChangeIdentityError::Unavailable(
            Unavailable::MalformedGitOutput,
        ));
    }
    let mut records = Vec::new();
    let mut cursor = 0;
    while cursor < tokens.len() {
        let status = tokens[cursor];
        cursor += 1;
        let single = |cursor: &mut usize| -> Result<Vec<u8>, ChangeIdentityError> {
            let path = tokens.get(*cursor).ok_or(ChangeIdentityError::Unavailable(
                Unavailable::MalformedGitOutput,
            ))?;
            *cursor += 1;
            Ok(path.to_vec())
        };
        let record = match status {
            b"A" => ParsedChange {
                old_path: None,
                new_path: Some(single(&mut cursor)?),
            },
            b"D" => ParsedChange {
                old_path: Some(single(&mut cursor)?),
                new_path: None,
            },
            b"M" | b"T" => {
                let path = single(&mut cursor)?;
                ParsedChange {
                    old_path: Some(path.clone()),
                    new_path: Some(path),
                }
            }
            _ if valid_similarity_status(status, b'R') || valid_similarity_status(status, b'C') => {
                ParsedChange {
                    old_path: Some(single(&mut cursor)?),
                    new_path: Some(single(&mut cursor)?),
                }
            }
            _ => return Err(ChangeIdentityError::Unsupported),
        };
        records.push(record);
    }
    Ok(records)
}

fn valid_similarity_status(value: &[u8], prefix: u8) -> bool {
    value.first() == Some(&prefix)
        && matches!(value.len(), 2..=4)
        && value[1..].iter().all(u8::is_ascii_digit)
        && std::str::from_utf8(&value[1..])
            .ok()
            .and_then(|score| score.parse::<u16>().ok())
            .is_some_and(|score| score <= 100)
}

async fn selected_change_is_binary(
    transaction: &mut GitCommandTransaction,
    merge_base_oid: &str,
    head_oid: &str,
    request: &LocalPullFileDiffRequest,
) -> Result<bool, Unavailable> {
    let args = diff_args("--numstat", merge_base_oid, head_oid, request);
    let (bytes, status) = local_read(transaction, &args, MAX_CHANGE_METADATA_BYTES)
        .await
        .map_err(read_metadata_failure)?;
    if status != 0 {
        return Err(Unavailable::GitUnavailable);
    }
    parse_numstat(&bytes, request)
}

fn parse_numstat(bytes: &[u8], request: &LocalPullFileDiffRequest) -> Result<bool, Unavailable> {
    if bytes.is_empty() || !bytes.ends_with(b"\0") {
        return Err(Unavailable::MalformedGitOutput);
    }
    let expected: HashSet<&[u8]> = request
        .old_path
        .iter()
        .chain(request.new_path.iter())
        .map(|path| path.as_bytes())
        .collect();
    let mut binary = false;
    for record in bytes[..bytes.len() - 1].split(|byte| *byte == 0) {
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let additions = fields.next().ok_or(Unavailable::MalformedGitOutput)?;
        let deletions = fields.next().ok_or(Unavailable::MalformedGitOutput)?;
        let path = fields.next().ok_or(Unavailable::MalformedGitOutput)?;
        if path.is_empty()
            || !expected.contains(path)
            || !valid_numstat_count(additions)
            || !valid_numstat_count(deletions)
            || ((additions == b"-") != (deletions == b"-"))
        {
            return Err(Unavailable::MalformedGitOutput);
        }
        binary |= additions == b"-" && deletions == b"-";
    }
    Ok(binary)
}

fn valid_numstat_count(value: &[u8]) -> bool {
    value == b"-" || (!value.is_empty() && value.iter().all(u8::is_ascii_digit))
}

fn diff_args(
    mode: &str,
    merge_base_oid: &str,
    head_oid: &str,
    request: &LocalPullFileDiffRequest,
) -> Vec<String> {
    let mut args = local_command(["diff", mode, "--no-ext-diff", "--no-textconv", "--no-color"]);
    if mode == "--name-status" || mode == "--numstat" {
        args.push("-z".into());
    }
    if mode == "--name-status" || mode == "--patch" {
        args.push("--find-renames=50%".into());
        args.push("--find-copies=50%".into());
        args.push("--find-copies-harder".into());
    }
    if mode == "--numstat" {
        args.push("--no-renames".into());
    }
    if mode == "--patch" {
        args.push("--full-index".into());
        args.push("--submodule=short".into());
    }
    args.push(merge_base_oid.into());
    args.push(head_oid.into());
    args.push("--".into());
    if let Some(path) = &request.old_path {
        args.push(path.clone());
    }
    if request.new_path.as_ref() != request.old_path.as_ref()
        && let Some(path) = &request.new_path
    {
        args.push(path.clone());
    }
    args
}

fn local_command<const N: usize>(tail: [&str; N]) -> Vec<String> {
    let mut args = vec![
        "--no-pager".into(),
        "--literal-pathspecs".into(),
        "-c".into(),
        "core.hooksPath=".into(),
        "-c".into(),
        "protocol.allow=never".into(),
    ];
    args.extend(tail.into_iter().map(str::to_owned));
    args
}

enum ReadFailure {
    LimitExceeded,
    Unavailable,
}

async fn local_read(
    transaction: &mut GitCommandTransaction,
    args: &[String],
    maximum: usize,
) -> Result<(Vec<u8>, i32), ReadFailure> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    transaction
        .sensitive_local_read(&refs, maximum)
        .await
        .map_err(|error| match error {
            RemoteObservationError::LimitExceeded => ReadFailure::LimitExceeded,
            _ => ReadFailure::Unavailable,
        })
}

fn read_metadata_failure(failure: ReadFailure) -> Unavailable {
    match failure {
        ReadFailure::LimitExceeded => Unavailable::MetadataLimitExceeded,
        ReadFailure::Unavailable => Unavailable::GitUnavailable,
    }
}

fn text_within_bounds(bytes: &[u8]) -> bool {
    if bytes.len() > MAX_LOCAL_PULL_FILE_TEXT_BYTES {
        return false;
    }
    let mut lines = 1;
    let mut line_bytes = 0;
    for byte in bytes {
        if *byte == b'\n' {
            lines += 1;
            line_bytes = 0;
            if lines > MAX_LOCAL_PULL_FILE_TEXT_LINES {
                return false;
            }
        } else {
            line_bytes += 1;
            if line_bytes > MAX_LOCAL_PULL_FILE_LINE_BYTES {
                return false;
            }
        }
    }
    true
}

fn unavailable(
    reason: Unavailable,
    provenance: Option<LocalPullFileDiffProvenance>,
) -> LocalPullFileDiff {
    LocalPullFileDiff {
        provenance,
        state: LocalPullFileDiffState::Unavailable { reason },
    }
}

fn unsupported(
    reason: Unsupported,
    provenance: Option<LocalPullFileDiffProvenance>,
) -> LocalPullFileDiff {
    LocalPullFileDiff {
        provenance,
        state: LocalPullFileDiffState::Unsupported { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_noncanonical_authority_and_unsafe_paths() {
        let oid = "a".repeat(40);
        for path in ["", "/root", "../escape", "a//b", "C:/root", "a\\..\\b"] {
            let request = LocalPullFileDiffRequest {
                base_oid: oid.clone(),
                head_oid: oid.clone(),
                merge_base_oid: None,
                old_path: Some(path.into()),
                new_path: Some(path.into()),
            };
            assert!(validate_request(&request).is_err(), "accepted {path:?}");
        }
        let request = LocalPullFileDiffRequest {
            base_oid: "A".repeat(40),
            head_oid: oid,
            merge_base_oid: None,
            old_path: Some("safe".into()),
            new_path: Some("safe".into()),
        };
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn parses_exact_nul_name_status_records() {
        let records = parse_name_status(b"R100\0old\0new\0").expect("rename");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].as_ref_pair(),
            (Some(b"old".as_slice()), Some(b"new".as_slice()))
        );
        assert!(parse_name_status(b"R100\0old\0").is_err());
        assert!(parse_name_status(b"M\0path").is_err());
    }

    #[test]
    fn enforces_patch_line_limits() {
        assert!(text_within_bounds(b"line\nline\n"));
        let exact_line = [vec![b'x'; MAX_LOCAL_PULL_FILE_LINE_BYTES], vec![b'\n']].concat();
        assert!(text_within_bounds(&exact_line));
        assert!(!text_within_bounds(&vec![
            b'x';
            MAX_LOCAL_PULL_FILE_LINE_BYTES + 1
        ]));
        assert!(text_within_bounds(
            "\n".repeat(MAX_LOCAL_PULL_FILE_TEXT_LINES - 1).as_bytes()
        ));
        let too_many = "\n".repeat(MAX_LOCAL_PULL_FILE_TEXT_LINES);
        assert!(!text_within_bounds(too_many.as_bytes()));
    }
}
