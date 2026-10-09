//! Frozen item shape inside version-one operation evidence, independent of IPC.
//! Adding a public projection field must not change immutable operation bytes.
use crate::{RemoteItem, RemoteItemKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemV1 {
    id: String,
    account_id: String,
    repository_id: Option<String>,
    provider_id: String,
    kind: RemoteItemKind,
    number: Option<String>,
    title: String,
    body: Option<String>,
    body_omitted: bool,
    author: Option<String>,
    web_url: Option<String>,
    state: String,
    updated_at: String,
    head_oid: Option<String>,
    is_draft: Option<bool>,
    reason: Option<String>,
    unread: Option<bool>,
}

pub(crate) fn serialize<S: Serializer>(
    item: &RemoteItem,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if item.native_inbox.is_some() {
        return Err(serde::ser::Error::custom(
            "Inbox state is not operation-v1 item evidence",
        ));
    }
    ItemV1 {
        id: item.id.clone(),
        account_id: item.account_id.clone(),
        repository_id: item.repository_id.clone(),
        provider_id: item.provider_id.clone(),
        kind: item.kind.clone(),
        number: item.number.clone(),
        title: item.title.clone(),
        body: item.body.clone(),
        body_omitted: item.body_omitted,
        author: item.author.clone(),
        web_url: item.web_url.clone(),
        state: item.state.clone(),
        updated_at: item.updated_at.clone(),
        head_oid: item.head_oid.clone(),
        is_draft: item.is_draft,
        reason: item.reason.clone(),
        unread: item.unread,
    }
    .serialize(serializer)
}
pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<RemoteItem, D::Error> {
    let item = ItemV1::deserialize(deserializer)?;
    Ok(RemoteItem {
        id: item.id,
        account_id: item.account_id,
        repository_id: item.repository_id,
        provider_id: item.provider_id,
        kind: item.kind,
        number: item.number,
        title: item.title,
        body: item.body,
        body_omitted: item.body_omitted,
        author: item.author,
        web_url: item.web_url,
        state: item.state,
        updated_at: item.updated_at,
        head_oid: item.head_oid,
        is_draft: item.is_draft,
        reason: item.reason,
        unread: item.unread,
        native_inbox: None,
    })
}

#[cfg(test)]
mod tests;
