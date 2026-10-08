//! Composite, explicitly incomplete history. Events never authorize current state.
use super::{
    transport::{ActivitySource, ItemRoute},
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const SOURCE: &str = "gitlab/activity/v4";
const MAX_PAGES: u64 = 20;
const MAX_CURSOR: usize = 4096;
const FIELDS: [DetailField; 4] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
    DetailField::Activity,
];
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedCursor {
    accepted: u64,
    next: Option<String>,
    hashes: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    account: String,
    actor: String,
    epoch: String,
    repository: String,
    project: u64,
    subject: String,
    native: u64,
    kind: RemoteItemKind,
    iid: u64,
    pages: u64,
    turn: usize,
    feeds: [FeedCursor; 3],
}
fn identity(r: &DetailRequest) -> Result<(u64, u64, u64, ItemRoute), ProviderError> {
    if r.account.provider != ProviderKind::Gitlab || r.account.host != "gitlab.com" {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if r.account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    let route = match r.subject.kind {
        RemoteItemKind::Issue => ItemRoute::Issues,
        RemoteItemKind::PullRequest => ItemRoute::MergeRequests,
        _ => return Err(ProviderError::new(ProviderErrorKind::Unsupported)),
    };
    let project = resource_details::repository_identity(&r.account, &r.repository)?;
    let native = positive_id(&r.subject.provider_id).ok_or_else(invalid)?;
    let iid = r
        .subject
        .number
        .as_deref()
        .and_then(positive_id)
        .ok_or_else(invalid)?;
    if r.facet != DetailFacet::Activity
        || r.subject.account_id != r.account.id
        || r.subject.repository_id.as_ref() != Some(&r.repository.id)
        || !r.repository.selected
        || positive_id(&r.account.actor_id).is_none()
        || positive_id(&r.account.authorization_epoch).is_none()
        || [&r.account.id, &r.subject.id, &r.repository.id]
            .iter()
            .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
    {
        return Err(invalid());
    }
    Ok((project, native, iid, route))
}
impl Cursor {
    fn open(r: &DetailRequest, http: &GitlabHttp) -> Result<(Self, ItemRoute), ProviderError> {
        let (project, native, iid, route) = identity(r)?;
        let initial = || -> Result<Self, ProviderError> {
            let mut feeds = Vec::new();
            for source in ActivitySource::ALL {
                feeds.push(FeedCursor {
                    accepted: 0,
                    next: Some(
                        http.activity_collection(project, iid, route, source, 1)?
                            .to_string(),
                    ),
                    hashes: vec![],
                });
            }
            Ok(Self {
                version: 1,
                account: r.account.id.clone(),
                actor: r.account.actor_id.clone(),
                epoch: r.account.authorization_epoch.clone(),
                repository: r.repository.id.clone(),
                project,
                subject: r.subject.id.clone(),
                native,
                kind: r.subject.kind.clone(),
                iid,
                pages: 0,
                turn: 0,
                feeds: feeds.try_into().map_err(|_| invalid())?,
            })
        };
        let Some(raw) = &r.cursor else {
            return Ok((initial()?, route));
        };
        if raw.len() > MAX_CURSOR {
            return Err(invalid());
        }
        let c: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if c.version != 1
            || c.account != r.account.id
            || c.actor != r.account.actor_id
            || c.epoch != r.account.authorization_epoch
            || c.repository != r.repository.id
            || c.project != project
            || c.subject != r.subject.id
            || c.native != native
            || c.kind != r.subject.kind
            || c.iid != iid
            || c.pages == 0
            || c.pages >= MAX_PAGES
            || c.turn >= 3
            || c.feeds[c.turn].next.is_none()
        {
            return Err(invalid());
        }
        let mut total = 0;
        for (index, feed) in c.feeds.iter().enumerate() {
            if feed.accepted > c.pages
                || feed.hashes.len() as u64 > feed.accepted
                || (feed.accepted == 0 && feed.next.is_none())
                || feed
                    .hashes
                    .iter()
                    .any(|h| h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()))
                || feed.hashes.iter().collect::<HashSet<_>>().len() != feed.hashes.len()
            {
                return Err(invalid());
            }
            total += feed.accepted;
            if let Some(next) = &feed.next {
                http.activity_continuation(
                    next,
                    project,
                    iid,
                    route,
                    ActivitySource::ALL[index],
                    feed.accepted + 1,
                )?;
            }
        }
        if total != c.pages {
            return Err(invalid());
        }
        Ok((c, route))
    }
    fn encoded(&self) -> Result<String, ProviderError> {
        let raw = serde_json::to_string(self).map_err(|_| invalid())?;
        if raw.len() > MAX_CURSOR {
            return Err(invalid());
        }
        Ok(raw)
    }
}
fn text(value: Option<&Value>, limit: usize) -> Option<String> {
    value?
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= limit && !s.contains('\0'))
        .map(str::to_owned)
}
fn timestamp(value: Option<&Value>) -> Option<String> {
    let raw = text(value, 128)?;
    let at = chrono::DateTime::parse_from_rfc3339(&raw).ok()?;
    Some(
        at.with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
    )
}
fn row_identity(
    row: &Map<String, Value>,
    c: &Cursor,
    source: ActivitySource,
) -> Result<bool, ProviderError> {
    let kind = if c.kind == RemoteItemKind::Issue {
        "Issue"
    } else {
        "MergeRequest"
    };
    let fields = if source == ActivitySource::Notes {
        vec![
            ("project_id", c.project),
            ("noteable_id", c.native),
            ("noteable_iid", c.iid),
        ]
    } else {
        vec![("resource_id", c.native)]
    };
    let mut complete = true;
    for (field, expected) in fields {
        match row.get(field) {
            Some(value) if value.as_u64() == Some(expected) => (),
            None => complete = false,
            _ => return Err(invalid()),
        }
    }
    let field = if source == ActivitySource::Notes {
        "noteable_type"
    } else {
        "resource_type"
    };
    match row.get(field) {
        Some(value) if value.as_str() == Some(kind) => (),
        None => complete = false,
        _ => return Err(invalid()),
    }
    Ok(complete)
}
fn entry(
    row: &Map<String, Value>,
    c: &Cursor,
    source: ActivitySource,
) -> Result<Option<DetailEntry>, ProviderError> {
    if !row_identity(row, c, source)? {
        return Ok(None);
    }
    if source == ActivitySource::Notes
        && (row.get("system").and_then(Value::as_bool) != Some(true)
            || row.get("position").is_some_and(|v| !v.is_null())
            || row.get("deleted").is_some_and(|v| v != &Value::Bool(false)))
    {
        return Ok(None);
    }
    let Some(id) = row.get("id").and_then(Value::as_u64).filter(|v| *v > 0) else {
        return Ok(None);
    };
    let Some(created) = timestamp(row.get("created_at")) else {
        return Ok(None);
    };
    let updated = if source == ActivitySource::Notes {
        let Some(updated) = timestamp(row.get("updated_at")) else {
            return Ok(None);
        };
        if updated < created {
            return Ok(None);
        }
        updated
    } else {
        created.clone()
    };
    let author_field = if source == ActivitySource::Notes {
        "author"
    } else {
        "user"
    };
    let author = match row.get(author_field) {
        Some(Value::Null) => None,
        Some(Value::Object(a)) => {
            let Some(login) =
                text(a.get("username"), 255).filter(|v| !v.chars().any(char::is_control))
            else {
                return Ok(None);
            };
            if a.get("id").and_then(Value::as_u64).is_none_or(|id| id == 0) {
                return Ok(None);
            }
            Some(login)
        }
        _ => return Ok(None),
    };
    let mut body = DetailValue {
        state: DetailValueState::Known,
        text: None,
    };
    let (kind, supported, description) = match source {
        ActivitySource::Notes => {
            body = match row.get("body") {
                Some(Value::String(s)) if s.contains('\0') => return Ok(None),
                Some(Value::String(s)) if s.len() <= 4096 => DetailValue {
                    state: DetailValueState::Known,
                    text: Some(s.clone()),
                },
                Some(Value::String(_)) => DetailValue {
                    state: DetailValueState::Oversized,
                    text: None,
                },
                None => DetailValue {
                    state: DetailValueState::Omitted,
                    text: None,
                },
                _ => return Ok(None),
            };
            ("system_note".to_string(), true, None)
        }
        ActivitySource::State => {
            let state = text(row.get("state"), 64);
            match state.as_deref() {
                Some("opened" | "closed" | "reopened" | "merged") => (state.unwrap(), true, None),
                _ => ("unknown_state".into(), false, state),
            }
        }
        ActivitySource::Labels => {
            let action = text(row.get("action"), 64);
            let label = row.get("label").and_then(|v| text(v.get("name"), 1024));
            match action.as_deref() {
                Some("add") => ("labeled".into(), true, label),
                Some("remove") => ("unlabeled".into(), true, label),
                _ => ("unknown_label_event".into(), false, action),
            }
        }
    };
    let native = crate::ActivityEvent {
        kind,
        supported,
        occurred_at: Some(created),
        description,
    };
    if !native.valid() {
        return Ok(None);
    }
    Ok(Some(DetailEntry {
        id: format!("gitlab-activity:{}:{id:020}", source.tag()),
        provider_id: format!("{}:{id}", source.tag()),
        author,
        title: None,
        state: None,
        observed_body_state: body.state,
        body,
        updated_at: Some(updated),
        head_oid: None,
        native: Some(crate::NativeDetailPayload::ActivityV1(native)),
        field_mask: FIELDS.into(),
        field_validations: vec![],
    }))
}
impl GitlabProvider {
    pub(super) async fn request_activity(
        &self,
        token: &SecretToken,
        r: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        let (mut c, _) = Cursor::open(&r, &self.http)?;
        let source = ActivitySource::ALL[c.turn];
        let response = self
            .http
            .get(
                reqwest::Url::parse(c.feeds[c.turn].next.as_deref().ok_or_else(invalid)?)
                    .map_err(|_| invalid())?,
                token,
            )
            .await?;
        let result = (|| {
            let rows: Vec<Value> = serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if rows.len() > 50 {
                return Err(invalid());
            }
            let feed = &mut c.feeds[c.turn];
            // Visibility filtering can legitimately produce multiple empty pages
            // with advancing links. Only repeated nonempty content is a loop.
            if !rows.is_empty() {
                let hash = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&rows).map_err(|_| invalid())?)
                );
                if feed.hashes.contains(&hash) {
                    return Err(invalid());
                }
                feed.hashes.push(hash);
            }
            let mut entries = Vec::with_capacity(rows.len());
            let mut ids = HashSet::new();
            for row in &rows {
                let Some(row) = row.as_object() else {
                    continue;
                };
                if let Some(value) = entry(row, &c, source)? {
                    if !ids.insert(value.id.clone()) {
                        return Err(invalid());
                    }
                    entries.push(value);
                }
            }
            let feed = &mut c.feeds[c.turn];
            feed.accepted += 1;
            feed.next = response.next.clone();
            c.pages += 1;
            let next = (1..=3)
                .map(|step| (c.turn + step) % 3)
                .find(|i| c.feeds[*i].next.is_some());
            let capped = next.is_some() && c.pages >= MAX_PAGES;
            let next_cursor = if let Some(next) = next.filter(|_| !capped) {
                c.turn = next;
                Some(c.encoded()?)
            } else {
                None
            };
            Ok(DetailPage {
                reconciliation: DetailReconciliation {
                    enumeration: if capped {
                        DetailEnumeration::Truncated
                    } else {
                        DetailEnumeration::Uncertain
                    },
                    head_scope: DetailHeadScope::SubjectHistory,
                },
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
                cooldown_seconds: response.cooldown,
            })
        })();
        result.map_err(|e| with_quota(e, response.cooldown))
    }
}
#[cfg(test)]
mod tests;
