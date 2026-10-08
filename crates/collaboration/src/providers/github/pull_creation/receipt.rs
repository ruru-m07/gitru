use super::*;
pub(crate) fn parse(
    a: &RemoteAccount,
    p: &Payload,
    f: &Frame,
    bytes: &[u8],
) -> Result<CreatedReceipt, ProviderError> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| bad(None))?;
    let text = |key: &str| v.get(key).and_then(Value::as_str).ok_or_else(|| bad(None));
    let id = v
        .get("id")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .ok_or_else(|| bad(None))?
        .to_string();
    let number = v
        .get("number")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .ok_or_else(|| bad(None))?
        .to_string();
    let web = format!(
        "https://github.com/{}/pull/{number}",
        f.repository.full_name
    );
    let body = match v.get("body") {
        Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        _ => return Err(bad(None)),
    };
    if text("url")?
        != format!(
            "https://api.github.com/repos/{}/pulls/{number}",
            f.repository.full_name
        )
        || text("html_url")? != web
        || text("title")? != p.values.title
        || body.as_deref().unwrap_or("") != p.values.body
        || v.pointer("/user/id")
            .and_then(Value::as_u64)
            .map(|n| n.to_string())
            .as_deref()
            != Some(&a.actor_id)
        || v.get("draft").and_then(Value::as_bool) != Some(p.values.is_draft)
        || !matches!(text("state")?, "open" | "closed")
    {
        return Err(bad(None));
    }
    let author = v
        .pointer("/user/login")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        .ok_or_else(|| bad(None))?;
    let created =
        chrono::DateTime::parse_from_rfc3339(text("created_at")?).map_err(|_| bad(None))?;
    let updated =
        chrono::DateTime::parse_from_rfc3339(text("updated_at")?).map_err(|_| bad(None))?;
    if updated < created {
        return Err(bad(None));
    }
    let mut item = RemoteItem {
        id: format!("github:pull:{id}"),
        account_id: a.id.clone(),
        repository_id: Some(f.repository.id.clone()),
        provider_id: id,
        kind: RemoteItemKind::PullRequest,
        number: Some(number),
        title: p.values.title.clone(),
        body,
        body_omitted: false,
        author: Some(author.into()),
        web_url: Some(web),
        state: text("state")?.into(),
        updated_at: updated
            .with_timezone(&Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        head_oid: None,
        is_draft: Some(p.values.is_draft),
        reason: None,
        unread: None,
        native_inbox: None,
    };
    let request = DetailRequest {
        account: a.clone(),
        repository: f.repository.clone(),
        subject: item.clone(),
        facet: DetailFacet::Body,
        cursor: None,
        etag: None,
        source: None,
    };
    let (_, mut metadata) =
        pull_details::normalize(&request, bytes, "github/pull-detail/2026-03-10")?;
    // These collections were not requested by this operation and are not needed
    // to prove creation. Keep their authority omitted, including a response
    // concurrently augmented by repository automation, rather than exceeding the
    // pre-admitted proof budget or claiming that an unretained collection is empty.
    metadata.values.labels.clear();
    metadata.values.assignees.clear();
    metadata.values.milestone = None;
    if let Some(author) = &mut metadata.values.author {
        author.web_url = None;
    }
    for field in &mut metadata.fields {
        if matches!(
            field.field,
            crate::MetadataField::Labels
                | crate::MetadataField::Assignees
                | crate::MetadataField::Milestone
        ) {
            field.state = DetailValueState::Omitted;
        }
    }
    // GitHub's creation response observes the two branch tips, not a merge base.
    metadata.fields.push(crate::MetadataObservedField {
        field: crate::MetadataField::MergeBase,
        state: DetailValueState::Omitted,
    });
    metadata.values.updated_at = Some(item.updated_at.clone());
    metadata.source.provider_updated_at = Some(item.updated_at.clone());
    item.head_oid = metadata.values.head.as_ref().map(|h| h.oid.clone());
    item.state = metadata.values.state.clone().ok_or_else(|| bad(None))?;
    Ok(CreatedReceipt {
        item,
        metadata: ReceiptMetadata::from_observation(metadata),
        created_at: created
            .with_timezone(&Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
    })
}
