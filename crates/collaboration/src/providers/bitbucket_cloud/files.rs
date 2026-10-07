//! Exact-spec Bitbucket diffstat. `topic=true` means merge-base-to-head.
//! https://developer.atlassian.com/cloud/bitbucket/rest/api-group-commits/
use super::*;
use crate::providers::pull_files as common;
use serde::Deserialize;
use serde_json::{Map, Value};

fn identity(request: &PullFileSourceRequest) -> Result<(String, u64), ProviderError> {
    common::validate_source_request(
        request,
        ProviderKind::BitbucketCloud,
        "bitbucket.org",
        PullFileSourceStrategy::BitbucketCloudDiffstat,
    )?;
    let repository = resource_details::repository_identity(&request.account, &request.repository)?;
    let source = canonical_uuid(&request.binding.context.source_repository_provider_id)?;
    let pull = common::positive(request.subject.number.as_deref().ok_or_else(invalid)?)?;
    if pull > i64::MAX as u64
        || source != request.binding.context.source_repository_provider_id
        || request.subject.provider_id != format!("{repository}:{pull}")
        || request.subject.id != format!("bitbucket_cloud:pull:{repository}:{pull}")
    {
        return Err(invalid());
    }
    Ok((repository, pull))
}
fn path(value: Option<&Value>) -> Result<Option<String>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let object = value.as_object().ok_or_else(invalid)?;
            if object
                .get("type")
                .is_some_and(|v| v.as_str() != Some("commit_file"))
            {
                return Err(invalid());
            }
            Ok(Some(common::text(
                object.get("path"),
                MAX_PULL_FILE_PATH_BYTES,
            )?))
        }
    }
}
fn file(row: &Map<String, Value>) -> Result<ProviderPullFile, ProviderError> {
    if row
        .get("type")
        .is_some_and(|v| v.as_str() != Some("diffstat"))
    {
        return Err(invalid());
    }
    let native = common::text(row.get("status"), MAX_PULL_FILE_NATIVE_STATE_BYTES)?;
    let kind = match native.as_str() {
        "added" => PullFileChangeKind::Added,
        "removed" => PullFileChangeKind::Deleted,
        "modified" => PullFileChangeKind::Modified,
        "renamed" => PullFileChangeKind::Renamed,
        "copied" => PullFileChangeKind::Copied,
        _ => PullFileChangeKind::Unknown,
    };
    let additions = common::count(row.get("lines_added"))?;
    let deletions = common::count(row.get("lines_removed"))?;
    let total_changes = match (&additions, &deletions) {
        (PullFileCount::Known(a), PullFileCount::Known(d)) => PullFileCount::Known(
            a.parse::<u64>()
                .map_err(|_| invalid())?
                .checked_add(d.parse::<u64>().map_err(|_| invalid())?)
                .ok_or_else(invalid)?
                .to_string(),
        ),
        _ => PullFileCount::Unknown,
    };
    let file = ProviderPullFile {
        identity: PullFileIdentity {
            old_path: path(row.get("old"))?,
            new_path: path(row.get("new"))?,
        },
        provider_file_id: None,
        change_kind: kind,
        provider_change_kind: native,
        additions,
        deletions,
        total_changes,
        old_mode: None,
        new_mode: None,
        mode_changed: PullFileFlag::Unknown,
        binary: PullFileFlag::Unknown,
        generated: PullFileFlag::Unknown,
        provider_collapsed: PullFileFlag::Unknown,
        provider_too_large: PullFileFlag::Unknown,
        diff_hint: PullFileDiffHint::Unknown,
    };
    file.validate().map_err(|_| invalid())?;
    Ok(file)
}
#[derive(Deserialize)]
struct Collection {
    values: Vec<Map<String, Value>>,
    next: Option<String>,
    size: Option<u64>,
}
impl BitbucketCloudProvider {
    pub(super) async fn request_pull_files(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileProviderPage, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        let (repository, _) = identity(&PullFileSourceRequest::from(&request))?;
        let route = Route::PullFiles {
            repository,
            base: request.binding.context.base_oid.clone(),
            head: request.binding.context.head_oid.clone(),
        };
        let initial = self.http.endpoint(&route)?;
        let mut cursor = common::Cursor::open(&request, initial.to_string())?;
        let url = if request.cursor.is_some() {
            self.http.continuation(&cursor.url, &route)?
        } else {
            initial
        };
        let current = self.http.fingerprint(url.as_str(), &route)?;
        cursor.check_current(&current)?;
        let response = self.http.get(url, &route, token).await?;
        let result = (|| {
            let collection: Collection =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if collection.values.len() > MAX_PULL_FILES_PER_PROVIDER_PAGE
                || collection.next.is_some() && collection.values.is_empty()
            {
                return Err(invalid());
            }
            let files = collection
                .values
                .iter()
                .map(file)
                .collect::<Result<Vec<_>, _>>()?;
            // Validate continuation authority even when a local cap will stop
            // this traversal. A malformed link is not evidence of more rows.
            let validated_next = collection
                .next
                .as_ref()
                .map(|next| {
                    let url = self.http.continuation(next, &route)?;
                    let fingerprint = self.http.fingerprint(url.as_str(), &route)?;
                    cursor.check_current(&fingerprint)?;
                    if fingerprint == current {
                        return Err(invalid());
                    }
                    Ok((url.to_string(), fingerprint))
                })
                .transpose()?;
            let total = u64::from(request.start_position) + files.len() as u64;
            let expected_total = cursor.observe_total(collection.size)?;
            if expected_total.is_some_and(|n| n < total || collection.next.is_some() && n <= total)
            {
                return Err(invalid());
            }
            let cap = if collection.next.is_none() && expected_total.is_some_and(|n| n > total) {
                Some(PullFileCapEvidence {
                    provenance: PullFileCapProvenance::Provider,
                    reason: PullFileCapReason::ProviderOverflow,
                    remote_has_more: PullFileFlag::Known(true),
                })
            } else {
                common::local_cap(&request, files.len(), collection.next.is_some())
            };
            let next = if cap.is_some() {
                None
            } else {
                validated_next
                    .map(|(url, fingerprint)| {
                        cursor.advance(files.len(), current, url, fingerprint)
                    })
                    .transpose()?
            };
            common::page(&request, files, next, cap, response.cooldown)
        })();
        result.map_err(|e| quota(e, response.cooldown))
    }
    pub(super) async fn request_pull_file_range(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileRangeValidationResult, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        self.request_file_source_range(token, PullFileSourceRequest::from(&request))
            .await
    }
    pub(super) async fn request_file_source_range(
        &self,
        token: &SecretToken,
        request: PullFileSourceRequest,
    ) -> Result<PullFileRangeValidationResult, ProviderError> {
        let (repository, pull) = identity(&request)?;
        let route = Route::PullRequest(repository.clone(), pull);
        let response = self
            .http
            .get(self.http.endpoint(&route)?, &route, token)
            .await?;
        let result = (|| {
            let json: Map<String, Value> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if resource_details::identity(&json, &repository)? != pull {
                return Err(invalid());
            }
            let side = |name: &str| -> Result<(String, String), ProviderError> {
                let value = json
                    .get(name)
                    .and_then(Value::as_object)
                    .ok_or_else(invalid)?;
                let oid = common::text(value.get("commit").and_then(|v| v.get("hash")), 64)?;
                let repository = canonical_uuid(
                    value
                        .get("repository")
                        .and_then(|v| v.get("uuid"))
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?,
                )?;
                Ok((oid, repository))
            };
            let (base_oid, base_repository_provider_id) = side("destination")?;
            let (head_oid, source_repository_provider_id) = side("source")?;
            common::validated_range(
                &request,
                PullFileRangeValidation {
                    base_oid,
                    head_oid,
                    base_repository_provider_id,
                    source_repository_provider_id,
                    merge_base_oid: None,
                },
                None,
                None,
                response.cooldown,
            )
        })();
        result.map_err(|e| quota(e, response.cooldown))
    }
}

impl BitbucketCloudProvider {
    pub(super) async fn request_selected_pull_file(
        &self,
        token: &SecretToken,
        request: PullFileSelectedRequest,
    ) -> Result<PullFileArtifactRead, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        let (repository, _) = identity(&request.resource)?;
        let context = &request.resource.binding.context;
        let selected = &request.membership.identity;
        let path = selected
            .new_path
            .as_ref()
            .or(selected.old_path.as_ref())
            .ok_or_else(common::invalid)?;
        let response = self
            .http
            .selected_diff(
                &repository,
                &context.base_oid,
                &context.head_oid,
                path,
                token,
            )
            .await?;
        let (content_state, unified_text, binary_hint) = match response.text {
            Err(error) if error.is_oversized() => (
                PullFileContentState::Oversized,
                None,
                request.file.file.binary,
            ),
            Err(_) => return Err(common::quota(common::invalid(), response.cooldown)),
            Ok(text) if text.is_empty() => (
                PullFileContentState::Omitted,
                None,
                request.file.file.binary,
            ),
            Ok(text) => {
                validate_selected_diff(&text, selected)
                    .map_err(|error| common::quota(error, response.cooldown))?;
                if text
                    .lines()
                    .any(|line| line.starts_with("Binary files ") || line == "GIT binary patch")
                {
                    (
                        PullFileContentState::Omitted,
                        None,
                        PullFileFlag::Known(true),
                    )
                } else {
                    (
                        PullFileContentState::Text,
                        Some(text),
                        request.file.file.binary,
                    )
                }
            }
        };
        Ok(PullFileArtifactRead {
            content_state,
            unified_text,
            binary_hint,
            cooldown_seconds: response.cooldown,
        })
    }
}

/// Path filtering can include a directory prefix. A successful text artifact
/// must contain exactly one diff section whose old/new paths match membership.
fn validate_selected_diff(text: &str, identity: &PullFileIdentity) -> Result<(), ProviderError> {
    let old = identity
        .old_path
        .as_ref()
        .or(identity.new_path.as_ref())
        .ok_or_else(common::invalid)?;
    let new = identity
        .new_path
        .as_ref()
        .or(identity.old_path.as_ref())
        .ok_or_else(common::invalid)?;
    let mut section = false;
    let mut hunk = false;
    let mut sides = [false; 2];
    let mut moves = [false; 2];
    let mut move_kind = None;
    let mut modes = [false; 2];
    let mut binary = false;
    for line in text.lines() {
        if let Some(paths) = line.strip_prefix("diff --git ") {
            if section {
                return Err(common::invalid());
            }
            section = true;
            if paths != format!("a/{old} b/{new}") {
                let (left, remainder) = header_path(paths)?;
                let (right, rest) =
                    header_path(remainder.strip_prefix(' ').ok_or_else(common::invalid)?)?;
                if !rest.is_empty() || left != format!("a/{old}") || right != format!("b/{new}") {
                    return Err(common::invalid());
                }
            }
            continue;
        }
        if !section {
            return Err(common::invalid());
        }
        if line.starts_with("@@") {
            if !sides.into_iter().all(|seen| seen) {
                return Err(common::invalid());
            }
            hunk = true;
            continue;
        }
        if hunk {
            continue;
        }
        for (index, prefix, path, side) in [
            (0, "--- ", identity.old_path.as_ref(), "a"),
            (1, "+++ ", identity.new_path.as_ref(), "b"),
        ] {
            if let Some(raw) = line.strip_prefix(prefix) {
                let actual = entire_path(raw)?;
                let expected =
                    path.map_or_else(|| "/dev/null".into(), |path| format!("{side}/{path}"));
                if sides[index] || actual != expected {
                    return Err(common::invalid());
                }
                sides[index] = true;
            }
        }
        for (kind, prefix, index, path) in [
            ("rename", "rename from ", 0, old),
            ("rename", "rename to ", 1, new),
            ("copy", "copy from ", 0, old),
            ("copy", "copy to ", 1, new),
        ] {
            if let Some(raw) = line.strip_prefix(prefix) {
                if moves[index]
                    || move_kind.is_some_and(|previous| previous != kind)
                    || entire_path(raw)? != *path
                {
                    return Err(common::invalid());
                }
                moves[index] = true;
                move_kind = Some(kind);
            }
        }
        for (index, prefix) in [(0, "old mode "), (1, "new mode ")] {
            if let Some(mode) = line.strip_prefix(prefix) {
                if modes[index]
                    || mode.len() != 6
                    || !mode.bytes().all(|byte| (b'0'..=b'7').contains(&byte))
                {
                    return Err(common::invalid());
                }
                modes[index] = true;
            }
        }
        binary |= line.starts_with("Binary files ") || line == "GIT binary patch";
    }
    let independent_paths =
        sides.into_iter().all(|seen| seen) || moves.into_iter().all(|seen| seen);
    let only_mode =
        old == new && modes.into_iter().all(|seen| seen) && !hunk && move_kind.is_none();
    // Binary content is never retained; this permits the standard header-only
    // omission marker without asserting an ambiguous textual file pairing.
    if section && (independent_paths || only_mode || binary && !hunk) {
        Ok(())
    } else {
        Err(common::invalid())
    }
}
fn entire_path(value: &str) -> Result<String, ProviderError> {
    if value.starts_with('"') {
        let (path, remainder) = quoted_path(value)?;
        if !remainder.is_empty() {
            return Err(common::invalid());
        }
        Ok(path)
    } else {
        Ok(value.into())
    }
}
fn header_path(value: &str) -> Result<(String, &str), ProviderError> {
    if value.starts_with('"') {
        quoted_path(value)
    } else if let Some((path, remainder)) = value.split_once(' ') {
        Ok((path.into(), &value[value.len() - remainder.len() - 1..]))
    } else {
        Ok((value.into(), ""))
    }
}
fn quoted_path(value: &str) -> Result<(String, &str), ProviderError> {
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'"') {
        return Err(common::invalid());
    }
    let mut out = Vec::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                return Ok((
                    String::from_utf8(out).map_err(|_| common::invalid())?,
                    &value[i + 1..],
                ));
            }
            b'\\' => {
                i += 1;
                let byte = *bytes.get(i).ok_or_else(common::invalid)?;
                match byte {
                    b'"' | b'\\' => out.push(byte),
                    b'0'..=b'3' => {
                        let second = *bytes
                            .get(i + 1)
                            .filter(|b| (b'0'..=b'7').contains(b))
                            .ok_or_else(common::invalid)?;
                        let third = *bytes
                            .get(i + 2)
                            .filter(|b| (b'0'..=b'7').contains(b))
                            .ok_or_else(common::invalid)?;
                        out.push((byte - b'0') * 64 + (second - b'0') * 8 + third - b'0');
                        i += 2;
                    }
                    _ => return Err(common::invalid()),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    Err(common::invalid())
}

#[cfg(test)]
mod selected_parser_tests {
    use super::*;
    fn identity(old: &str, new: &str) -> PullFileIdentity {
        PullFileIdentity {
            old_path: Some(old.into()),
            new_path: Some(new.into()),
        }
    }
    #[test]
    fn independent_rename_paths_resolve_ambiguous_combined_header() {
        let one = identity("foo b/bar", "baz");
        let two = identity("foo", "bar b/baz");
        let ambiguous = "diff --git a/foo b/bar b/baz\n";
        assert!(validate_selected_diff(ambiguous, &one).is_err());
        assert!(validate_selected_diff(ambiguous, &two).is_err());
        let resolved =
            format!("{ambiguous}similarity index 100%\nrename from foo b/bar\nrename to baz\n");
        assert!(validate_selected_diff(&resolved, &one).is_ok());
        assert!(validate_selected_diff(&resolved, &two).is_err());
    }
    #[test]
    fn whitespace_quoted_octal_mixed_headers_and_mode_only_are_bound() {
        let selected = identity("with space", "new space");
        let text = "diff --git a/with space b/new space\n--- a/with space\n+++ b/new space\n@@ -1 +1 @@\n-a\n+b\n";
        assert!(validate_selected_diff(text, &selected).is_ok());
        let unicode = identity("é", "new");
        let quoted = r#"diff --git "a/\303\251" b/new
--- "a/\303\251"
+++ b/new
@@ -1 +1 @@
-a
+b
"#;
        assert!(validate_selected_diff(quoted, &unicode).is_ok());
        let mode = "diff --git a/with space b/with space\nold mode 100644\nnew mode 100755\n";
        assert!(validate_selected_diff(mode, &identity("with space", "with space")).is_ok());
    }
    #[test]
    fn hunk_before_header_repeated_sides_and_second_sections_fail() {
        let selected = identity("a", "a");
        for text in [
            "@@ -1 +1 @@\n-a\n+b\n",
            "diff --git a/a b/a\n@@ -1 +1 @@\n-a\n+b\n",
            "diff --git a/a b/a\n--- a/a\n--- a/a\n+++ b/a\n",
            "diff --git a/a b/a\n--- a/a\n+++ b/a\ndiff --git a/a b/a\n",
        ] {
            assert!(validate_selected_diff(text, &selected).is_err());
        }
    }
}
