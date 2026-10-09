//! Parse required201 identity separately from optional metadata membership.
use super::{invalid, observations};
use crate::{issue_metadata::native as n, providers::ProviderError};
use serde_json::Value;

pub(crate) fn parse_created(
    status: u16,
    p: &n::PayloadV2,
    preparation: &n::PreparationV2,
    bytes: &[u8],
) -> Result<n::ReceiptV2, ProviderError> {
    if status != 201 || bytes.len() > 4 * 1024 * 1024 || !n::preparation_matches(preparation, p) {
        return Err(invalid());
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let id = |key| {
        v.get(key)
            .and_then(Value::as_u64)
            .filter(|v| *v > 0)
            .map(|v| v.to_string())
            .ok_or_else(invalid)
    };
    let text = |key| v.get(key).and_then(Value::as_str).ok_or_else(invalid);
    let number = id("number")?;
    let repository = &preparation.frame.repository_path;
    let body = match v.get("body") {
        Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        _ => return Err(invalid()),
    };
    if v.get("pull_request").is_some_and(|v| !v.is_null())
        || text("url")? != format!("https://api.github.com/repos/{repository}/issues/{number}")
        || text("repository_url")? != format!("https://api.github.com/repos/{repository}")
        || text("html_url")? != format!("https://github.com/{repository}/issues/{number}")
        || text("title")? != p.title
        || body.as_deref().unwrap_or("") != p.body
    {
        return Err(invalid());
    }
    let actor = v
        .pointer("/user/id")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .map(|v| v.to_string())
        .ok_or_else(invalid)?;
    let login = v
        .pointer("/user/login")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    n::display(login, 256).map_err(|_| invalid())?;
    let stamp = |key| -> Result<String, ProviderError> {
        Ok(chrono::DateTime::parse_from_rfc3339(text(key)?)
            .map_err(|_| invalid())?
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true))
    };
    let result = n::ReceiptV2 {
        preparation: preparation.clone(),
        core: n::CreatedCoreV2 {
            provider_id: id("id")?,
            number,
            title: text("title")?.into(),
            body,
            author_id: actor,
            author_login: login.into(),
            state: text("state")?.into(),
            web_url: text("html_url")?.into(),
            created_at: stamp("created_at")?,
            updated_at: stamp("updated_at")?,
        },
        metadata: observations::observe(&v, &p.metadata),
    };
    if !n::receipt_matches(&result, p) {
        return Err(invalid());
    }
    n::encode(&result).map_err(|_| invalid())?;
    Ok(result)
}
