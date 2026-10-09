//! Read-only conversation comments, bound to immutable repository addressing.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) const SOURCE: &str = "github/comments/2026-03-10";
const STRATEGY: &str = "uncertain_subject_history";
const MAX_PAGES: u64 = 20;
const MAX_CURSOR_BYTES: usize = 4096;
const FIELDS: [DetailField; 3] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
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

fn native_id(value: &Value) -> Result<u64, ProviderError> {
    value
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(invalid)
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
        format!("/repositories/{repository}/issues/{number}/comments"),
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

fn parent_url(raw: &str, request: &DetailRequest, number: u64) -> Result<(), ProviderError> {
    if raw.len() > 2048
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.contains(['%', '\\', '#'])
        || raw.split('/').any(|part| matches!(part, "." | ".."))
    {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(raw).map_err(|_| invalid())?;
    let paths = [
        format!("/repos/{}/issues/{number}", request.repository.full_name),
        format!(
            "/repositories/{}/issues/{number}",
            request.repository.provider_id
        ),
    ];
    if url.as_str() != raw
        || url.scheme() != "https"
        || url.host_str() != Some("api.github.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !paths.iter().any(|path| path == url.path())
    {
        return Err(invalid());
    }
    Ok(())
}

fn comment(
    row: &Map<String, Value>,
    request: &DetailRequest,
    number: u64,
) -> Result<DetailEntry, ProviderError> {
    let id = native_id(row.get("id").ok_or_else(invalid)?)?;
    parent_url(
        row.get("issue_url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?,
        request,
        number,
    )?;
    let updated_at = row
        .get("updated_at")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if updated_at.len() > 128 || chrono::DateTime::parse_from_rfc3339(updated_at).is_err() {
        return Err(invalid());
    }
    let author = match row.get("user").ok_or_else(invalid)? {
        Value::Null => None,
        Value::Object(user) => {
            native_id(user.get("id").ok_or_else(invalid)?)?;
            let login = user
                .get("login")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            if login.is_empty() || login.len() > 255 || login.chars().any(char::is_control) {
                return Err(invalid());
            }
            Some(login.into())
        }
        _ => return Err(invalid()),
    };
    let body = match row.get("body") {
        None => DetailValue {
            state: DetailValueState::Omitted,
            text: None,
        },
        Some(Value::String(value)) if value.len() <= 65_536 => DetailValue {
            state: DetailValueState::Known,
            text: Some(value.clone()),
        },
        Some(Value::String(_)) => DetailValue {
            state: DetailValueState::Oversized,
            text: None,
        },
        _ => return Err(invalid()),
    };
    Ok(DetailEntry {
        id: format!("github-comment:{id:020}"),
        provider_id: id.to_string(),
        author,
        title: None,
        state: None,
        observed_body_state: body.state,
        body,
        updated_at: Some(updated_at.into()),
        head_oid: None,
        native: None,
        field_mask: FIELDS.into(),
        field_validations: vec![],
    })
}

impl GithubProvider {
    pub(super) async fn request_comments(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet != DetailFacet::Comments
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
            for row in rows {
                let entry = comment(&row, &request, number)?;
                if !identities.insert(entry.provider_id.clone()) {
                    return Err(invalid());
                }
                entries.push(entry);
            }
            let reconciliation = if cursor.pages == 0 && response.next_url.is_none() {
                DetailReconciliation::full_history()
            } else {
                DetailReconciliation::default()
            };
            cursor.pages += 1;
            let next_cursor = response
                .next_url
                .as_ref()
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
