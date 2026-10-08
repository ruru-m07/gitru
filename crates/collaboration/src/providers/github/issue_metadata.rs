//! Bounded repository metadata catalogs and single-read identity revalidation.
use super::*;
use crate::{
    IssueMetadataAvailability as Availability, IssueMetadataKind as Kind,
    IssueMetadataReason as Reason, IssueMetadataReference as Reference,
    issue_metadata::native as n, *,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

fn invalid() -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidResponse)
}
fn quota(mut error: ProviderError, cooldown: Option<u64>) -> ProviderError {
    error.account_cooldown_seconds = error
        .account_cooldown_seconds
        .into_iter()
        .chain(cooldown)
        .max();
    error
}
fn validate(c: &IssueMetadataReadContext) -> Result<(), ProviderError> {
    if c.account.provider != ProviderKind::Github || c.account.host != "github.com" {
        return Err(ProviderError::new(ProviderErrorKind::Unsupported));
    }
    if c.account.state != AccountState::Active {
        return Err(ProviderError::new(ProviderErrorKind::Authentication));
    }
    if c.repository.account_id != c.account.id || !valid_repository_path(&c.repository.full_name) {
        return Err(invalid());
    }
    for id in [&c.account.id, &c.repository.id] {
        crate::issue_creation::native::identifier(id).map_err(|_| invalid())?;
    }
    for id in [
        &c.repository.provider_id,
        &c.account.actor_id,
        &c.account.authorization_epoch,
    ] {
        n::positive(id).map_err(|_| invalid())?;
    }
    crate::issue_creation::native::revision(&c.authorization_view, false).map_err(|_| invalid())?;
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u32,
    account: String,
    actor: String,
    epoch: String,
    view: String,
    repository: String,
    native_repository: String,
    path: String,
    kind: Kind,
    catalog_generation: String,
    page: u32,
}
impl Cursor {
    fn new(c: &IssueMetadataReadContext, kind: Kind, catalog_generation: &str, page: u32) -> Self {
        Self {
            version: 1,
            account: c.account.id.clone(),
            actor: c.account.actor_id.clone(),
            epoch: c.account.authorization_epoch.clone(),
            view: c.authorization_view.clone(),
            repository: c.repository.id.clone(),
            native_repository: c.repository.provider_id.clone(),
            path: c.repository.full_name.clone(),
            kind,
            catalog_generation: catalog_generation.into(),
            page,
        }
    }
    fn parse(r: &IssueMetadataCatalogRequest) -> Result<Self, ProviderError> {
        let Some(raw) = &r.cursor else {
            return Ok(Self::new(&r.context, r.kind, &r.catalog_generation, 1));
        };
        if raw.len() > 16_384 {
            return Err(invalid());
        }
        let value: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        if !(2..=20).contains(&value.page)
            || value != Self::new(&r.context, r.kind, &r.catalog_generation, value.page)
            || serde_json::to_string(&value).map_err(|_| invalid())? != *raw
        {
            return Err(invalid());
        }
        Ok(value)
    }
}
fn route(c: &IssueMetadataReadContext, kind: Kind) -> String {
    format!(
        "repositories/{}/{}",
        c.repository.provider_id,
        match kind {
            Kind::Labels => "labels",
            Kind::Assignees => "assignees",
            Kind::Milestones => "milestones",
        }
    )
}
fn id(v: &Value) -> Result<String, ProviderError> {
    v.as_u64()
        .filter(|n| *n > 0)
        .map(|n| n.to_string())
        .ok_or_else(invalid)
}
fn text(v: &Value, key: &str, max: usize) -> Result<String, ProviderError> {
    let s = v.get(key).and_then(Value::as_str).ok_or_else(invalid)?;
    n::display(s, max).map_err(|_| invalid())?;
    Ok(s.into())
}
fn option(v: &Value, kind: Kind) -> Result<IssueMetadataOption, ProviderError> {
    let native = id(v.get("id").ok_or_else(invalid)?)?;
    let (reference, availability, reason) = match kind {
        Kind::Labels => {
            let color = match v.get("color") {
                None | Some(Value::Null) => None,
                Some(Value::String(s))
                    if s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()) =>
                {
                    Some(s.clone())
                }
                _ => return Err(invalid()),
            };
            let (availability, reason) = match v.get("archived_at") {
                Some(Value::Null) => (Availability::Available, None),
                Some(Value::String(s)) if chrono::DateTime::parse_from_rfc3339(s).is_ok() => {
                    (Availability::Unavailable, Some(Reason::Archived))
                }
                None => (Availability::Unknown, Some(Reason::Unobserved)),
                _ => return Err(invalid()),
            };
            (
                Reference::Label(IssueMetadataLabel {
                    provider_id: native,
                    name: text(v, "name", 1024)?,
                    color,
                }),
                availability,
                reason,
            )
        }
        Kind::Assignees => (
            Reference::Assignee(IssueMetadataAssignee {
                provider_id: native,
                login: text(v, "login", 256)?,
            }),
            Availability::Available,
            None,
        ),
        Kind::Milestones => {
            let (availability, reason) = match v.get("state").and_then(Value::as_str) {
                Some("open") => (Availability::Available, None),
                Some("closed") => (Availability::Unavailable, Some(Reason::Closed)),
                _ => return Err(invalid()),
            };
            (
                Reference::Milestone(IssueMetadataMilestone {
                    provider_id: native,
                    number: id(v.get("number").ok_or_else(invalid)?)?,
                    title: text(v, "title", 1024)?,
                }),
                availability,
                reason,
            )
        }
    };
    Ok(IssueMetadataOption {
        reference,
        availability,
        reason,
    })
}
fn option_id(v: &IssueMetadataOption) -> &str {
    match &v.reference {
        Reference::Label(v) => &v.provider_id,
        Reference::Assignee(v) => &v.provider_id,
        Reference::Milestone(v) => &v.provider_id,
    }
}
fn literal(http: &GithubHttp, route: &str, segment: &str) -> Result<reqwest::Url, ProviderError> {
    if matches!(segment, "." | "..") {
        return Err(invalid());
    }
    let mut url = http.endpoint(&format!("{route}/"))?;
    url.path_segments_mut()
        .map_err(|_| invalid())?
        .pop_if_empty()
        .push(segment);
    Ok(url)
}
impl GithubProvider {
    pub(super) async fn issue_metadata_catalog(
        &self,
        token: &SecretToken,
        r: IssueMetadataCatalogRequest,
    ) -> Result<IssueMetadataCatalogPage, ProviderError> {
        validate(&r.context)?;
        n::positive(&r.catalog_generation).map_err(|_| invalid())?;
        let cursor = Cursor::parse(&r)?;
        let path = format!("/{}", route(&r.context, r.kind));
        let mut url = self.http.endpoint(&path)?;
        url.query_pairs_mut()
            .append_pair("per_page", "100")
            .append_pair("page", &cursor.page.to_string());
        let fixed: &[(&str, &str)] = if r.kind == Kind::Milestones {
            &[("state", "all")]
        } else {
            &[]
        };
        for (key, value) in fixed {
            url.query_pairs_mut().append_pair(key, value);
        }
        let page = self
            .http
            .get_collection_with_fixed_query(url, token, &path, u64::from(cursor.page), 100, fixed)
            .await?;
        let parse = || -> Result<IssueMetadataCatalogPage, ProviderError> {
            let values: Vec<Value> = serde_json::from_slice(&page.body).map_err(|_| invalid())?;
            if values.len() > 100 {
                return Err(invalid());
            }
            let options = values
                .iter()
                .map(|v| option(v, r.kind))
                .collect::<Result<Vec<_>, _>>()?;
            let mut seen = HashSet::new();
            if options.iter().any(|v| !seen.insert(option_id(v))) {
                return Err(invalid());
            }
            let truncated = cursor.page == 20 && page.next_url.is_some();
            let next_cursor = if page.next_url.is_some() && !truncated {
                Some(
                    serde_json::to_string(&Cursor {
                        page: cursor.page + 1,
                        ..cursor
                    })
                    .map_err(|_| invalid())?,
                )
            } else {
                None
            };
            Ok(IssueMetadataCatalogPage {
                options,
                next_cursor,
                truncated,
                coverage: CoverageState::Partial,
                cooldown_seconds: page.cooldown_seconds,
            })
        };
        parse().map_err(|e| quota(e, page.cooldown_seconds))
    }
    pub(super) async fn issue_metadata_point(
        &self,
        token: &SecretToken,
        r: IssueMetadataPointRequest,
    ) -> Result<IssueMetadataPointRead, ProviderError> {
        validate(&r.context)?;
        let (url, no_content) = match &r.point {
            IssueMetadataPoint::Repository => (
                self.http.endpoint(&format!(
                    "repositories/{}",
                    r.context.repository.provider_id
                ))?,
                false,
            ),
            IssueMetadataPoint::Label(v) => {
                n::label(v).map_err(|_| invalid())?;
                (
                    literal(&self.http, &route(&r.context, Kind::Labels), &v.name)?,
                    false,
                )
            }
            IssueMetadataPoint::AssigneeIdentity(v) => {
                n::assignee(v).map_err(|_| invalid())?;
                (
                    self.http.endpoint(&format!("user/{}", v.provider_id))?,
                    false,
                )
            }
            IssueMetadataPoint::AssigneeAssignable(v) => {
                n::assignee(v).map_err(|_| invalid())?;
                (
                    literal(&self.http, &route(&r.context, Kind::Assignees), &v.login)?,
                    true,
                )
            }
            IssueMetadataPoint::Milestone(v) => {
                n::milestone(v).map_err(|_| invalid())?;
                (
                    self.http.endpoint(&format!(
                        "{}/{}",
                        route(&r.context, Kind::Milestones),
                        v.number
                    ))?,
                    false,
                )
            }
        };
        let page = self.http.get_metadata_point(url, token, no_content).await?;
        let parse = || -> Result<IssueMetadataPointValue, ProviderError> {
            if no_content {
                return Ok(IssueMetadataPointValue::Assignable);
            }
            let v: Value = serde_json::from_slice(&page.body).map_err(|_| invalid())?;
            let kind = match &r.point {
                IssueMetadataPoint::Repository => {
                    if id(v.get("id").ok_or_else(invalid)?)? != r.context.repository.provider_id
                        || v.get("full_name").and_then(Value::as_str)
                            != Some(r.context.repository.full_name.as_str())
                        || v.get("has_issues").and_then(Value::as_bool) != Some(true)
                        || v.get("archived").and_then(Value::as_bool) != Some(false)
                    {
                        return Err(invalid());
                    }
                    let metadata_access = match v.pointer("/permissions/push") {
                        Some(Value::Bool(true)) => Availability::Available,
                        Some(Value::Bool(false)) => Availability::Unavailable,
                        None | Some(Value::Null) => Availability::Unknown,
                        _ => return Err(invalid()),
                    };
                    return Ok(IssueMetadataPointValue::Repository { metadata_access });
                }
                IssueMetadataPoint::Label(_) => Kind::Labels,
                IssueMetadataPoint::AssigneeIdentity(_) => Kind::Assignees,
                IssueMetadataPoint::Milestone(_) => Kind::Milestones,
                IssueMetadataPoint::AssigneeAssignable(_) => return Err(invalid()),
            };
            let mut observed = option(&v, kind)?;
            let (id_match, name_match) = match (&r.point, &observed.reference) {
                (IssueMetadataPoint::Label(want), Reference::Label(got)) => {
                    (want.provider_id == got.provider_id, want.name == got.name)
                }
                (IssueMetadataPoint::AssigneeIdentity(want), Reference::Assignee(got)) => {
                    (want.provider_id == got.provider_id, want.login == got.login)
                }
                (IssueMetadataPoint::Milestone(want), Reference::Milestone(got)) => (
                    want.provider_id == got.provider_id,
                    want.number == got.number,
                ),
                _ => return Err(invalid()),
            };
            if !id_match || !name_match {
                observed.availability = Availability::Unavailable;
                observed.reason = Some(if !id_match {
                    Reason::ChangedIdentity
                } else {
                    Reason::ChangedName
                });
            }
            Ok(IssueMetadataPointValue::Selection {
                reference: observed.reference,
                identity_matches: id_match && name_match,
                availability: observed.availability,
                reason: observed.reason,
            })
        };
        Ok(IssueMetadataPointRead {
            value: parse().map_err(|e| quota(e, page.cooldown_seconds))?,
            cooldown_seconds: page.cooldown_seconds,
        })
    }
}

pub(crate) mod observations;
pub(crate) mod receipt;
#[cfg(test)]
mod tests;
