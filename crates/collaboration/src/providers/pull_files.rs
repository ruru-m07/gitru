//! Shared bounded cursor and normalization mechanics for native file adapters.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

pub(super) fn validate_request(
    request: &PullFileCollectionRequest,
    provider: ProviderKind,
    host: &str,
    strategy: PullFileSourceStrategy,
) -> Result<(), ProviderError> {
    request.validate().map_err(|_| invalid())?;
    if request.account.provider != provider || request.account.host != host {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if request.source.strategy != strategy
        || request.source.adapter_version != 1
        || !request.repository.selected
    {
        return Err(invalid());
    }
    Ok(())
}

fn binding(request: &PullFileCollectionRequest) -> Result<String, ProviderError> {
    let lease = &request.lease;
    let bytes = serde_json::to_vec(&(
        (
            &lease.run_id,
            &lease.generation,
            &request.account.id,
            &request.account.actor_id,
            &request.account.authorization_epoch,
            &request.authorization_view,
        ),
        (
            &request.binding.instance_id,
            &request.repository.id,
            &request.repository.provider_id,
            &request.subject.id,
            &request.subject.provider_id,
            &request.subject.number,
        ),
        (&request.binding.context, &request.source),
    ))
    .map_err(|_| invalid())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cursor {
    version: u8,
    binding: String,
    pages: u32,
    rows: u32,
    pub url: String,
    visited: Vec<String>,
    total: Option<u64>,
}

impl Cursor {
    pub fn open(
        request: &PullFileCollectionRequest,
        initial: String,
    ) -> Result<Self, ProviderError> {
        let expected = binding(request)?;
        match &request.cursor {
            None => Ok(Self {
                version: 1,
                binding: expected,
                pages: 0,
                rows: 0,
                url: initial,
                visited: vec![],
                total: None,
            }),
            Some(raw) => {
                if raw.len() > MAX_PULL_FILE_PROVIDER_CURSOR_BYTES {
                    return Err(invalid());
                }
                let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
                if cursor.version != 1
                    || cursor.binding != expected
                    || cursor.pages != request.lease.provider_page_count
                    || cursor.rows != request.start_position
                    || cursor.visited.len() != cursor.pages as usize
                    || cursor.visited.iter().any(|v| {
                        v.len() != 64
                            || !v
                                .bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    })
                    || cursor
                        .visited
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != cursor.visited.len()
                {
                    return Err(invalid());
                }
                Ok(cursor)
            }
        }
    }
    pub fn observe_total(&mut self, observed: Option<u64>) -> Result<Option<u64>, ProviderError> {
        if self
            .total
            .zip(observed)
            .is_some_and(|(old, new)| old != new)
        {
            return Err(invalid());
        }
        self.total = self.total.or(observed);
        Ok(self.total)
    }
    pub fn check_current(&self, fingerprint: &str) -> Result<(), ProviderError> {
        if self.visited.iter().any(|v| v == fingerprint) {
            Err(invalid())
        } else {
            Ok(())
        }
    }
    pub fn advance(
        mut self,
        count: usize,
        current: String,
        next: String,
        next_fingerprint: String,
    ) -> Result<String, ProviderError> {
        self.check_current(&current)?;
        self.visited.push(current);
        self.check_current(&next_fingerprint)?;
        self.pages += 1;
        self.rows += count as u32;
        self.url = next;
        let encoded = serde_json::to_string(&self).map_err(|_| invalid())?;
        if encoded.len() > MAX_PULL_FILE_PROVIDER_CURSOR_BYTES {
            return Err(invalid());
        }
        Ok(encoded)
    }
}

pub(super) fn fingerprint(url: &reqwest::Url) -> String {
    let mut pairs: Vec<_> = url.query_pairs().collect();
    pairs.sort();
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(url.path(), pairs)).expect("URL strings"))
    )
}

pub(super) fn positive(value: &str) -> Result<u64, ProviderError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v > 0 && v.to_string() == value)
        .ok_or_else(invalid)
}
pub(super) fn text(value: Option<&Value>, max: usize) -> Result<String, ProviderError> {
    value
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty() && v.len() <= max && !v.chars().any(char::is_control))
        .map(str::to_string)
        .ok_or_else(invalid)
}
pub(super) fn flag(value: Option<&Value>) -> Result<PullFileFlag, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(PullFileFlag::Unknown),
        Some(Value::Bool(v)) => Ok(PullFileFlag::Known(*v)),
        _ => Err(invalid()),
    }
}
pub(super) fn count(value: Option<&Value>) -> Result<PullFileCount, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(PullFileCount::Unknown),
        Some(value) => Ok(PullFileCount::Known(
            value.as_u64().ok_or_else(invalid)?.to_string(),
        )),
    }
}
pub(super) fn patch_hint(value: Option<&Value>) -> Result<PullFileDiffHint, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(PullFileDiffHint::Omitted),
        Some(Value::String(value)) if value.len() > MAX_PULL_FILE_TEXT_BYTES => {
            Ok(PullFileDiffHint::Oversized)
        }
        Some(Value::String(_)) => Ok(PullFileDiffHint::Candidate),
        _ => Err(invalid()),
    }
}
pub(super) fn local_cap(
    request: &PullFileCollectionRequest,
    count: usize,
    has_more: bool,
) -> Option<PullFileCapEvidence> {
    has_more
        .then(|| {
            let reason = if request.start_position + count as u32 >= MAX_PULL_FILES {
                Some(PullFileCapReason::LocalFileLimit)
            } else if request.lease.provider_page_count + 1 >= MAX_PULL_FILE_PROVIDER_PAGES {
                Some(PullFileCapReason::LocalPageLimit)
            } else {
                None
            };
            reason.map(|reason| PullFileCapEvidence {
                provenance: PullFileCapProvenance::Local,
                reason,
                remote_has_more: PullFileFlag::Known(true),
            })
        })
        .flatten()
}
pub(super) fn page(
    request: &PullFileCollectionRequest,
    files: Vec<ProviderPullFile>,
    next_cursor: Option<String>,
    cap: Option<PullFileCapEvidence>,
    cooldown: Option<u64>,
) -> Result<PullFileProviderPage, ProviderError> {
    let page = PullFileProviderPage {
        context: request.binding.context.clone(),
        files,
        source: request.source.clone(),
        start_position: request.start_position,
        next_cursor,
        cap,
        freshness_seconds: 60,
        cooldown_seconds: cooldown,
    };
    page.validate_for(request).map_err(|_| invalid())?;
    Ok(page)
}
pub(super) fn validated_range(
    request: &PullFileCollectionRequest,
    validation: PullFileRangeValidation,
    count: Option<u32>,
    cap: Option<PullFileCapEvidence>,
    cooldown: Option<u64>,
) -> Result<PullFileRangeValidationResult, ProviderError> {
    validation.validate().map_err(|_| invalid())?;
    if !request
        .binding
        .context
        .matches_range_validation(&validation)
    {
        return Err(invalid());
    }
    Ok(PullFileRangeValidationResult {
        validation,
        expected_file_count: count,
        collection_cap: cap,
        cooldown_seconds: cooldown,
    })
}
pub(super) fn quota(mut error: ProviderError, cooldown: Option<u64>) -> ProviderError {
    error.account_cooldown_seconds = error
        .account_cooldown_seconds
        .into_iter()
        .chain(cooldown)
        .max();
    error
}

#[cfg(test)]
pub(super) fn fixture_request(
    commit: PullCommitRequest,
    strategy: PullFileSourceStrategy,
) -> PullFileCollectionRequest {
    let binding = PullFileBinding {
        instance_id: "fixture-instance".into(),
        repository_id: commit.repository.id.clone(),
        repository_provider_id: commit.repository.provider_id.clone(),
        pull_id: commit.subject.id.clone(),
        pull_provider_id: commit.subject.provider_id.clone(),
        number: commit.subject.number.clone(),
        context: PullFileContext {
            merge_base_oid: None,
            base_oid: commit.context.base_oid,
            head_oid: commit.context.head_oid,
            base_repository_provider_id: commit.repository.provider_id.clone(),
            source_repository_provider_id: commit.context.source_repository_provider_id,
            body_metadata_facet_revision: "1".into(),
        },
    };
    let source = PullFileSource {
        strategy,
        adapter_version: 1,
    };
    let lease = PullFileLease {
        run_id: uuid::Uuid::new_v4().to_string(),
        generation: uuid::Uuid::new_v4().to_string(),
        account_id: commit.account.id.clone(),
        authorization_epoch: commit.account.authorization_epoch.clone(),
        authorization_view: "1".into(),
        binding: binding.clone(),
        source: source.clone(),
        provider_page_count: 0,
        accepted_row_count: 0,
        next_cursor: None,
        seen_cursors: vec![],
    };
    PullFileCollectionRequest {
        account: commit.account,
        authorization_view: "1".into(),
        repository: commit.repository,
        subject: commit.subject,
        binding,
        source,
        cursor: None,
        start_position: 0,
        lease,
    }
}

#[cfg(test)]
pub(super) fn continue_request(
    request: &mut PullFileCollectionRequest,
    page: &PullFileProviderPage,
) {
    request.lease = page.next_lease(request).unwrap().unwrap();
    request.cursor = request.lease.next_cursor.clone();
    request.start_position = request.lease.accepted_row_count;
}
