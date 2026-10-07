//! Sparse materialization of ordered durable intent. Provider rows stay untouched.
use super::*;
use crate::effective::{EFFECT_VERSION, ItemIntentPatch, MAX_ACTIVE_EFFECTS};
use crate::{PendingCommandIntent, PendingItemIntent};

pub(crate) async fn admit_in(
    tx: &mut Transaction<'_, Sqlite>,
    submission: &crate::CommandSubmission,
    patch: Option<ItemIntentPatch>,
) -> Result<()> {
    let Some(patch) = patch else {
        return Ok(());
    };
    let kind = match submission.target().kind() {
        crate::CommandTargetKind::PullRequest => RemoteItemKind::PullRequest,
        crate::CommandTargetKind::Issue => RemoteItemKind::Issue,
        crate::CommandTargetKind::Notification => RemoteItemKind::Notification,
        _ => {
            return Err(CollaborationError::invalid(
                "Command target does not support item effects",
            ));
        }
    };
    patch.validate(&kind)?;
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM items WHERE account_id=? AND id=? AND kind=?)",
    )
    .bind(submission.account_id())
    .bind(submission.target().id())
    .bind(tag(&kind)?)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    if !present {
        return Err(CollaborationError::new(
            ErrorCode::NotFound,
            "Command effect requires a cached subject",
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM commands c JOIN command_effects e USING(account_id,command_id) WHERE c.account_id=? AND c.target_id=? AND c.authorization_epoch=? AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')")
        .bind(submission.account_id()).bind(submission.target().id()).bind(positive_revision(submission.authorization_epoch())?)
        .fetch_one(&mut **tx).await.map_err(storage_error)?;
    if count >= MAX_ACTIVE_EFFECTS {
        return Err(CollaborationError::new(
            ErrorCode::Busy,
            "Resolve pending changes before adding more",
        ));
    }
    sqlx::query("INSERT INTO command_effects(account_id,command_id,submission_hash,version,patch_json) VALUES(?,?,?,?,?)")
        .bind(submission.account_id()).bind(submission.command_id()).bind(submission.submission_hash().as_slice())
        .bind(EFFECT_VERSION).bind(encode(&patch)?).execute(&mut **tx).await.map_err(storage_error)?;
    refresh_target_in(tx, submission.account_id(), submission.target().id()).await
}

/// Call after base or command-state changes inside the same writer transaction.
pub(crate) async fn refresh_target_in(
    tx: &mut Transaction<'_, Sqlite>,
    account_id: &str,
    target_id: &str,
) -> Result<()> {
    let base: Option<String> =
        sqlx::query_scalar("SELECT json FROM items WHERE account_id=? AND id=?")
            .bind(account_id)
            .bind(target_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    let Some(base) = base else {
        return Ok(());
    };
    let mut item: RemoteItem = decode(&base)?;
    let account = account_in(tx, account_id, false).await?;
    let previous: Option<(String, String)> = sqlx::query_as(
        "SELECT json,pending_json FROM effective_item_overrides WHERE account_id=? AND id=?",
    )
    .bind(account_id)
    .bind(target_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let rows=sqlx::query("SELECT c.command_id,c.state,e.patch_json,e.version FROM commands c JOIN command_effects e USING(account_id,command_id) WHERE c.account_id=? AND c.target_id=? AND c.authorization_epoch=? AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict') AND NOT EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=c.account_id AND q.command_id=c.command_id) ORDER BY c.enqueue_order LIMIT ?")
        .bind(account_id).bind(target_id).bind(positive_revision(&account.authorization_epoch)?).bind(MAX_ACTIVE_EFFECTS+1)
        .fetch_all(&mut **tx).await.map_err(storage_error)?;
    if rows.len() > MAX_ACTIVE_EFFECTS as usize {
        return Err(CollaborationError::storage());
    }
    let mut patch = ItemIntentPatch::default();
    let mut pending = Vec::new();
    if account.state == AccountState::Active {
        for row in rows {
            if row.get::<i64, _>("version") != EFFECT_VERSION {
                continue;
            }
            let next: ItemIntentPatch = decode(row.get("patch_json"))?;
            next.validate(&item.kind)?;
            pending.push(PendingCommandIntent {
                command_id: row.get("command_id"),
                state: row.get("state"),
                fields: next.fields(),
            });
            patch.overlay(next);
        }
    }
    let next = if pending.is_empty() {
        None
    } else {
        patch.apply(&mut item);
        Some((encode(&item)?, encode(&pending)?))
    };
    if previous == next {
        return Ok(());
    }
    sqlx::query("DELETE FROM effective_items_fts WHERE account_id=? AND id=?")
        .bind(account_id)
        .bind(target_id)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    if let Some((json, pending)) = next {
        let body: String = item
            .body
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(MAX_FTS_BODY_CHARS)
            .collect();
        sqlx::query("INSERT INTO effective_items_fts(account_id,id,title,body) VALUES(?,?,?,?)")
            .bind(account_id)
            .bind(target_id)
            .bind(&item.title)
            .bind(body)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
        sqlx::query("INSERT INTO effective_item_overrides(account_id,id,authorization_epoch,state,json,patch_json,pending_json) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,id) DO UPDATE SET authorization_epoch=excluded.authorization_epoch,state=excluded.state,json=excluded.json,patch_json=excluded.patch_json,pending_json=excluded.pending_json")
            .bind(account_id).bind(target_id).bind(positive_revision(&account.authorization_epoch)?).bind(&item.state).bind(json).bind(encode(&patch)?).bind(pending)
            .execute(&mut **tx).await.map_err(storage_error)?;
    } else {
        sqlx::query("DELETE FROM effective_item_overrides WHERE account_id=? AND id=?")
            .bind(account_id)
            .bind(target_id)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
    }
    let revision = record_change(
        tx,
        account_id,
        positive_revision(&account.authorization_epoch)?,
        &format!("effective:{target_id}"),
        false,
    )
    .await?;
    sqlx::query("INSERT INTO effective_item_revisions(account_id,id,kind,repository_id,revision) VALUES(?,?,?,?,?) ON CONFLICT(account_id,id) DO UPDATE SET kind=excluded.kind,repository_id=excluded.repository_id,revision=excluded.revision")
        .bind(account_id).bind(target_id).bind(tag(&item.kind)?).bind(&item.repository_id).bind(positive_revision(&revision)?)
        .execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

pub(super) async fn query_revision_in(
    tx: &mut Transaction<'_, Sqlite>,
    query: &ItemQuery,
) -> Result<i64> {
    sqlx::query_scalar("SELECT coalesce(max(revision),0) FROM effective_item_revisions WHERE account_id=? AND kind=? AND (? IS NULL OR repository_id=?)")
        .bind(&query.account_id).bind(tag(&query.kind)?).bind(&query.repository_id).bind(&query.repository_id)
        .fetch_one(&mut **tx).await.map_err(storage_error)
}

pub(super) async fn subject_revision_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<String> {
    let revision: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM effective_item_revisions WHERE account_id=? AND id=?",
    )
    .bind(account)
    .bind(subject)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    Ok(revision.unwrap_or(0).to_string())
}

pub(super) async fn pending_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    ids: &[&str],
) -> Result<Vec<PendingItemIntent>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    if ids.len() > 100 {
        return Err(CollaborationError::invalid("Too many pending subjects"));
    }
    let mut query = sqlx::QueryBuilder::<Sqlite>::new(
        "SELECT e.id,e.pending_json FROM effective_item_overrides e JOIN accounts a ON a.id=e.account_id AND a.authorization_epoch=e.authorization_epoch WHERE e.account_id=",
    );
    query.push_bind(account).push(" AND e.id IN (");
    let mut list = query.separated(",");
    for id in ids {
        list.push_bind(id);
    }
    list.push_unseparated(") ORDER BY e.id");
    let rows = query
        .build()
        .fetch_all(&mut **tx)
        .await
        .map_err(storage_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(PendingItemIntent {
                subject_id: row.get("id"),
                commands: decode(row.get("pending_json"))?,
            })
        })
        .collect()
}

pub(super) async fn patch_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    subject: &str,
) -> Result<Option<ItemIntentPatch>> {
    let json:Option<String>=sqlx::query_scalar("SELECT e.patch_json FROM effective_item_overrides e JOIN accounts a ON a.id=e.account_id AND a.authorization_epoch=e.authorization_epoch WHERE e.account_id=? AND e.id=?")
        .bind(account).bind(subject).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    json.map(|json| decode(&json)).transpose()
}

/// Search exactly one effective representation; a replaced title must not match the old base.
pub(super) fn push_search(
    sql: &mut sqlx::QueryBuilder<Sqlite>,
    alias: &str,
    account: &str,
    search: &str,
) {
    let phrase = format!("\"{}\"", search.replace('"', "\"\""));
    sql.push(format!(" AND {alias}.id IN (SELECT id FROM items_fts WHERE items_fts MATCH "))
        .push_bind(phrase.clone()).push(" AND account_id=").push_bind(account.to_owned())
        .push(" AND NOT EXISTS(SELECT 1 FROM effective_item_overrides e JOIN accounts a ON a.id=e.account_id AND a.authorization_epoch=e.authorization_epoch WHERE e.account_id=items_fts.account_id AND e.id=items_fts.id) UNION SELECT id FROM effective_items_fts WHERE effective_items_fts MATCH ")
        .push_bind(phrase).push(" AND account_id=").push_bind(account.to_owned())
        .push(" AND EXISTS(SELECT 1 FROM effective_item_overrides e JOIN accounts a ON a.id=e.account_id AND a.authorization_epoch=e.authorization_epoch WHERE e.account_id=effective_items_fts.account_id AND e.id=effective_items_fts.id))");
}

#[cfg(test)]
mod tests;
