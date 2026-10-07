//! Exact-spec Bitbucket diffstat. `topic=true` means merge-base-to-head.
//! https://developer.atlassian.com/cloud/bitbucket/rest/api-group-commits/
use super::*;
use crate::providers::pull_files as common;
use serde::Deserialize;
use serde_json::{Map, Value};

fn identity(request: &PullFileCollectionRequest) -> Result<(String, u64), ProviderError> {
    common::validate_request(
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
        let (repository, _) = identity(&request)?;
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
