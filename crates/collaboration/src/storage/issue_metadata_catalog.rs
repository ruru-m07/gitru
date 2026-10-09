//! Bounded, evictable metadata observations. Catalog reads never confer write authority.
use super::*;
use crate::{
    DetailFreshness, IssueMetadataAvailability, IssueMetadataKind, IssueMetadataOption,
    IssueMetadataPage, IssueMetadataQuery, IssueMetadataReason, IssueMetadataReference,
    issue_metadata::native as n,
    providers::{IssueMetadataCatalogPage, IssueMetadataCatalogRequest, IssueMetadataReadContext},
};
use std::collections::{BTreeSet, HashSet};

const MAX_PAGE: usize = 100;
const MAX_PAGES: u32 = 20;
const MAX_CURSOR: usize = 16_384;
const FRESH_SECONDS: i64 = 300;

#[derive(Debug, Clone)]
pub(crate) struct CatalogLease {
    pub request: IssueMetadataCatalogRequest,
    pub page_count: u32,
    pub catalog_revision: String,
}
#[derive(Debug)]
pub(crate) struct CatalogApplyReceipt {
    pub revision: String,
    pub next_lease: Option<CatalogLease>,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    actor: String,
    provider: ProviderKind,
    host: String,
    native_repository: String,
    path: String,
}
impl Binding {
    fn from(c: &IssueMetadataReadContext) -> Self {
        Self {
            actor: c.account.actor_id.clone(),
            provider: c.account.provider,
            host: c.account.host.clone(),
            native_repository: c.repository.provider_id.clone(),
            path: c.repository.full_name.clone(),
        }
    }
}
struct Header {
    epoch: String,
    view: String,
    binding: Binding,
    generation: i64,
    revision: i64,
    cursor: Option<String>,
    pages: u32,
    complete: bool,
    truncated: bool,
    evicted: bool,
    observed: Option<String>,
    sync: SyncStatus,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogCursor {
    version: u32,
    query: String,
    epoch: String,
    view: String,
    revision: String,
    key: String,
    id: String,
}
pub(crate) fn scope(repository: &str, kind: IssueMetadataKind) -> String {
    format!("repository_metadata:{repository}:{}", name(kind))
}
fn name(kind: IssueMetadataKind) -> &'static str {
    match kind {
        IssueMetadataKind::Labels => "labels",
        IssueMetadataKind::Assignees => "assignees",
        IssueMetadataKind::Milestones => "milestones",
    }
}
fn kind(s: &str) -> Result<IssueMetadataKind> {
    match s {
        "labels" => Ok(IssueMetadataKind::Labels),
        "assignees" => Ok(IssueMetadataKind::Assignees),
        "milestones" => Ok(IssueMetadataKind::Milestones),
        _ => Err(CollaborationError::storage()),
    }
}
fn stale() -> CollaborationError {
    CollaborationError::new(ErrorCode::StaleView, "Metadata catalog authority changed")
}
fn validate_key(account: &str, epoch: &str, repository: &str) -> Result<()> {
    validate_identifier(account)?;
    validate_identifier(repository)?;
    crate::issue_creation::native::revision(epoch, true)?;
    Ok(())
}
fn timestamp(value: &str) -> Result<String> {
    if value.len() > 128 {
        return Err(n::invalid());
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|v| {
            v.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        })
        .map_err(|_| n::invalid())
}
fn denied(sync: &SyncStatus) -> bool {
    sync.state == SyncState::AuthRequired
        || sync.error.as_ref().is_some_and(|e| {
            matches!(
                e.code,
                ErrorCode::AuthRequired | ErrorCode::PermissionDenied
            )
        })
}
async fn context_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    epoch: &str,
    repository: &str,
) -> Result<IssueMetadataReadContext> {
    validate_key(account, epoch, repository)?;
    let a = account_in(tx, account, true).await?;
    if a.authorization_epoch != epoch {
        return Err(stale());
    }
    let selected: bool = sqlx::query_scalar(
        "SELECT coalesce((SELECT selected FROM repositories WHERE account_id=? AND id=?),0)",
    )
    .bind(account)
    .bind(repository)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_error)?;
    if !selected {
        return Err(stale());
    }
    let f = super::issue_creation::capture_in(tx, &a, repository).await?;
    n::positive(&a.actor_id)?;
    n::positive(&f.repository.provider_id)?;
    Ok(IssueMetadataReadContext {
        account: a,
        repository: f.repository,
        authorization_view: f.authorization_view,
    })
}
async fn header_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    repository: &str,
    family: IssueMetadataKind,
) -> Result<Option<Header>> {
    let row = sqlx::query("SELECT authorization_epoch,authorization_view,binding_json,generation,catalog_revision,cursor,pages,complete,truncated,evicted,observed_at,sync_json FROM repository_metadata_catalogs WHERE account_id=? AND repository_id=? AND kind=?")
        .bind(account).bind(repository).bind(name(family)).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    row.map(|r| {
        Ok(Header {
            epoch: r.get("authorization_epoch"),
            view: r.get("authorization_view"),
            binding: decode(r.get("binding_json"))?,
            generation: r.get("generation"),
            revision: r.get("catalog_revision"),
            cursor: r.get("cursor"),
            pages: r.get::<i64, _>("pages") as u32,
            complete: r.get("complete"),
            truncated: r.get("truncated"),
            evicted: r.get("evicted"),
            observed: r.get("observed_at"),
            sync: decode(r.get("sync_json"))?,
        })
    })
    .transpose()
}
fn matches_context(h: &Header, c: &IssueMetadataReadContext) -> bool {
    h.epoch == c.account.authorization_epoch
        && h.view == c.authorization_view
        && h.binding == Binding::from(c)
}
fn lease(c: IssueMetadataReadContext, family: IssueMetadataKind, h: &Header) -> CatalogLease {
    CatalogLease {
        request: IssueMetadataCatalogRequest {
            context: c,
            kind: family,
            catalog_generation: h.generation.to_string(),
            cursor: h.cursor.clone(),
        },
        page_count: h.pages,
        catalog_revision: h.revision.to_string(),
    }
}
async fn lease_in(tx: &mut Transaction<'_, Sqlite>, l: &CatalogLease) -> Result<Header> {
    let r = &l.request;
    let c = &r.context;
    validate_key(
        &c.account.id,
        &c.account.authorization_epoch,
        &c.repository.id,
    )?;
    crate::issue_creation::native::revision(&r.catalog_generation, true)?;
    crate::issue_creation::native::revision(&l.catalog_revision, true)?;
    if r.cursor
        .as_ref()
        .is_some_and(|v| v.is_empty() || v.len() > MAX_CURSOR)
        || l.page_count >= MAX_PAGES
    {
        return Err(stale());
    }
    let current = context_in(
        tx,
        &c.account.id,
        &c.account.authorization_epoch,
        &c.repository.id,
    )
    .await?;
    let h = header_in(tx, &c.account.id, &c.repository.id, r.kind)
        .await?
        .ok_or_else(stale)?;
    if !matches_context(&h, c)
        || !matches_context(&h, &current)
        || c.repository.account_id != c.account.id
        || h.generation.to_string() != r.catalog_generation
        || h.cursor != r.cursor
        || h.pages != l.page_count
        || h.revision.to_string() != l.catalog_revision
        || h.complete
    {
        return Err(stale());
    }
    Ok(h)
}
fn validate_option(v: &IssueMetadataOption, family: IssueMetadataKind) -> Result<(&str, String)> {
    let (id, display) = match (&v.reference, family) {
        (IssueMetadataReference::Label(v), IssueMetadataKind::Labels) => {
            n::label(v)?;
            (&v.provider_id, &v.name)
        }
        (IssueMetadataReference::Assignee(v), IssueMetadataKind::Assignees) => {
            n::assignee(v)?;
            (&v.provider_id, &v.login)
        }
        (IssueMetadataReference::Milestone(v), IssueMetadataKind::Milestones) => {
            n::milestone(v)?;
            (&v.provider_id, &v.title)
        }
        _ => return Err(n::invalid()),
    };
    match (v.availability, v.reason) {
        (IssueMetadataAvailability::Available, None)
        | (IssueMetadataAvailability::Unknown, Some(IssueMetadataReason::Unobserved)) => {}
        (IssueMetadataAvailability::Unavailable, Some(reason))
            if matches!(
                (family, reason),
                (IssueMetadataKind::Labels, IssueMetadataReason::Archived)
                    | (IssueMetadataKind::Milestones, IssueMetadataReason::Closed)
            ) => {}
        _ => return Err(n::invalid()),
    }
    let key = display.to_lowercase();
    if key.len() > 4096 || encode(v)?.len() > MAX_CURSOR {
        return Err(n::invalid());
    }
    Ok((id, key))
}

impl Store {
    pub(crate) async fn begin_issue_metadata(
        &self,
        account: &str,
        epoch: &str,
        repository: &str,
        family: IssueMetadataKind,
    ) -> Result<CatalogLease> {
        validate_key(account, epoch, repository)?;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let c = context_in(&mut tx, account, epoch, repository).await?;
        let old = header_in(&mut tx, account, repository, family).await?;
        let keep = old.as_ref().is_some_and(|h| matches_context(h, &c));
        if !keep {
            sqlx::query("DELETE FROM repository_metadata_catalogs WHERE account_id=? AND repository_id=? AND kind=?")
                .bind(account).bind(repository).bind(name(family)).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let revision = record_change(
            &mut tx,
            account,
            positive_revision(epoch)?,
            &scope(repository, family),
            false,
        )
        .await?;
        let sync = SyncStatus {
            state: SyncState::Syncing,
            last_success_at: old
                .as_ref()
                .filter(|_| keep)
                .and_then(|h| h.sync.last_success_at.clone()),
            error: old.as_ref().filter(|h| keep && denied(&h.sync)).map(|_| {
                CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "Metadata catalog access requires validation",
                )
            }),
            ..SyncStatus::default()
        };
        sqlx::query("INSERT INTO repository_metadata_catalogs(account_id,repository_id,kind,authorization_epoch,authorization_view,binding_json,generation,catalog_revision,cursor,pages,complete,truncated,observed_at,sync_json) VALUES(?,?,?,?,?,?,?,?,NULL,0,0,0,NULL,?) ON CONFLICT(account_id,repository_id,kind) DO UPDATE SET generation=excluded.generation,catalog_revision=excluded.catalog_revision,cursor=NULL,pages=0,complete=0,truncated=0,evicted=0,sync_json=excluded.sync_json")
            .bind(account).bind(repository).bind(name(family)).bind(epoch).bind(&c.authorization_view).bind(encode(&Binding::from(&c))?).bind(positive_revision(&revision)?).bind(positive_revision(&revision)?).bind(encode(&sync)?)
            .execute(&mut *tx).await.map_err(storage_error)?;
        bound_in(&mut tx, account, epoch).await?;
        if metadata(&mut tx).await?.0 != revision {
            // A begin receipt uses its current catalog revision as the change
            // hint; include sibling eviction hints committed by this operation.
            let final_revision = record_change(
                &mut tx,
                account,
                positive_revision(epoch)?,
                &scope(repository, family),
                false,
            )
            .await?;
            sqlx::query("UPDATE repository_metadata_catalogs SET catalog_revision=? WHERE account_id=? AND repository_id=? AND kind=?")
                .bind(positive_revision(&final_revision)?).bind(account).bind(repository).bind(name(family)).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let h = header_in(&mut tx, account, repository, family)
            .await?
            .ok_or_else(stale)?;
        let result = lease(c, family, &h);
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
    pub(crate) async fn resume_issue_metadata(
        &self,
        account: &str,
        epoch: &str,
        repository: &str,
        family: IssueMetadataKind,
    ) -> Result<Option<CatalogLease>> {
        validate_key(account, epoch, repository)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let c = context_in(&mut tx, account, epoch, repository).await?;
        let h = header_in(&mut tx, account, repository, family).await?;
        let result = h
            .filter(|h| matches_context(h, &c) && !h.complete && h.pages < MAX_PAGES)
            .map(|h| lease(c, family, &h));
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
    pub(crate) async fn issue_metadata_request(
        &self,
        l: &CatalogLease,
    ) -> Result<IssueMetadataCatalogRequest> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        lease_in(&mut tx, l).await?;
        let mut r = l.request.clone();
        r.context = context_in(
            &mut tx,
            &r.context.account.id,
            &r.context.account.authorization_epoch,
            &r.context.repository.id,
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(r)
    }
    pub(crate) async fn apply_issue_metadata(
        &self,
        l: &CatalogLease,
        page: IssueMetadataCatalogPage,
        observed_at: &str,
    ) -> Result<CatalogApplyReceipt> {
        let observed = timestamp(observed_at)?;
        if l.page_count >= MAX_PAGES
            || page.options.len() > MAX_PAGE
            || page.coverage != CoverageState::Partial
            || page.next_cursor.as_ref().is_some_and(|s| {
                s.is_empty() || s.len() > MAX_CURSOR || Some(s) == l.request.cursor.as_ref()
            })
            || page.truncated && page.next_cursor.is_some()
            || l.page_count + 1 >= MAX_PAGES && page.next_cursor.is_some()
        {
            return Err(n::invalid());
        }
        let mut ids = HashSet::new();
        for v in &page.options {
            if !ids.insert(validate_option(v, l.request.kind)?.0) {
                return Err(n::invalid());
            }
        }
        let r = &l.request;
        let a = &r.context.account;
        let repo = &r.context.repository.id;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        lease_in(&mut tx, l).await?;
        let revision = record_change(
            &mut tx,
            &a.id,
            positive_revision(&a.authorization_epoch)?,
            &scope(repo, r.kind),
            false,
        )
        .await?;
        for v in &page.options {
            let (id, search) = validate_option(v, r.kind)?;
            sqlx::query("INSERT INTO repository_metadata_options(account_id,repository_id,kind,provider_id,option_json,search_key,seen_generation,data_revision,observed_at) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(account_id,repository_id,kind,provider_id) DO UPDATE SET option_json=excluded.option_json,search_key=excluded.search_key,seen_generation=excluded.seen_generation,data_revision=excluded.data_revision,observed_at=excluded.observed_at")
                .bind(&a.id).bind(repo).bind(name(r.kind)).bind(id).bind(encode(v)?).bind(search).bind(positive_revision(&r.catalog_generation)?).bind(positive_revision(&revision)?).bind(&observed).execute(&mut *tx).await.map_err(storage_error)?;
        }
        let sync = SyncStatus {
            state: if page.next_cursor.is_some() {
                SyncState::Syncing
            } else {
                SyncState::Idle
            },
            last_success_at: Some(observed.clone()),
            ..SyncStatus::default()
        };
        sqlx::query("UPDATE repository_metadata_catalogs SET catalog_revision=?,cursor=?,pages=pages+1,complete=?,truncated=?,observed_at=?,sync_json=? WHERE account_id=? AND repository_id=? AND kind=?")
            .bind(positive_revision(&revision)?).bind(&page.next_cursor).bind(page.next_cursor.is_none()).bind(page.truncated).bind(&observed).bind(encode(&sync)?).bind(&a.id).bind(repo).bind(name(r.kind)).execute(&mut *tx).await.map_err(storage_error)?;
        bound_in(&mut tx, &a.id, &a.authorization_epoch).await?;
        let revision = metadata(&mut tx).await?.0;
        let next_lease = if page.next_cursor.is_some() {
            let h = header_in(&mut tx, &a.id, repo, r.kind)
                .await?
                .ok_or_else(stale)?;
            Some(lease(r.context.clone(), r.kind, &h))
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(CatalogApplyReceipt {
            revision,
            next_lease,
        })
    }
    pub(crate) async fn fail_issue_metadata(
        &self,
        l: &CatalogLease,
        mut sync: SyncStatus,
    ) -> Result<String> {
        if matches!(sync.state, SyncState::Idle | SyncState::Syncing)
            || encode(&sync)?.len() > MAX_CURSOR
        {
            return Err(n::invalid());
        }
        if let Some(t) = &sync.next_retry_at {
            timestamp(t)?;
        }
        let a = &l.request.context.account;
        let repo = &l.request.context.repository.id;
        let mut writer = self.inner.writer.acquire().await?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let h = lease_in(&mut tx, l).await?;
        sync.last_success_at = h.sync.last_success_at;
        let revision = record_change(
            &mut tx,
            &a.id,
            positive_revision(&a.authorization_epoch)?,
            &scope(repo, l.request.kind),
            false,
        )
        .await?;
        sqlx::query("UPDATE repository_metadata_catalogs SET catalog_revision=?,sync_json=? WHERE account_id=? AND repository_id=? AND kind=?")
            .bind(positive_revision(&revision)?).bind(encode(&sync)?).bind(&a.id).bind(repo).bind(name(l.request.kind)).execute(&mut *tx).await.map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
    pub async fn issue_metadata_options(&self, q: IssueMetadataQuery) -> Result<IssueMetadataPage> {
        validate_identifier(&q.account_id)?;
        validate_identifier(&q.repository_id)?;
        if !(1..=50).contains(&q.limit)
            || q.search.len() > 1024
            || q.search.chars().any(char::is_control)
            || q.cursor.as_ref().is_some_and(|s| s.len() > MAX_CURSOR)
        {
            return Err(n::invalid());
        }
        let search = q.search.to_lowercase();
        let query_hash = format!(
            "{:x}",
            Sha256::digest(encode(&(&q.account_id, &q.repository_id, q.kind, &search))?.as_bytes())
        );
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let a = account_in(&mut tx, &q.account_id, true).await?;
        let c = context_in(&mut tx, &a.id, &a.authorization_epoch, &q.repository_id).await?;
        let h = header_in(&mut tx, &a.id, &q.repository_id, q.kind)
            .await?
            .filter(|h| matches_context(h, &c));
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let mut page = IssueMetadataPage {
            account_id: q.account_id.clone(),
            repository_id: q.repository_id.clone(),
            kind: q.kind,
            options: vec![],
            next_cursor: None,
            coverage: missing_coverage(),
            freshness: DetailFreshness::Unknown,
            sync: SyncStatus::default(),
            revision,
            authorization_view,
            catalog_revision: h.as_ref().map(|h| h.revision.to_string()),
        };
        let Some(h) = h else {
            if q.cursor.is_some() {
                return Err(stale());
            }
            tx.commit().await.map_err(storage_error)?;
            return Ok(page);
        };
        if denied(&h.sync) {
            if q.cursor.is_some() {
                return Err(stale());
            }
            page.sync = h.sync;
            tx.commit().await.map_err(storage_error)?;
            return Ok(page);
        }
        let after: Option<CatalogCursor> = q
            .cursor
            .as_ref()
            .map(|raw| -> Result<_> {
                let cursor: CatalogCursor = serde_json::from_str(raw).map_err(|_| n::invalid())?;
                if cursor.version != 1
                    || cursor.query != query_hash
                    || cursor.epoch != a.authorization_epoch
                    || cursor.view != page.authorization_view
                    || cursor.revision != h.revision.to_string()
                    || cursor.key.len() > 4096
                    || n::positive(&cursor.id).is_err()
                    || encode(&cursor)? != *raw
                {
                    return Err(stale());
                }
                Ok(cursor)
            })
            .transpose()?;
        let rows = sqlx::query("SELECT option_json,search_key,provider_id FROM repository_metadata_options WHERE account_id=? AND repository_id=? AND kind=? AND instr(search_key,?)>0 AND (? IS NULL OR (search_key,provider_id)>(?,?)) ORDER BY search_key,provider_id LIMIT ?")
            .bind(&a.id).bind(&q.repository_id).bind(name(q.kind)).bind(&search).bind(after.as_ref().map(|v| &v.key)).bind(after.as_ref().map(|v| &v.key)).bind(after.as_ref().map(|v| &v.id)).bind(i64::from(q.limit)+1).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let more = rows.len() > q.limit as usize;
        for row in rows.iter().take(q.limit as usize) {
            let value: IssueMetadataOption = decode(row.get("option_json"))?;
            validate_option(&value, q.kind)?;
            page.options.push(value);
        }
        if more && let Some(last) = rows.get(q.limit as usize - 1) {
            page.next_cursor = Some(encode(&CatalogCursor {
                version: 1,
                query: query_hash,
                epoch: a.authorization_epoch.clone(),
                view: page.authorization_view.clone(),
                revision: h.revision.to_string(),
                key: last.get("search_key"),
                id: last.get("provider_id"),
            })?);
        }
        page.coverage = Coverage {
            state: if h.observed.is_some() {
                CoverageState::Partial
            } else {
                CoverageState::Missing
            },
            validated_at: h.observed.clone(),
            remote_has_more: h.cursor.is_some() || h.truncated,
        };
        let retained: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repository_metadata_options WHERE account_id=? AND repository_id=? AND kind=? AND seen_generation<>?)")
            .bind(&a.id).bind(&q.repository_id).bind(name(q.kind)).bind(h.generation).fetch_one(&mut *tx).await.map_err(storage_error)?;
        page.freshness = if h.observed.is_none() {
            DetailFreshness::Unknown
        } else if h.complete
            && !h.truncated
            && !h.evicted
            && !retained
            && h.sync.state == SyncState::Idle
            && h.observed
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .is_some_and(|t| {
                    let seconds = chrono::Utc::now().signed_duration_since(t).num_seconds();
                    (0..FRESH_SECONDS).contains(&seconds)
                })
        {
            DetailFreshness::Fresh
        } else {
            DetailFreshness::Stale
        };
        page.sync = h.sync;
        tx.commit().await.map_err(storage_error)?;
        Ok(page)
    }
}

/// Retention is local eviction, never evidence of remote deletion.
async fn bound_in(tx: &mut Transaction<'_, Sqlite>, account: &str, epoch: &str) -> Result<()> {
    let mut affected = BTreeSet::new();
    let headers = sqlx::query("DELETE FROM repository_metadata_catalogs WHERE rowid IN(SELECT rowid FROM repository_metadata_catalogs WHERE account_id=? ORDER BY catalog_revision DESC,repository_id,kind LIMIT -1 OFFSET 48) RETURNING repository_id,kind")
        .bind(account).fetch_all(&mut **tx).await.map_err(storage_error)?;
    for row in headers {
        affected.insert((
            row.get::<String, _>("repository_id"),
            row.get::<String, _>("kind"),
        ));
    }
    // At most 48 retained headers; each family is bounded independently before
    // applying the shared account budget.
    let families: Vec<(String,String)> = sqlx::query_as("SELECT repository_id,kind FROM repository_metadata_catalogs WHERE account_id=? ORDER BY repository_id,kind LIMIT 48")
        .bind(account).fetch_all(&mut **tx).await.map_err(storage_error)?;
    for (repo, family) in families {
        let count = sqlx::query("DELETE FROM repository_metadata_options WHERE rowid IN(SELECT rowid FROM repository_metadata_options WHERE account_id=? AND repository_id=? AND kind=? ORDER BY data_revision DESC,provider_id LIMIT -1 OFFSET 2000)")
            .bind(account).bind(&repo).bind(&family).execute(&mut **tx).await.map_err(storage_error)?.rows_affected();
        if count > 0 {
            affected.insert((repo, family));
        }
    }
    let rows = sqlx::query("DELETE FROM repository_metadata_options WHERE rowid IN(SELECT rowid FROM repository_metadata_options WHERE account_id=? ORDER BY data_revision DESC,repository_id,kind,provider_id LIMIT -1 OFFSET 6000) RETURNING repository_id,kind")
        .bind(account).fetch_all(&mut **tx).await.map_err(storage_error)?;
    for row in rows {
        affected.insert((
            row.get::<String, _>("repository_id"),
            row.get::<String, _>("kind"),
        ));
    }
    for (repo, family) in affected {
        let revision = record_change(
            tx,
            account,
            positive_revision(epoch)?,
            &scope(&repo, kind(&family)?),
            false,
        )
        .await?;
        sqlx::query("UPDATE repository_metadata_catalogs SET catalog_revision=?,evicted=1 WHERE account_id=? AND repository_id=? AND kind=?")
            .bind(positive_revision(&revision)?).bind(account).bind(repo).bind(family).execute(&mut **tx).await.map_err(storage_error)?;
    }
    Ok(())
}

pub(super) async fn cleanup_abandoned_in(connection: &mut SqliteConnection) -> Result<()> {
    let mut tx = connection.begin().await.map_err(storage_error)?;
    let mut after = (String::new(), String::new(), String::new());
    loop {
        let rows: Vec<(String,String,String,String)> = sqlx::query_as("SELECT account_id,repository_id,kind,authorization_epoch FROM repository_metadata_catalogs WHERE (account_id,repository_id,kind)>(?,?,?) ORDER BY account_id,repository_id,kind LIMIT 128")
            .bind(&after.0).bind(&after.1).bind(&after.2).fetch_all(&mut *tx).await.map_err(storage_error)?;
        if rows.is_empty() {
            break;
        }
        for (account, repo, family, epoch) in rows {
            let family_kind = kind(&family)?;
            let h = header_in(&mut tx, &account, &repo, family_kind)
                .await?
                .ok_or_else(stale)?;
            let current = context_in(&mut tx, &account, &epoch, &repo).await;
            let valid = match current {
                Ok(c) => matches_context(&h, &c),
                Err(e)
                    if matches!(
                        e.code,
                        ErrorCode::AuthRequired
                            | ErrorCode::StaleView
                            | ErrorCode::NotFound
                            | ErrorCode::PermissionDenied
                    ) =>
                {
                    false
                }
                Err(e) => return Err(e),
            };
            if !valid {
                sqlx::query("DELETE FROM repository_metadata_catalogs WHERE account_id=? AND repository_id=? AND kind=?").bind(&account).bind(&repo).bind(&family).execute(&mut *tx).await.map_err(storage_error)?;
            } else if h.sync.state == SyncState::Syncing {
                let mut sync = h.sync;
                sync.state = SyncState::Idle;
                let revision = record_change(
                    &mut tx,
                    &account,
                    positive_revision(&epoch)?,
                    &scope(&repo, family_kind),
                    false,
                )
                .await?;
                sqlx::query("UPDATE repository_metadata_catalogs SET catalog_revision=?,sync_json=? WHERE account_id=? AND repository_id=? AND kind=?").bind(positive_revision(&revision)?).bind(encode(&sync)?).bind(&account).bind(&repo).bind(&family).execute(&mut *tx).await.map_err(storage_error)?;
            }
            after = (account, repo, family);
        }
    }
    tx.commit().await.map_err(storage_error)
}

#[cfg(test)]
#[path = "issue_metadata_catalog_tests.rs"]
mod tests;
