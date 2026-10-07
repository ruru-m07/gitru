//! Bounded GitLab merge-request diffs with separately validated parent refs.
//! https://docs.gitlab.com/api/merge_requests/#list-merge-request-diffs
use super::*;
use crate::providers::pull_files as common;
use serde_json::{Map, Value};

fn identity(request: &PullFileSourceRequest) -> Result<(u64, u64), ProviderError> {
    common::validate_source_request(
        request,
        ProviderKind::Gitlab,
        "gitlab.com",
        PullFileSourceStrategy::GitlabMergeRequestDiffs,
    )?;
    let project = resource_details::repository_identity(&request.account, &request.repository)?;
    let iid = common::positive(request.subject.number.as_deref().ok_or_else(invalid)?)?;
    common::positive(&request.account.actor_id)?;
    common::positive(&request.binding.context.source_repository_provider_id)?;
    let native = common::positive(&request.subject.provider_id)?;
    if request.subject.id != format!("gitlab:pull:{native}")
        || request.repository.id != format!("gitlab:repository:{project}")
    {
        return Err(invalid());
    }
    Ok((project, iid))
}
fn async_oid(value: Option<&Value>) -> Result<String, ProviderError> {
    match value {
        None | Some(Value::Null) => Err(ProviderError::new(ProviderErrorKind::Unavailable)),
        Some(Value::String(value)) if value.is_empty() => {
            Err(ProviderError::new(ProviderErrorKind::Unavailable))
        }
        Some(value) => {
            let oid = common::text(Some(value), 64)?;
            if !is_canonical_pull_file_oid(&oid) {
                return Err(invalid());
            }
            Ok(oid)
        }
    }
}
fn mode(value: Option<&Value>) -> Result<Option<String>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value))
            if value.len() == 6 && value.bytes().all(|b| (b'0'..=b'7').contains(&b)) =>
        {
            Ok(Some(value.clone()))
        }
        _ => Err(invalid()),
    }
}
fn file(row: &Map<String, Value>) -> Result<ProviderPullFile, ProviderError> {
    let new = common::flag(row.get("new_file"))?;
    let deleted = common::flag(row.get("deleted_file"))?;
    let renamed = common::flag(row.get("renamed_file"))?;
    let true_count = [new, deleted, renamed]
        .iter()
        .filter(|f| **f == PullFileFlag::Known(true))
        .count();
    if true_count > 1 {
        return Err(invalid());
    }
    let a_mode = mode(row.get("a_mode"))?;
    let b_mode = mode(row.get("b_mode"))?;
    let mode_changed = match (&a_mode, &b_mode) {
        (Some(a), Some(b)) => PullFileFlag::Known(a != b),
        _ => PullFileFlag::Unknown,
    };
    let (kind, native) = if new == PullFileFlag::Known(true) {
        (PullFileChangeKind::Added, "new_file")
    } else if deleted == PullFileFlag::Known(true) {
        (PullFileChangeKind::Deleted, "deleted_file")
    } else if renamed == PullFileFlag::Known(true) {
        (PullFileChangeKind::Renamed, "renamed_file")
    } else if [new, deleted, renamed]
        .iter()
        .all(|v| *v == PullFileFlag::Known(false))
    {
        (PullFileChangeKind::Modified, "modified")
    } else {
        (PullFileChangeKind::Unknown, "unknown")
    };
    let old_path = common::text(row.get("old_path"), MAX_PULL_FILE_PATH_BYTES)?;
    let new_path = common::text(row.get("new_path"), MAX_PULL_FILE_PATH_BYTES)?;
    // GitLab repeats paths on add/delete; preserve semantic side absence.
    let identity = PullFileIdentity {
        old_path: (kind != PullFileChangeKind::Added).then_some(old_path),
        new_path: (kind != PullFileChangeKind::Deleted).then_some(new_path),
    };
    let collapsed = common::flag(row.get("collapsed"))?;
    let too_large = common::flag(row.get("too_large"))?;
    let observed_hint = common::patch_hint(row.get("diff"))?;
    let diff_hint = if too_large == PullFileFlag::Known(true) {
        PullFileDiffHint::Oversized
    } else if collapsed == PullFileFlag::Known(true) {
        PullFileDiffHint::Omitted
    } else {
        observed_hint
    };
    let file = ProviderPullFile {
        identity,
        provider_file_id: None,
        change_kind: kind,
        provider_change_kind: native.into(),
        additions: PullFileCount::Unknown,
        deletions: PullFileCount::Unknown,
        total_changes: PullFileCount::Unknown,
        old_mode: a_mode,
        new_mode: b_mode,
        mode_changed,
        binary: PullFileFlag::Unknown,
        generated: common::flag(row.get("generated_file"))?,
        provider_collapsed: collapsed,
        provider_too_large: too_large,
        diff_hint,
    };
    file.validate().map_err(|_| invalid())?;
    Ok(file)
}
impl GitlabProvider {
    pub(super) async fn request_pull_files(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileProviderPage, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        let (project, iid) = identity(&PullFileSourceRequest::from(&request))?;
        let initial = self.http.pull_files(project, iid)?;
        let cursor = common::Cursor::open(&request, initial.to_string())?;
        let url = self.http.pull_file_continuation(
            &cursor.url,
            project,
            iid,
            u64::from(request.lease.provider_page_count + 1),
        )?;
        let current = common::fingerprint(&url);
        cursor.check_current(&current)?;
        let response = self.http.get(url, token).await?;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if rows.len() > MAX_PULL_FILES_PER_PROVIDER_PAGE
                || response.next.is_some() && rows.is_empty()
            {
                return Err(invalid());
            }
            let files = rows.iter().map(file).collect::<Result<Vec<_>, _>>()?;
            let cap = common::local_cap(&request, files.len(), response.next.is_some());
            let next = if cap.is_some() {
                None
            } else {
                response
                    .next
                    .map(|next| {
                        let url = self.http.pull_file_continuation(
                            &next,
                            project,
                            iid,
                            u64::from(request.lease.provider_page_count + 2),
                        )?;
                        cursor.advance(files.len(), current, next, common::fingerprint(&url))
                    })
                    .transpose()?
            };
            common::page(&request, files, next, cap, response.cooldown)
        })();
        result.map_err(|e| with_quota(e, response.cooldown))
    }
    pub(super) async fn request_selected_pull_file(
        &self,
        token: &SecretToken,
        request: PullFileSelectedRequest,
    ) -> Result<PullFileArtifactRead, ProviderError> {
        request.validate().map_err(|_| common::invalid())?;
        let (project, iid) = identity(&request.resource)?;
        let url = self
            .http
            .selected_pull_file(project, iid, request.file.provider_position)?;
        let response = self.http.get(url, token).await?;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| common::invalid())?;
            if rows.len() != 1 {
                return Err(common::invalid());
            }
            let observed = file(&rows[0])?;
            common::selected_content(&request, &observed, rows[0].get("diff"), response.cooldown)
        })();
        result.map_err(|error| common::quota(error, response.cooldown))
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
        let (project, iid) = identity(&request)?;
        let response = self
            .http
            .get(
                self.http
                    .resource_detail(project, iid, transport::ItemRoute::MergeRequests)?,
                token,
            )
            .await?;
        let result = (|| {
            let json: Map<String, Value> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            let (native, observed_iid) =
                resource_details::identity(&json, project, transport::ItemRoute::MergeRequests)?;
            if native.to_string() != request.subject.provider_id || observed_iid != iid {
                return Err(invalid());
            }
            let refs = json
                .get("diff_refs")
                .and_then(Value::as_object)
                .ok_or_else(|| ProviderError::new(ProviderErrorKind::Unavailable))?;
            let head = async_oid(refs.get("head_sha"))?;
            if json.get("sha").and_then(Value::as_str) != Some(head.as_str()) {
                return Err(ProviderError::new(ProviderErrorKind::Unavailable));
            }
            let validation = PullFileRangeValidation {
                base_oid: async_oid(refs.get("start_sha"))?,
                head_oid: head,
                base_repository_provider_id: resource_details::id(
                    json.get("target_project_id").ok_or_else(invalid)?,
                )?
                .to_string(),
                source_repository_provider_id: resource_details::id(
                    json.get("source_project_id").ok_or_else(invalid)?,
                )?
                .to_string(),
                merge_base_oid: Some(async_oid(refs.get("base_sha"))?),
            };
            // The diffs endpoint can silently enforce provider limits. Only an
            // exact fresh count can prove exhaustion; 1000+ is cap evidence.
            let changes = json
                .get("changes_count")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| ProviderError::new(ProviderErrorKind::Unavailable))?;
            let (count, cap) = if changes == "1000+" {
                (
                    None,
                    Some(PullFileCapEvidence {
                        provenance: PullFileCapProvenance::Provider,
                        reason: PullFileCapReason::ProviderFileLimit,
                        remote_has_more: PullFileFlag::Unknown,
                    }),
                )
            } else {
                let count = changes
                    .parse::<u32>()
                    .ok()
                    .filter(|v| v.to_string() == changes)
                    .ok_or_else(invalid)?;
                (Some(count), None)
            };
            common::validated_range(&request, validation, count, cap, response.cooldown)
        })();
        result.map_err(|e| with_quota(e, response.cooldown))
    }
}
