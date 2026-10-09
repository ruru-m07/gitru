//! Native GitHub PR file summaries. Patch presence is only a fetch hint.
//! https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files
use super::*;
use crate::providers::pull_files as common;
use serde_json::{Map, Value};

fn identity(request: &PullFileSourceRequest) -> Result<(String, u64), ProviderError> {
    common::validate_source_request(
        request,
        ProviderKind::Github,
        "github.com",
        PullFileSourceStrategy::GithubPullFiles,
    )?;
    let repository = common::positive(&request.repository.provider_id)?;
    let pull = common::positive(
        request
            .subject
            .number
            .as_deref()
            .ok_or_else(common::invalid)?,
    )?;
    common::positive(&request.account.actor_id)?;
    common::positive(&request.subject.provider_id)?;
    common::positive(&request.binding.context.source_repository_provider_id)?;
    if request.repository.id != format!("github:repository:{repository}")
        || request.subject.id != format!("github:pull:{}", request.subject.provider_id)
        || !valid_repository_path(&request.repository.full_name)
    {
        return Err(common::invalid());
    }
    Ok((format!("/repositories/{repository}/pulls/{pull}"), pull))
}

fn file(row: &Map<String, Value>) -> Result<ProviderPullFile, ProviderError> {
    let path = common::text(row.get("filename"), MAX_PULL_FILE_PATH_BYTES)?;
    let state = common::text(row.get("status"), MAX_PULL_FILE_NATIVE_STATE_BYTES)?;
    let kind = match state.as_str() {
        "added" => PullFileChangeKind::Added,
        "removed" => PullFileChangeKind::Deleted,
        "modified" | "changed" => PullFileChangeKind::Modified,
        "renamed" => PullFileChangeKind::Renamed,
        "copied" => PullFileChangeKind::Copied,
        _ => PullFileChangeKind::Unknown,
    };
    let identity = match kind {
        PullFileChangeKind::Added => PullFileIdentity {
            old_path: None,
            new_path: Some(path),
        },
        PullFileChangeKind::Deleted => PullFileIdentity {
            old_path: Some(path),
            new_path: None,
        },
        PullFileChangeKind::Renamed | PullFileChangeKind::Copied => PullFileIdentity {
            old_path: Some(common::text(
                row.get("previous_filename"),
                MAX_PULL_FILE_PATH_BYTES,
            )?),
            new_path: Some(path),
        },
        PullFileChangeKind::Modified => PullFileIdentity {
            old_path: Some(path.clone()),
            new_path: Some(path),
        },
        _ => PullFileIdentity {
            old_path: None,
            new_path: Some(path),
        },
    };
    let provider_file_id = match row.get("sha") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let oid = common::text(Some(value), 64)?;
            if !is_canonical_pull_file_oid(&oid) {
                return Err(common::invalid());
            }
            Some(oid)
        }
    };
    let file = ProviderPullFile {
        identity,
        provider_file_id,
        change_kind: kind,
        provider_change_kind: state,
        additions: common::count(row.get("additions"))?,
        deletions: common::count(row.get("deletions"))?,
        total_changes: common::count(row.get("changes"))?,
        old_mode: None,
        new_mode: None,
        mode_changed: PullFileFlag::Unknown,
        binary: PullFileFlag::Unknown,
        generated: PullFileFlag::Unknown,
        provider_collapsed: PullFileFlag::Unknown,
        provider_too_large: PullFileFlag::Unknown,
        diff_hint: common::patch_hint(row.get("patch"))?,
    };
    file.validate().map_err(|_| common::invalid())?;
    Ok(file)
}

impl GithubProvider {
    pub(super) async fn request_pull_files(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileProviderPage, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        let (parent, _) = identity(&PullFileSourceRequest::from(&request))?;
        let path = format!("{parent}/files");
        let initial = self
            .http
            .endpoint(&format!("{}?per_page=100", path.trim_start_matches('/')))?;
        let cursor = common::Cursor::open(&request, initial.to_string())?;
        let (url, page_number) = self.http.check_pull_file_url(&cursor.url, &path)?;
        if page_number != u64::from(request.lease.provider_page_count + 1) {
            return Err(common::invalid());
        }
        let current = common::fingerprint(&url);
        cursor.check_current(&current)?;
        let response = self
            .http
            .get_pull_file_collection(url, token, &path, page_number)
            .await?;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| common::invalid())?;
            if rows.len() > MAX_PULL_FILES_PER_PROVIDER_PAGE
                || response.next_url.is_some() && rows.is_empty()
            {
                return Err(common::invalid());
            }
            let files = rows.iter().map(file).collect::<Result<Vec<_>, _>>()?;
            let capped = request.start_position + files.len() as u32 == MAX_PULL_FILES;
            let cap = if capped {
                Some(PullFileCapEvidence {
                    provenance: PullFileCapProvenance::Provider,
                    reason: PullFileCapReason::ProviderFileLimit,
                    remote_has_more: if response.next_url.is_some() {
                        PullFileFlag::Known(true)
                    } else {
                        PullFileFlag::Unknown
                    },
                })
            } else {
                common::local_cap(&request, files.len(), response.next_url.is_some())
            };
            let next = if cap.is_some() {
                None
            } else {
                response
                    .next_url
                    .map(|next| {
                        let (url, _) = self.http.check_pull_file_url(&next, &path)?;
                        cursor.advance(files.len(), current, next, common::fingerprint(&url))
                    })
                    .transpose()?
            };
            common::page(&request, files, next, cap, response.cooldown_seconds)
        })();
        result.map_err(|error| common::quota(error, response.cooldown_seconds))
    }

    pub(super) async fn request_selected_pull_file(
        &self,
        token: &SecretToken,
        request: PullFileSelectedRequest,
    ) -> Result<PullFileArtifactRead, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        let (parent, _) = identity(&request.resource)?;
        let path = format!("{parent}/files");
        let ordinal = request.file.provider_position;
        let url = self.http.endpoint(&format!(
            "{}?per_page=1&page={}",
            path.trim_start_matches('/'),
            ordinal + 1
        ))?;
        let response = self
            .http
            .get_selected_pull_file(url, token, &path, ordinal)
            .await?;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| common::invalid())?;
            if rows.len() != 1 {
                return Err(common::invalid());
            }
            let observed = file(&rows[0])?;
            common::selected_content(
                &request,
                &observed,
                rows[0].get("patch"),
                response.cooldown_seconds,
            )
        })();
        result.map_err(|error| common::quota(error, response.cooldown_seconds))
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
        let (path, pull) = identity(&request)?;
        let response = self
            .http
            .get_point(self.http.endpoint(path.trim_start_matches('/'))?, token)
            .await?;
        let result = (|| {
            let json: Value =
                serde_json::from_slice(&response.body).map_err(|_| common::invalid())?;
            if json
                .get("id")
                .and_then(Value::as_u64)
                .map(|id| id.to_string())
                .as_deref()
                != Some(&request.subject.provider_id)
                || json.get("number").and_then(Value::as_u64) != Some(pull)
            {
                return Err(common::invalid());
            }
            let repository_id = |side: &str| {
                json.get(side)
                    .and_then(|v| v.get("repo"))
                    .and_then(|v| v.get("id"))
                    .and_then(Value::as_u64)
                    .filter(|v| *v > 0)
                    .map(|v| v.to_string())
                    .ok_or_else(common::invalid)
            };
            let validation = PullFileRangeValidation {
                merge_base_oid: None,
                base_oid: common::text(json.get("base").and_then(|v| v.get("sha")), 64)?,
                head_oid: common::text(json.get("head").and_then(|v| v.get("sha")), 64)?,
                base_repository_provider_id: repository_id("base")?,
                source_repository_provider_id: repository_id("head")?,
            };
            let count = match json.get("changed_files") {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    u32::try_from(v.as_u64().ok_or_else(common::invalid)?)
                        .map_err(|_| common::invalid())?,
                ),
            };
            let cap = count
                .filter(|n| *n >= MAX_PULL_FILES)
                .map(|n| PullFileCapEvidence {
                    provenance: PullFileCapProvenance::Provider,
                    reason: PullFileCapReason::ProviderFileLimit,
                    remote_has_more: PullFileFlag::Known(n > MAX_PULL_FILES),
                });
            common::validated_range(&request, validation, count, cap, response.cooldown_seconds)
        })();
        result.map_err(|error| common::quota(error, response.cooldown_seconds))
    }
}
