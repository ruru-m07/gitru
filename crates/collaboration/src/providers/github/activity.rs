//! Bounded heterogeneous activity, independent from conversation comments.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "github/activity/2026-03-10";
const STRATEGY: &str = "uncertain_subject_history";
const MAX_PAGES: u64 = 20;
const MAX_CURSOR_BYTES: usize = 4096;
const FIELDS: [DetailField; 4] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
    DetailField::Activity,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    strategy: String,
    account: String,
    actor: String,
    epoch: String,
    repository: String,
    repository_native: String,
    subject: String,
    subject_native: String,
    kind: RemoteItemKind,
    number: u64,
    pages: u64,
    page_hashes: Vec<String>,
    url: String,
}

fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}

fn positive(raw: &str) -> Result<u64, ProviderError> {
    let value = raw.parse::<u64>().map_err(|_| invalid())?;
    if value == 0 || value.to_string() != raw {
        return Err(invalid());
    }
    Ok(value)
}

fn bounded_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn identity(request: &DetailRequest) -> Result<(String, u64), ProviderError> {
    let repository = positive(&request.repository.provider_id)?;
    positive(&request.subject.provider_id)?;
    let number = positive(request.subject.number.as_deref().ok_or_else(invalid)?)?;
    positive(&request.account.actor_id)?;
    positive(&request.account.authorization_epoch)?;
    if request.account.provider != ProviderKind::Github
        || request.account.host != "github.com"
        || request.account.state != AccountState::Active
        || !bounded_identity(&request.account.id)
        || request.repository.account_id != request.account.id
        || !bounded_identity(&request.repository.id)
        || !request.repository.selected
        || !valid_repository_path(&request.repository.full_name)
        || request.subject.account_id != request.account.id
        || request.subject.repository_id.as_ref() != Some(&request.repository.id)
        || !bounded_identity(&request.subject.id)
    {
        return Err(invalid());
    }
    Ok((
        format!("/repositories/{repository}/issues/{number}/timeline"),
        number,
    ))
}

impl Cursor {
    fn open(
        request: &DetailRequest,
        path: &str,
        number: u64,
        http: &GithubHttp,
    ) -> Result<Self, ProviderError> {
        let Some(raw) = &request.cursor else {
            return Ok(Self {
                version: 1,
                strategy: STRATEGY.into(),
                account: request.account.id.clone(),
                actor: request.account.actor_id.clone(),
                epoch: request.account.authorization_epoch.clone(),
                repository: request.repository.id.clone(),
                repository_native: request.repository.provider_id.clone(),
                subject: request.subject.id.clone(),
                subject_native: request.subject.provider_id.clone(),
                kind: request.subject.kind.clone(),
                number,
                pages: 0,
                page_hashes: vec![],
                url: http
                    .endpoint(&format!("{}?per_page=50", path.trim_start_matches('/')))?
                    .to_string(),
            });
        };
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        let cursor: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if cursor.version != 1
            || cursor.strategy != STRATEGY
            || cursor.account != request.account.id
            || cursor.actor != request.account.actor_id
            || cursor.epoch != request.account.authorization_epoch
            || cursor.repository != request.repository.id
            || cursor.repository_native != request.repository.provider_id
            || cursor.subject != request.subject.id
            || cursor.subject_native != request.subject.provider_id
            || cursor.kind != request.subject.kind
            || cursor.number != number
            || cursor.page_hashes.len() != cursor.pages as usize
            || cursor
                .page_hashes
                .iter()
                .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
            || cursor.pages == 0
            || cursor.pages >= MAX_PAGES
        {
            return Err(invalid());
        }
        let (_, next) = http.check_collection_url(&cursor.url, path)?;
        if next != cursor.pages + 1 {
            return Err(invalid());
        }
        Ok(cursor)
    }

    fn encoded(&self) -> Result<String, ProviderError> {
        let raw = serde_json::to_string(self).map_err(|_| invalid())?;
        if raw.len() > MAX_CURSOR_BYTES {
            return Err(invalid());
        }
        Ok(raw)
    }
}

fn timestamp(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?;
    if value.len() > 128 {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(value).ok().map(|v| {
        v.with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    })
}
fn short(value: Option<&Value>, max: usize) -> Option<String> {
    value?
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= max && !s.contains('\0'))
        .map(str::to_owned)
}
fn activity(row: &Map<String, Value>) -> Option<DetailEntry> {
    let kind = short(row.get("event"), 64)?;
    if !kind
        .bytes()
        .all(|b| b.is_ascii_lowercase() || matches!(b, b'_' | b'-'))
    {
        return None;
    }
    let identity = if kind == "committed" {
        let sha = short(row.get("sha"), 64)?;
        if !crate::is_canonical_commit_oid(&sha) {
            return None;
        }
        format!("sha:{sha}")
    } else if let Some(id) = row.get("id").and_then(Value::as_u64).filter(|v| *v > 0) {
        format!("id:{id:020}")
    } else {
        let node = short(row.get("node_id"), 256)?;
        if node.chars().any(char::is_control) {
            return None;
        }
        format!("node:{:x}", Sha256::digest(node.as_bytes()))
    };
    let supported = matches!(
        kind.as_str(),
        "commented"
            | "committed"
            | "reviewed"
            | "closed"
            | "reopened"
            | "merged"
            | "assigned"
            | "unassigned"
            | "labeled"
            | "unlabeled"
            | "renamed"
            | "milestoned"
            | "demilestoned"
            | "locked"
            | "unlocked"
            | "head_ref_deleted"
            | "head_ref_restored"
            | "head_ref_force_pushed"
            | "base_ref_changed"
            | "review_requested"
            | "review_request_removed"
            | "ready_for_review"
            | "convert_to_draft"
            | "cross-referenced"
            | "connected"
            | "disconnected"
    );
    let occurred_at = timestamp(row.get("created_at")).or_else(|| {
        (kind == "reviewed")
            .then(|| timestamp(row.get("submitted_at")))
            .flatten()
    });
    let updated_at = timestamp(row.get("updated_at")).or_else(|| occurred_at.clone());
    let author = row
        .get("actor")
        .or_else(|| row.get("user"))
        .and_then(|v| v.get("login"))
        .and_then(|v| short(Some(v), 256))
        .filter(|s| !s.chars().any(char::is_control));
    let description = match kind.as_str() {
        "labeled" | "unlabeled" => row.get("label").and_then(|v| short(v.get("name"), 1024)),
        "assigned" | "unassigned" => row.get("assignee").and_then(|v| short(v.get("login"), 256)),
        "milestoned" | "demilestoned" => row
            .get("milestone")
            .and_then(|v| short(v.get("title"), 1024)),
        "reviewed" => short(row.get("state"), 128),
        "renamed" => row.get("rename").and_then(|v| {
            Some(format!(
                "{} → {}",
                short(v.get("from"), 480)?,
                short(v.get("to"), 480)?
            ))
        }),
        _ => None,
    };
    let has_body = matches!(kind.as_str(), "commented" | "reviewed" | "committed");
    let body = if has_body {
        match row.get(if kind == "committed" {
            "message"
        } else {
            "body"
        }) {
            Some(Value::String(s)) if s.len() <= 4096 && !s.contains('\0') => DetailValue {
                state: DetailValueState::Known,
                text: Some(s.clone()),
            },
            Some(Value::String(_)) => DetailValue {
                state: DetailValueState::Oversized,
                text: None,
            },
            Some(Value::Null) => DetailValue {
                state: DetailValueState::Known,
                text: None,
            },
            _ => DetailValue {
                state: DetailValueState::Omitted,
                text: None,
            },
        }
    } else {
        DetailValue {
            state: DetailValueState::Known,
            text: None,
        }
    };
    let native = crate::ActivityEvent {
        kind: kind.clone(),
        supported,
        occurred_at: occurred_at.clone(),
        description,
    };
    if !native.valid() {
        return None;
    }
    Some(DetailEntry {
        id: format!("github-activity:{kind}:{identity}"),
        provider_id: format!("{kind}:{identity}"),
        author,
        title: None,
        state: None,
        observed_body_state: body.state,
        body,
        updated_at,
        head_oid: None,
        native: Some(crate::NativeDetailPayload::ActivityV1(native)),
        field_mask: FIELDS.into(),
        field_validations: vec![],
    })
}

impl GithubProvider {
    pub(super) async fn request_activity(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Activity
            || !matches!(
                request.subject.kind,
                RemoteItemKind::PullRequest | RemoteItemKind::Issue
            )
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        let (path, number) = identity(&request)?;
        let mut cursor = Cursor::open(&request, &path, number, &self.http)?;
        let response = self
            .http
            .get_collection(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
                &path,
                cursor.pages + 1,
            )
            .await?;
        let result = (|| {
            let rows: Vec<Map<String, Value>> =
                serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if rows.len() > 50 {
                return Err(invalid());
            }
            let mut identities = HashSet::new();
            let mut entries = Vec::with_capacity(rows.len());
            let mut represented = true;
            for row in rows {
                let Some(entry) = activity(&row) else {
                    represented = false;
                    continue;
                };
                if !identities.insert(entry.provider_id.clone()) {
                    return Err(invalid());
                }
                entries.push(entry);
            }
            // Prevent a provider returning the exact same nonempty page forever
            // under increasing page numbers. The bounded cursor carries only hashes.
            if !entries.is_empty() {
                let mut ids: Vec<_> = entries.iter().map(|e| e.provider_id.as_str()).collect();
                ids.sort_unstable();
                let hash = format!("{:x}", Sha256::digest(ids.join("\n").as_bytes()));
                if cursor.page_hashes.contains(&hash) {
                    return Err(invalid());
                }
                cursor.page_hashes.push(hash);
            } else {
                cursor.page_hashes.push("0".repeat(64));
            }
            let reconciliation = if represented && cursor.pages == 0 && response.next_url.is_none()
            {
                DetailReconciliation::full_history()
            } else {
                DetailReconciliation::default()
            };
            cursor.pages += 1;
            let next_cursor = response
                .next_url
                .as_ref()
                .filter(|_| cursor.pages < MAX_PAGES)
                .map(|next| {
                    cursor.url = next.clone();
                    cursor.encoded()
                })
                .transpose()?;
            Ok(DetailPage {
                reconciliation,
                body: DetailValue::default(),
                metadata: None,
                entries,
                source: DetailSource {
                    source: SOURCE.into(),
                    adapter_version: 1,
                    field_mask: FIELDS.into(),
                    provider_updated_at: None,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                },
                next_cursor,
                etag: None,
                not_modified: false,
                freshness_seconds: 180,
                cooldown_seconds: response.cooldown_seconds,
            })
        })();
        result.map_err(|mut error: ProviderError| {
            error.account_cooldown_seconds = error
                .account_cooldown_seconds
                .into_iter()
                .chain(response.cooldown_seconds)
                .max();
            error
        })
    }
}
