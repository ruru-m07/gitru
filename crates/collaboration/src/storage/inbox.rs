//! Gitru-owned inbox organization. Provider read/done observations stay immutable.
use super::*;
use chrono::{DateTime, SecondsFormat, Utc};

const MAX_CURSOR_BYTES: usize = 4_096;
const MAX_SNOOZE_DAYS: i64 = 30;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InboxCursor {
    version: u32,
    account_id: String,
    authorization_view: String,
    projection_view: String,
    query: String,
    evaluated_at: String,
    updated_at: String,
    id: String,
}

#[derive(Debug, Clone)]
struct SavedLocalInboxState {
    disposition: LocalInboxDisposition,
    bookmarked: bool,
    snoozed_until: Option<String>,
    activity_updated_at: String,
    generation: i64,
}

fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    if value.len() > 128 || value.trim() != value {
        return Err(CollaborationError::invalid("Invalid inbox timestamp"));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| CollaborationError::invalid("Invalid inbox timestamp"))
}

fn canonical_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn parse_generation(value: &str, allow_zero: bool) -> Result<i64> {
    let generation = value
        .parse::<i64>()
        .map_err(|_| CollaborationError::invalid("Invalid inbox generation"))?;
    if generation < i64::from(!allow_zero) || generation.to_string() != value {
        return Err(CollaborationError::invalid("Invalid inbox generation"));
    }
    Ok(generation)
}

fn disposition_tag(disposition: LocalInboxDisposition) -> &'static str {
    match disposition {
        LocalInboxDisposition::Inbox => "inbox",
        LocalInboxDisposition::Done => "done",
    }
}

fn disposition_from_tag(value: &str) -> Result<LocalInboxDisposition> {
    match value {
        "inbox" => Ok(LocalInboxDisposition::Inbox),
        "done" => Ok(LocalInboxDisposition::Done),
        _ => Err(CollaborationError::storage()),
    }
}

fn projected_state(
    item_updated_at: &str,
    saved: Option<SavedLocalInboxState>,
    evaluated_at: DateTime<Utc>,
) -> Result<LocalInboxState> {
    let item_time = parse_time(item_updated_at)?;
    let Some(saved) = saved else {
        return Ok(LocalInboxState {
            disposition: LocalInboxDisposition::Inbox,
            effective_disposition: LocalInboxEffectiveDisposition::Inbox,
            bookmarked: false,
            snoozed_until: None,
            activity_updated_at: canonical_time(item_time),
            superseded_by_activity: false,
            generation: "0".into(),
        });
    };
    let authored_activity = parse_time(&saved.activity_updated_at)?;
    let snoozed_until = saved.snoozed_until.as_deref().map(parse_time).transpose()?;
    let superseded_by_activity = item_time > authored_activity;
    let effective_disposition = if superseded_by_activity {
        LocalInboxEffectiveDisposition::Inbox
    } else if saved.disposition == LocalInboxDisposition::Done {
        LocalInboxEffectiveDisposition::Done
    } else if snoozed_until.is_some_and(|deadline| deadline > evaluated_at) {
        LocalInboxEffectiveDisposition::Snoozed
    } else {
        LocalInboxEffectiveDisposition::Inbox
    };
    Ok(LocalInboxState {
        disposition: saved.disposition,
        effective_disposition,
        bookmarked: saved.bookmarked,
        snoozed_until: snoozed_until.map(canonical_time),
        activity_updated_at: canonical_time(authored_activity),
        superseded_by_activity,
        generation: saved.generation.to_string(),
    })
}

fn validate_query(query: &InboxQuery) -> Result<()> {
    if query.limit == 0 || query.limit > MAX_ITEMS {
        return Err(CollaborationError::invalid(
            "Inbox limit must be between 1 and 100",
        ));
    }
    if query.search.as_ref().is_some_and(|value| value.len() > 256) {
        return Err(CollaborationError::invalid("Search text is too long"));
    }
    if query
        .remote_state
        .as_deref()
        .is_some_and(|value| !matches!(value, "unread" | "read" | "pending" | "done"))
    {
        return Err(CollaborationError::invalid(
            "Unsupported provider inbox filter",
        ));
    }
    Ok(())
}

fn push_common_predicates(sql: &mut sqlx::QueryBuilder<Sqlite>, query: &InboxQuery) {
    sql.push(" n.account_id=")
        .push_bind(query.account_id.clone())
        .push(" AND n.kind='notification' AND EXISTS(SELECT 1 FROM scope_membership m WHERE m.account_id=n.account_id AND m.scope='notifications' AND m.entity_id=n.id AND m.active=1) AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=n.account_id AND s.scope='notifications' AND s.access_denied=1)");
    match query.remote_state.as_deref() {
        Some("unread") => {
            sql.push(" AND json_extract(n.json,'$.unread')=1");
        }
        Some("read") => {
            sql.push(" AND json_extract(n.json,'$.unread')=0");
        }
        Some(state @ ("pending" | "done")) => {
            sql.push(" AND n.state=").push_bind(state.to_owned());
        }
        Some(_) | None => {}
    }
    if let Some(search) = query
        .search
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        let phrase = format!("\"{}\"", search.replace('"', "\"\""));
        sql.push(" AND n.id IN(SELECT id FROM items_fts WHERE items_fts MATCH ")
            .push_bind(phrase)
            .push(" AND account_id=")
            .push_bind(query.account_id.clone())
            .push(")");
    }
}

fn push_local_filter(
    sql: &mut sqlx::QueryBuilder<Sqlite>,
    filter: LocalInboxFilter,
    evaluated_at: &str,
) {
    match filter {
        LocalInboxFilter::Inbox => {
            sql.push(" AND (l.notification_id IS NULL OR n.updated_at>l.activity_updated_at OR (l.disposition='inbox' AND (l.snoozed_until IS NULL OR l.snoozed_until<=")
                .push_bind(evaluated_at.to_owned())
                .push(")))");
        }
        LocalInboxFilter::Snoozed => {
            sql.push(" AND l.disposition='inbox' AND n.updated_at<=l.activity_updated_at AND l.snoozed_until>")
                .push_bind(evaluated_at.to_owned());
        }
        LocalInboxFilter::Done => {
            sql.push(" AND l.disposition='done' AND n.updated_at<=l.activity_updated_at");
        }
        LocalInboxFilter::Bookmarked => {
            sql.push(" AND l.bookmarked=1");
        }
        LocalInboxFilter::All => {}
    }
}

async fn projection_view(tx: &mut Transaction<'_, Sqlite>, account_id: &str) -> Result<String> {
    let values: (i64, i64) = sqlx::query_as(
        "SELECT COALESCE((SELECT data_revision FROM sync_scopes WHERE account_id=? AND scope='notifications'),0),COALESCE((SELECT revision FROM local_inbox_projection WHERE account_id=?),0)",
    )
    .bind(account_id)
    .bind(account_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    Ok(format!("{:x}", Sha256::digest(encode(&values)?.as_bytes())))
}

fn saved_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Option<SavedLocalInboxState>> {
    let local_id: Option<String> = row.get("local_id");
    if local_id.is_none() {
        return Ok(None);
    }
    Ok(Some(SavedLocalInboxState {
        disposition: disposition_from_tag(row.get("local_disposition"))?,
        bookmarked: row.get("local_bookmarked"),
        snoozed_until: row.get("local_snoozed_until"),
        activity_updated_at: row.get("local_activity_updated_at"),
        generation: row.get("local_generation"),
    }))
}

impl Store {
    pub async fn inbox(&self, query: InboxQuery) -> Result<InboxPage> {
        self.inbox_at(query, Utc::now()).await
    }

    async fn inbox_at(&self, mut query: InboxQuery, now: DateTime<Utc>) -> Result<InboxPage> {
        validate_query(&query)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        account_in(&mut tx, &query.account_id, true).await?;
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let projection_view = projection_view(&mut tx, &query.account_id).await?;
        let fingerprint = {
            let cursor = query.cursor.take();
            let value = encode(&query)?;
            query.cursor = cursor;
            value
        };
        let cursor: Option<InboxCursor> = query
            .cursor
            .as_ref()
            .map(|value| {
                if value.len() > MAX_CURSOR_BYTES {
                    return Err(CollaborationError::invalid("Invalid local inbox cursor"));
                }
                serde_json::from_str(value)
                    .map_err(|_| CollaborationError::invalid("Invalid local inbox cursor"))
            })
            .transpose()?;
        if let Some(cursor) = &cursor
            && (cursor.version != 1
                || cursor.account_id != query.account_id
                || cursor.query != fingerprint
                || cursor.authorization_view != authorization_view
                || cursor.projection_view != projection_view)
        {
            return Err(stale());
        }
        let evaluated_at = cursor
            .as_ref()
            .map(|cursor| parse_time(&cursor.evaluated_at))
            .transpose()?
            .unwrap_or(now);
        if evaluated_at > now {
            return Err(stale());
        }
        let evaluated_at_text = canonical_time(evaluated_at);
        let mut sql = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT n.json,l.notification_id AS local_id,l.disposition AS local_disposition,l.bookmarked AS local_bookmarked,l.snoozed_until AS local_snoozed_until,l.activity_updated_at AS local_activity_updated_at,l.generation AS local_generation FROM items n LEFT JOIN local_inbox_state l ON l.account_id=n.account_id AND l.notification_id=n.id WHERE",
        );
        push_common_predicates(&mut sql, &query);
        push_local_filter(&mut sql, query.local_state, &evaluated_at_text);
        if let Some(cursor) = &cursor {
            sql.push(" AND (n.updated_at,n.id)<(")
                .push_bind(cursor.updated_at.clone())
                .push(",")
                .push_bind(cursor.id.clone())
                .push(")");
        }
        sql.push(" ORDER BY n.updated_at DESC,n.id DESC LIMIT ")
            .push_bind(i64::from(query.limit) + 1);
        let rows = sql
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
        let mut entries = rows
            .iter()
            .map(|row| {
                let item: RemoteItem = decode(row.get("json"))?;
                let local = projected_state(&item.updated_at, saved_from_row(row)?, evaluated_at)?;
                Ok(InboxEntry { item, local })
            })
            .collect::<Result<Vec<_>>>()?;
        let has_more = entries.len() > query.limit as usize;
        entries.truncate(query.limit as usize);
        let next_cursor = if has_more {
            entries
                .last()
                .map(|entry| {
                    encode(&InboxCursor {
                        version: 1,
                        account_id: query.account_id.clone(),
                        authorization_view: authorization_view.clone(),
                        projection_view: projection_view.clone(),
                        query: fingerprint.clone(),
                        evaluated_at: evaluated_at_text.clone(),
                        updated_at: entry.item.updated_at.clone(),
                        id: entry.item.id.clone(),
                    })
                })
                .transpose()?
        } else {
            None
        };
        let next_local_change_at = if query.local_state == LocalInboxFilter::Done {
            None
        } else {
            let mut deadline = sqlx::QueryBuilder::<Sqlite>::new(
                "SELECT MIN(l.snoozed_until) FROM items n JOIN local_inbox_state l ON l.account_id=n.account_id AND l.notification_id=n.id WHERE",
            );
            push_common_predicates(&mut deadline, &query);
            if query.local_state == LocalInboxFilter::Bookmarked {
                deadline.push(" AND l.bookmarked=1");
            }
            deadline
                .push(" AND l.disposition='inbox' AND n.updated_at<=l.activity_updated_at AND l.snoozed_until>")
                .push_bind(evaluated_at_text.clone());
            deadline
                .build_query_scalar()
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?
        };
        let presentation_query = ItemQuery {
            account_id: query.account_id.clone(),
            kind: RemoteItemKind::Notification,
            repository_id: None,
            state: None,
            search: None,
            cursor: None,
            limit: query.limit,
        };
        let (coverage, sync) = query_presentation(&mut tx, &presentation_query).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(InboxPage {
            entries,
            revision,
            authorization_view,
            next_cursor,
            coverage,
            sync,
            evaluated_at: evaluated_at_text,
            next_local_change_at,
        })
    }

    pub async fn set_local_inbox_state(
        &self,
        request: SetLocalInboxStateRequest,
    ) -> Result<LocalInboxWriteReceipt> {
        self.set_local_inbox_state_at(request, Utc::now()).await
    }

    async fn set_local_inbox_state_at(
        &self,
        request: SetLocalInboxStateRequest,
        now: DateTime<Utc>,
    ) -> Result<LocalInboxWriteReceipt> {
        validate_identifier(&request.notification_id)?;
        let expected_generation = parse_generation(&request.expected_generation, true)?;
        let requested_snooze = request
            .snoozed_until
            .as_deref()
            .map(parse_time)
            .transpose()?;
        if let Some(deadline) = requested_snooze
            && (deadline <= now || deadline > now + chrono::Duration::days(MAX_SNOOZE_DAYS))
        {
            return Err(CollaborationError::invalid(
                "Snooze deadline must be within the next 30 days",
            ));
        }
        let requested_snooze = requested_snooze.map(canonical_time);
        match request.mutation {
            LocalInboxMutation::Disposition => {
                let disposition = request.disposition.ok_or_else(|| {
                    CollaborationError::invalid("Disposition mutation is incomplete")
                })?;
                if request.bookmarked.is_some()
                    || (disposition == LocalInboxDisposition::Done && requested_snooze.is_some())
                {
                    return Err(CollaborationError::invalid("Invalid disposition mutation"));
                }
            }
            LocalInboxMutation::Bookmark => {
                if request.bookmarked.is_none()
                    || request.disposition.is_some()
                    || requested_snooze.is_some()
                {
                    return Err(CollaborationError::invalid("Invalid bookmark mutation"));
                }
            }
        }
        let mut writer = self.inner.writer.lock().await;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        epoch_in(&mut tx, &request.account_id, &request.authorization_epoch).await?;
        let activity_updated_at: Option<String> = sqlx::query_scalar(
            "SELECT n.updated_at FROM items n WHERE n.account_id=? AND n.id=? AND n.kind='notification' AND EXISTS(SELECT 1 FROM scope_membership m WHERE m.account_id=n.account_id AND m.scope='notifications' AND m.entity_id=n.id AND m.active=1) AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=n.account_id AND s.scope='notifications' AND s.access_denied=1)",
        )
        .bind(&request.account_id)
        .bind(&request.notification_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        let activity_updated_at = activity_updated_at.ok_or_else(not_found)?;
        let current = sqlx::query(
            "SELECT notification_id AS local_id,disposition AS local_disposition,bookmarked AS local_bookmarked,snoozed_until AS local_snoozed_until,activity_updated_at AS local_activity_updated_at,generation AS local_generation FROM local_inbox_state WHERE account_id=? AND notification_id=?",
        )
        .bind(&request.account_id)
        .bind(&request.notification_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?
        .as_ref()
        .map(saved_from_row)
        .transpose()?
        .flatten();
        if current.as_ref().map_or(0, |state| state.generation) != expected_generation {
            return Err(stale());
        }
        let generation = expected_generation
            .checked_add(1)
            .ok_or_else(CollaborationError::storage)?;
        let saved = match request.mutation {
            LocalInboxMutation::Disposition => SavedLocalInboxState {
                disposition: request.disposition.ok_or_else(|| {
                    CollaborationError::invalid("Disposition mutation is incomplete")
                })?,
                bookmarked: current.as_ref().is_some_and(|state| state.bookmarked),
                snoozed_until: requested_snooze,
                activity_updated_at: activity_updated_at.clone(),
                generation,
            },
            LocalInboxMutation::Bookmark => SavedLocalInboxState {
                disposition: current
                    .as_ref()
                    .map_or(LocalInboxDisposition::Inbox, |state| state.disposition),
                bookmarked: request.bookmarked.ok_or_else(|| {
                    CollaborationError::invalid("Bookmark mutation is incomplete")
                })?,
                snoozed_until: current
                    .as_ref()
                    .and_then(|state| state.snoozed_until.clone()),
                activity_updated_at: current.as_ref().map_or_else(
                    || activity_updated_at.clone(),
                    |state| state.activity_updated_at.clone(),
                ),
                generation,
            },
        };
        sqlx::query("INSERT INTO local_inbox_state(account_id,notification_id,disposition,bookmarked,snoozed_until,activity_updated_at,generation) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,notification_id) DO UPDATE SET disposition=excluded.disposition,bookmarked=excluded.bookmarked,snoozed_until=excluded.snoozed_until,activity_updated_at=excluded.activity_updated_at,generation=excluded.generation")
            .bind(&request.account_id)
            .bind(&request.notification_id)
            .bind(disposition_tag(saved.disposition))
            .bind(saved.bookmarked)
            .bind(&saved.snoozed_until)
            .bind(&saved.activity_updated_at)
            .bind(generation)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        sqlx::query("INSERT INTO local_inbox_projection(account_id,revision) VALUES(?,1) ON CONFLICT(account_id) DO UPDATE SET revision=revision+1")
            .bind(&request.account_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            &request.account_id,
            positive_revision(&request.authorization_epoch)?,
            &format!("local_inbox:{}", request.notification_id),
            false,
        )
        .await?;
        let (_, authorization_view) = metadata(&mut tx).await?;
        let state = projected_state(&activity_updated_at, Some(saved), now)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(LocalInboxWriteReceipt {
            state,
            revision,
            authorization_view,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_is_expired_at_the_exact_utc_instant() {
        let deadline = parse_time("2026-10-07T12:00:00Z").unwrap();
        let saved = SavedLocalInboxState {
            disposition: LocalInboxDisposition::Inbox,
            bookmarked: true,
            snoozed_until: Some(canonical_time(deadline)),
            activity_updated_at: "2026-10-07T10:00:00Z".into(),
            generation: 1,
        };
        let before = projected_state(
            "2026-10-07T10:00:00Z",
            Some(saved.clone()),
            deadline - chrono::Duration::nanoseconds(1),
        )
        .unwrap();
        assert_eq!(
            before.effective_disposition,
            LocalInboxEffectiveDisposition::Snoozed
        );
        let exact = projected_state("2026-10-07T10:00:00Z", Some(saved), deadline).unwrap();
        assert_eq!(
            exact.effective_disposition,
            LocalInboxEffectiveDisposition::Inbox
        );
        assert!(exact.bookmarked);
    }
}
