//! Receipt-derived canonical cache publication; the caller owns proof and commit.
use crate::storage::*;
use crate::{
    DetailCommit, DetailFacet, DetailField, DetailReconciliation, DetailSource,
    DetailSubjectBinding, DetailValue, DetailValueState, ResourceMetadataObservation,
};

pub(crate) struct Publication<'a> {
    pub command_id: &'a str,
    pub authorization_view: &'a str,
    pub repository: &'a RemoteRepository,
    pub item: &'a RemoteItem,
    pub metadata: Option<ResourceMetadataObservation>,
    pub observed_at: &'a str,
}

/// Called only after the operation policy has verified the authenticated 201.
/// No revision is published until the outer transaction also links the draft and
/// confirms the command. Historical receipts never replace newer observations.
pub(crate) async fn publish_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    publication: Publication<'_>,
) -> Result<String> {
    let Publication {
        command_id,
        authorization_view,
        repository,
        item,
        metadata: observed_metadata,
        observed_at,
    } = publication;
    epoch_in(tx, &account.id, &account.authorization_epoch).await?;
    let current = account_in(tx, &account.id, true).await?;
    let scope = repository_scope(&repository.id, &RemoteItemKind::PullRequest);
    if current.provider != ProviderKind::Github
        || current.host != "github.com"
        || current.actor_id != account.actor_id
        || metadata(tx).await?.1 != authorization_view
        || repository.account_id != account.id
        || item.account_id != account.id
        || item.repository_id.as_deref() != Some(&repository.id)
        || item.kind != RemoteItemKind::PullRequest
        || item.id != format!("github:pull:{}", item.provider_id)
        || item.body_omitted
        || item.title.is_empty()
        || item.title.len() > 16_384
        || item
            .body
            .as_ref()
            .is_some_and(|body| body.len() > MAX_BODY_BYTES)
        || item
            .head_oid
            .as_deref()
            .is_none_or(|v| !crate::pull_creation::native::oid(v))
        || item.is_draft.is_none()
        || item.native_inbox.is_some()
        || item.unread.is_some()
    {
        return Err(stale());
    }
    validate_number(&item.provider_id)?;
    validate_number(item.number.as_deref().ok_or_else(stale)?)?;
    canonical_time(observed_at)?;
    ensure_selected_scope(tx, &account.id, &scope).await?;
    let saved_repository: String =
        sqlx::query_scalar("SELECT json FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(&repository.id)
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    let saved_repository: RemoteRepository = decode(&saved_repository)?;
    if saved_repository.provider_id != repository.provider_id
        || saved_repository.full_name != repository.full_name
        || saved_repository.web_url != repository.web_url
    {
        return Err(stale());
    }
    let permitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commands WHERE account_id=? AND command_id=? AND target_kind='repository' AND target_id=? AND operation_kind='github.create_pull_request' AND payload_version=1 AND CAST(authorization_epoch AS TEXT)=?) AND NOT EXISTS(SELECT 1 FROM sync_scopes WHERE account_id=? AND scope IN (?,?) AND access_denied=1)")
        .bind(&account.id).bind(command_id).bind(&repository.id).bind(&account.authorization_epoch)
        .bind(&account.id).bind(&scope).bind(DetailFacet::Body.scope(&item.id))
        .fetch_one(&mut **tx).await.map_err(storage_error)?;
    if !permitted {
        return Err(stale());
    }
    let mut incoming = item.clone();
    incoming.updated_at = canonical_time(&item.updated_at)?;
    let previous: Option<String> =
        sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(&item.id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    let previous: Option<RemoteItem> = previous.as_deref().map(decode).transpose()?;
    if previous.as_ref().is_some_and(|old| {
        old.provider_id != item.provider_id
            || old.kind != item.kind
            || old.repository_id != item.repository_id
            || old.number != item.number
    }) {
        return Err(stale());
    }
    // Equal timestamps are not proof that a delayed response supersedes data
    // which another authenticated observation has already published.
    let body_can_replace = body_is_older_in(tx, &current, &incoming).await?;
    let replace = previous.as_ref().is_none_or(|old| {
        timestamp_older(&old.updated_at, &incoming.updated_at) && body_can_replace
    });
    let canonical = if replace {
        &incoming
    } else {
        previous.as_ref().ok_or_else(stale)?
    };
    identities::item_in(tx, &current, canonical).await?;
    if replace {
        sqlx::query("INSERT INTO items(account_id,id,repository_id,kind,state,updated_at,json) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,id) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at,json=excluded.json")
            .bind(&account.id).bind(&item.id).bind(&repository.id).bind("pull_request").bind(&canonical.state)
            .bind(&canonical.updated_at).bind(encode(canonical)?).execute(&mut **tx).await.map_err(storage_error)?;
        sqlx::query("DELETE FROM items_fts WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(&item.id)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
        sqlx::query("INSERT INTO items_fts(account_id,id,title,body) VALUES(?,?,?,?)")
            .bind(&account.id)
            .bind(&item.id)
            .bind(&canonical.title)
            .bind(
                canonical
                    .body
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(MAX_FTS_BODY_CHARS)
                    .collect::<String>(),
            )
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    }
    sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO NOTHING")
        .bind(&account.id).bind(&scope).bind(Uuid::new_v4().to_string())
        .bind(encode(&missing_coverage())?).bind(encode(&SyncStatus::default())?)
        .execute(&mut **tx).await.map_err(storage_error)?;
    let membership: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM scope_membership WHERE account_id=? AND scope=? AND entity_id=?)")
        .bind(&account.id).bind(&scope).bind(&item.id).fetch_one(&mut **tx).await.map_err(storage_error)?;
    if !membership {
        // This marker is evidence of creation, never evidence of enumeration.
        sqlx::query("INSERT INTO scope_membership(account_id,scope,entity_id,last_seen_run) VALUES(?,?,?,?)")
            .bind(&account.id).bind(&scope).bind(&item.id).bind(format!("receipt:{command_id}"))
            .execute(&mut **tx).await.map_err(storage_error)?;
        sqlx::query("INSERT INTO pull_creation_visibility(account_id,authorization_epoch,entity_id,command_id) VALUES(?,?,?,?)")
            .bind(&account.id).bind(&account.authorization_epoch).bind(&item.id).bind(command_id)
            .execute(&mut **tx).await.map_err(storage_error)?;
    }
    if replace || previous.as_ref() == Some(&incoming) {
        seed_body_in(
            tx,
            &current,
            authorization_view,
            repository,
            &incoming,
            observed_metadata,
            observed_at,
        )
        .await?;
    }
    effective::refresh_target_in(tx, &account.id, &item.id).await?;
    let revision = record_change(
        tx,
        &account.id,
        positive_revision(&account.authorization_epoch)?,
        &scope,
        false,
    )
    .await?;
    sqlx::query("UPDATE sync_scopes SET data_revision=?,etag=NULL,last_modified=NULL WHERE account_id=? AND scope=?")
        .bind(revision_number(&revision)?).bind(&account.id).bind(&scope).execute(&mut **tx).await.map_err(storage_error)?;
    Ok(item.id.clone())
}

fn canonical_time(value: &str) -> Result<String> {
    if value.len() > 128 {
        return Err(stale());
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|time| {
            time.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        })
        .map_err(|_| stale())
}

fn validate_number(value: &str) -> Result<()> {
    if value.len() > 20
        || value
            .parse::<u64>()
            .ok()
            .filter(|v| *v > 0)
            .is_none_or(|v| v.to_string() != value)
    {
        return Err(stale());
    }
    Ok(())
}

async fn seed_body_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    authorization_view: &str,
    repository: &RemoteRepository,
    item: &RemoteItem,
    metadata: Option<ResourceMetadataObservation>,
    observed_at: &str,
) -> Result<()> {
    if !body_is_older_in(tx, account, item).await? {
        return Ok(());
    }
    let scope = DetailFacet::Body.scope(&item.id);
    let run_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sync_scopes(account_id,scope,run_id,coverage_json,sync_json) VALUES(?,?,?,?,?) ON CONFLICT(account_id,scope) DO UPDATE SET run_id=excluded.run_id,next_cursor=NULL,etag=NULL,last_modified=NULL")
        .bind(&account.id).bind(&scope).bind(&run_id).bind(encode(&missing_coverage())?).bind(encode(&SyncStatus::default())?)
        .execute(&mut **tx).await.map_err(storage_error)?;
    let source = metadata
        .as_ref()
        .map(|m| m.source.source.clone())
        .unwrap_or_else(|| "github/pull-creation/2026-03-10".into());
    let instance_id = identities::instance_in(tx, account).await?.id;
    details::apply_detail_in(
        tx,
        DetailCommit {
            reconciliation: DetailReconciliation::full_history(),
            account_id: account.id.clone(),
            authorization_epoch: account.authorization_epoch.clone(),
            authorization_view: authorization_view.into(),
            instance_id,
            subject_id: item.id.clone(),
            facet: DetailFacet::Body,
            run_id,
            request_cursor: None,
            body: DetailValue {
                state: DetailValueState::Known,
                text: item.body.clone(),
            },
            metadata,
            subject_binding: Some(DetailSubjectBinding {
                repository_id: repository.id.clone(),
                repository_provider_id: repository.provider_id.clone(),
                provider_id: item.provider_id.clone(),
                number: item.number.clone(),
                kind: RemoteItemKind::PullRequest,
                head_oid: item.head_oid.clone(),
            }),
            check_context: None,
            review_context: None,
            entries: vec![],
            source: DetailSource {
                source,
                adapter_version: 1,
                field_mask: vec![DetailField::Body],
                provider_updated_at: Some(item.updated_at.clone()),
                observed_at: observed_at.into(),
            },
            next_cursor: None,
            etag: None,
            not_modified: false,
            whole_scope: true,
            complete: true,
            freshness_seconds: 60,
        },
    )
    .await?;
    Ok(())
}

async fn body_is_older_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    item: &RemoteItem,
) -> Result<bool> {
    let old: Option<Option<String>> = sqlx::query_scalar("SELECT value_source_json FROM detail_observations WHERE account_id=? AND subject_id=? AND facet='body' AND authorization_epoch=?")
        .bind(&account.id).bind(&item.id).bind(&account.authorization_epoch).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    if let Some(old) = old {
        let old: Option<DetailSource> = old.as_deref().map(decode).transpose()?;
        if old
            .as_ref()
            .and_then(|s| s.provider_updated_at.as_deref())
            .is_none_or(|time| !timestamp_older(time, &item.updated_at))
        {
            return Ok(false);
        }
    }
    if resource_metadata::read_in(tx, account, &item.id)
        .await?
        .and_then(|metadata| metadata.values.updated_at)
        .is_some_and(|time| !timestamp_older(&time, &item.updated_at))
    {
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
#[path = "publication_tests.rs"]
mod tests;
