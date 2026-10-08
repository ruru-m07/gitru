//! Ordinary GitLab issue/MR notes. System activity and anchored discussions keep
//! their own facets; skipping them never grants absence authority to this view.
use super::{transport::ItemRoute, *};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
const SOURCE: &str = "gitlab/conversation-notes/v4";
const MAX_PAGES: u64 = 20;
const FIELDS: [DetailField; 3] = [
    DetailField::Body,
    DetailField::Author,
    DetailField::UpdatedAt,
];
#[derive(Serialize, Deserialize)]
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
    url: String,
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
    if r.facet != DetailFacet::Comments
        || r.subject.account_id != r.account.id
        || r.subject.repository_id.as_ref() != Some(&r.repository.id)
        || positive_id(&r.account.actor_id).is_none()
        || positive_id(&r.account.authorization_epoch).is_none()
        || [&r.account.id, &r.subject.id, &r.repository.id]
            .iter()
            .any(|s| s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control))
    {
        return Err(invalid());
    }
    Ok((project, native, iid, route))
}
impl Cursor {
    fn open(r: &DetailRequest, http: &GitlabHttp) -> Result<(Self, ItemRoute), ProviderError> {
        let (project, native, iid, route) = identity(r)?;
        let c = Self {
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
            url: http.notes(project, iid, route)?.to_string(),
        };
        let Some(raw) = &r.cursor else {
            return Ok((c, route));
        };
        if raw.len() > 4096 {
            return Err(invalid());
        }
        let saved: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if saved.version != 1
            || saved.account != c.account
            || saved.actor != c.actor
            || saved.epoch != c.epoch
            || saved.repository != c.repository
            || saved.project != c.project
            || saved.subject != c.subject
            || saved.native != c.native
            || saved.kind != c.kind
            || saved.iid != c.iid
            || saved.pages == 0
            || saved.pages >= MAX_PAGES
        {
            return Err(invalid());
        }
        http.note_continuation(&saved.url, project, iid, route, saved.pages + 1)?;
        Ok((saved, route))
    }
    fn encoded(&self) -> Result<String, ProviderError> {
        let s = serde_json::to_string(self).map_err(|_| invalid())?;
        if s.len() > 4096 {
            return Err(invalid());
        }
        Ok(s)
    }
}
fn note(row: &Map<String, Value>, c: &Cursor) -> Result<Option<DetailEntry>, ProviderError> {
    let kind = if c.kind == RemoteItemKind::Issue {
        "Issue"
    } else {
        "MergeRequest"
    };
    // A conflicting native identity is a bad response, never a skipped row from
    // an apparently complete collection. Missing identity is only partial data.
    for (field, want) in [
        ("project_id", c.project),
        ("noteable_id", c.native),
        ("noteable_iid", c.iid),
    ] {
        match row.get(field) {
            Some(v) if v.as_u64() == Some(want) => {}
            None => return Ok(None),
            _ => return Err(invalid()),
        }
    }
    match row.get("noteable_type") {
        Some(v) if v.as_str() == Some(kind) => {}
        None => return Ok(None),
        _ => return Err(invalid()),
    }
    if row.get("system").and_then(Value::as_bool) != Some(false)
        || row.get("type").is_some_and(|v| !v.is_null())
        || row
            .get("resolvable")
            .is_some_and(|v| v != &Value::Bool(false))
        || row.get("deleted").is_some_and(|v| v != &Value::Bool(false))
        || row.get("position").is_some_and(|v| !v.is_null())
    {
        return Ok(None);
    }
    let Some(id) = row.get("id").and_then(Value::as_u64).filter(|id| *id > 0) else {
        return Ok(None);
    };
    let Some(updated) = row
        .get("updated_at")
        .and_then(Value::as_str)
        .filter(|v| v.len() <= 128)
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
    else {
        return Ok(None);
    };
    let Some(created) = row.get("created_at") else {
        return Ok(None);
    };
    {
        let Some(created) = created
            .as_str()
            .filter(|v| v.len() <= 128)
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        else {
            return Ok(None);
        };
        if created > updated {
            return Ok(None);
        }
    }
    let author =
        match row.get("author") {
            Some(Value::Null) => None,
            Some(Value::Object(a)) => {
                let Some(login) = a.get("username").and_then(Value::as_str).filter(|v| {
                    !v.is_empty() && v.len() <= 255 && !v.chars().any(char::is_control)
                }) else {
                    return Ok(None);
                };
                if a.get("id").and_then(Value::as_u64).is_none_or(|id| id == 0) {
                    return Ok(None);
                }
                Some(login.into())
            }
            _ => return Ok(None),
        };
    let body = match row.get("body") {
        Some(Value::String(s)) if s.contains('\0') => return Ok(None),
        Some(Value::String(s)) if s.len() <= 65536 => DetailValue {
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
    Ok(Some(DetailEntry {
        id: format!("gitlab-note:{id:020}"),
        provider_id: id.to_string(),
        author,
        title: None,
        state: None,
        observed_body_state: body.state,
        body,
        updated_at: Some(
            updated
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        ),
        head_oid: None,
        native: None,
        field_mask: FIELDS.into(),
        field_validations: vec![],
    }))
}
impl GitlabProvider {
    pub(super) async fn request_comments(
        &self,
        token: &SecretToken,
        r: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        let (mut cursor, _) = Cursor::open(&r, &self.http)?;
        // Conditional parent/singleton validators cannot validate a notes page.
        let response = self
            .http
            .get(
                reqwest::Url::parse(&cursor.url).map_err(|_| invalid())?,
                token,
            )
            .await?;
        let result = (|| {
            let rows: Vec<Value> = serde_json::from_slice(&response.body).map_err(|_| invalid())?;
            if rows.len() > 50 {
                return Err(invalid());
            }
            let mut represented = true;
            let mut ids = HashSet::new();
            let mut entries = Vec::with_capacity(rows.len());
            for row in rows {
                let Some(row) = row.as_object() else {
                    represented = false;
                    continue;
                };
                if let Some(entry) = note(row, &cursor)? {
                    if !ids.insert(entry.provider_id.clone()) {
                        return Err(invalid());
                    }
                    represented &= entry.body.state == DetailValueState::Known;
                    entries.push(entry);
                } else {
                    represented = false;
                }
            }
            let full = cursor.pages == 0 && response.next.is_none() && represented;
            cursor.pages += 1;
            let next_cursor = if let Some(next) = response.next.as_ref() {
                cursor.url = next.clone();
                Some(cursor.encoded()?)
            } else {
                None
            };
            Ok(DetailPage {
                reconciliation: if full {
                    DetailReconciliation::full_history()
                } else {
                    DetailReconciliation::default()
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
