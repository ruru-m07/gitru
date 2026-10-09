//! Version-two issue creation proof validation. Catalogs are evictable and confer
//! no authority on imported commands; restore retains immutable intent in quarantine.
use super::*;
use crate::{ProviderKind, issue_metadata::native as n};

async fn refuse(db: &mut SqliteConnection, sql: &'static str) -> Result<()> {
    if sqlx::query(sql)
        .fetch_optional(db)
        .await
        .map_err(|_| invalid_backup())?
        .is_some()
    {
        return Err(invalid_backup());
    }
    Ok(())
}
fn payload(row: &SqliteRow) -> Result<n::PayloadV2> {
    n::decode_payload(
        &column::<Vec<u8>>(row, "payload_bytes")?,
        &column::<String>(row, "account_id")?,
        &column::<String>(row, "command_id")?,
        &column::<String>(row, "target_id")?,
        &column::<i64>(row, "authorization_epoch")?.to_string(),
    )
    .map_err(|_| invalid_backup())
}
fn hash(row: &SqliteRow) -> Result<String> {
    Ok(column::<Vec<u8>>(row, "submission_hash")?
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn account(row: &SqliteRow) -> Result<RemoteAccount> {
    let a: RemoteAccount = serde_json::from_str(&column::<String>(row, "account_json")?)
        .map_err(|_| invalid_backup())?;
    if a.provider != ProviderKind::Github
        || a.host != "github.com"
        || a.id != column::<String>(row, "account_id")?
    {
        return Err(invalid_backup());
    }
    Ok(a)
}
fn selection(raw: Option<String>) -> Result<n::SelectionV2> {
    let v = raw
        .map(|s| n::decode_json::<n::SelectionV2>(s.as_bytes()))
        .transpose()
        .map_err(|_| invalid_backup())?
        .unwrap_or_default();
    v.validate().map_err(|_| invalid_backup())?;
    Ok(v)
}

pub(super) async fn verify(db: &mut SqliteConnection) -> Result<()> {
    {
        let mut rows =
            sqlx::query("SELECT metadata_json FROM issue_draft_metadata").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            selection(Some(column(&row, "metadata_json")?))?;
        }
    }
    // A v1 submission was sealed without metadata. Later edits may add metadata,
    // but the very same authored generation cannot acquire unsent selections.
    {
        let mut rows = sqlx::query("SELECT m.metadata_json FROM issue_submissions s JOIN commands c USING(account_id,command_id) JOIN issue_drafts d USING(account_id,draft_id) JOIN issue_draft_metadata m USING(account_id,draft_id) WHERE c.payload_version=1 AND s.draft_generation=d.generation").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            if !selection(Some(column(&row, "metadata_json")?))?.is_empty() {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db,"SELECT 1 FROM commands c LEFT JOIN issue_submissions s USING(account_id,command_id) WHERE c.operation_kind='github.create_issue' AND c.payload_version=2 AND s.command_id IS NULL LIMIT 1").await?;
    {
        let mut rows = sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.payload_bytes,a.json AS account_json,s.draft_id,s.draft_generation,s.content_hash,d.title,d.body,d.generation,m.metadata_json FROM issue_submissions s JOIN commands c USING(account_id,command_id) JOIN accounts a ON a.id=c.account_id JOIN issue_drafts d USING(account_id,draft_id) LEFT JOIN issue_draft_metadata m USING(account_id,draft_id) WHERE c.payload_version=2").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let p = payload(&row)?;
            account(&row)?;
            if p.request.draft_id != column::<String>(&row, "draft_id")?
                || p.request.draft_generation
                    != column::<i64>(&row, "draft_generation")?.to_string()
                || n::content_hash(&p.title, &p.body, &p.metadata).map_err(|_| invalid_backup())?
                    != column::<Vec<u8>>(&row, "content_hash")?
                || column::<i64>(&row, "generation")? == column::<i64>(&row, "draft_generation")?
                    && (p.title != column::<String>(&row, "title")?
                        || p.body != column::<String>(&row, "body")?
                        || p.metadata != selection(column(&row, "metadata_json")?)?)
            {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db,"SELECT 1 FROM commands c WHERE c.operation_kind='github.create_issue' AND c.payload_version=2 AND c.state IN ('accepted','rejected','superseded') LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM commands c WHERE c.operation_kind='github.create_issue' AND c.payload_version=2 AND c.state='conflict' AND NOT EXISTS(SELECT 1 FROM delivery_resolutions d JOIN command_evidence e ON e.account_id=d.account_id AND e.command_id=d.command_id AND e.ordinal=d.evidence_ordinal WHERE d.account_id=c.account_id AND d.command_id=c.command_id AND d.purpose='conflict' AND e.kind='github.issue_creation_declined' AND e.version=2) LIMIT 1").await?;
    // This non-idempotent codec has one possible POST, with a durable exact base.
    refuse(db,"SELECT 1 FROM delivery_attempts a JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_attempt_context x USING(account_id,command_id,attempt_number) WHERE c.operation_kind='github.create_issue' AND c.payload_version=2 AND (a.attempt_number<>1 OR x.command_id IS NULL OR a.authorization_epoch<>c.authorization_epoch) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.payload_bytes,c.submission_hash,a.json AS account_json,x.execution_base FROM delivery_attempt_context x JOIN commands c USING(account_id,command_id) JOIN accounts a ON a.id=c.account_id WHERE c.operation_kind='github.create_issue' AND c.payload_version=2").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let p = payload(&row)?;
            let prep: n::PreparationV2 =
                n::decode_json(&column::<Vec<u8>>(&row, "execution_base")?)
                    .map_err(|_| invalid_backup())?;
            if !n::preparation_matches(&prep, &p)
                || prep.actor != account(&row)?.actor_id
                || prep.command_hash != hash(&row)?
            {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db,"SELECT 1 FROM delivery_resolutions r JOIN commands c USING(account_id,command_id) JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.evidence_ordinal WHERE c.operation_kind='github.create_issue' AND c.payload_version=2 AND (r.purpose NOT IN ('confirmed','conflict') OR e.version<>2 OR r.purpose='confirmed' AND e.kind<>'github.issue_created' OR r.purpose='conflict' AND e.kind<>'github.issue_creation_declined') LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions r ON r.account_id=e.account_id AND r.command_id=e.command_id AND r.evidence_ordinal=e.ordinal AND r.purpose='conflict' WHERE e.kind='github.issue_creation_declined' AND e.version=2 AND (c.operation_kind<>'github.create_issue' OR c.payload_version<>2 OR c.state NOT IN ('conflict','cancelled') OR e.attempt_number IS NOT NULL OR r.command_id IS NULL OR EXISTS(SELECT 1 FROM delivery_attempts a WHERE a.account_id=c.account_id AND a.command_id=c.command_id)) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.payload_bytes,c.submission_hash,a.json AS account_json,e.payload FROM command_evidence e JOIN commands c USING(account_id,command_id) JOIN accounts a ON a.id=c.account_id WHERE e.kind='github.issue_creation_declined' AND e.version=2").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let p = payload(&row)?;
            let proof: n::DeclinedV2 = n::decode_json(&column::<Vec<u8>>(&row, "payload")?)
                .map_err(|_| invalid_backup())?;
            if !n::declined_matches(&proof, &p)
                || proof.actor != account(&row)?.actor_id
                || proof.command_hash != hash(&row)?
            {
                return Err(invalid_backup());
            }
        }
    }
    // Confirmation, exact referenced proof and authored identity mapping are one
    // transaction. Validate every direction, not just existence of some proof.
    refuse(db,"SELECT 1 FROM issue_resolutions r JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=r.account_id AND d.command_id=r.command_id AND d.purpose='confirmed' LEFT JOIN command_evidence e ON e.account_id=d.account_id AND e.command_id=d.command_id AND e.ordinal=d.evidence_ordinal AND e.kind='github.issue_created' AND e.version=2 WHERE c.payload_version=2 AND (c.state<>'confirmed' OR d.command_id IS NULL OR e.command_id IS NULL) LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM commands c LEFT JOIN issue_resolutions r USING(account_id,command_id) WHERE c.operation_kind='github.create_issue' AND c.payload_version=2 AND c.state='confirmed' AND r.command_id IS NULL LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN issue_resolutions r USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=e.account_id AND d.command_id=e.command_id AND d.evidence_ordinal=e.ordinal AND d.purpose='confirmed' WHERE e.kind='github.issue_created' AND e.version=2 AND (c.operation_kind<>'github.create_issue' OR c.payload_version<>2 OR c.state<>'confirmed' OR r.command_id IS NULL OR d.command_id IS NULL OR e.attempt_number IS NULL) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.payload_bytes,c.submission_hash,a.json AS account_json,e.payload,x.execution_base,r.draft_id,r.entity_id,r.provider_id,r.number,r.url FROM command_evidence e JOIN commands c USING(account_id,command_id) JOIN accounts a ON a.id=c.account_id LEFT JOIN issue_resolutions r USING(account_id,command_id) LEFT JOIN delivery_attempt_context x ON x.account_id=e.account_id AND x.command_id=e.command_id AND x.attempt_number=e.attempt_number WHERE e.kind='github.issue_created' AND e.version=2").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let p = payload(&row)?;
            let proof: n::ReceiptV2 = n::decode_json(&column::<Vec<u8>>(&row, "payload")?)
                .map_err(|_| invalid_backup())?;
            if !n::receipt_matches(&proof, &p)
                || proof.preparation.actor != account(&row)?.actor_id
                || proof.preparation.command_hash != hash(&row)?
                || n::encode(&proof.preparation).map_err(|_| invalid_backup())?
                    != column::<Vec<u8>>(&row, "execution_base")?
                || p.request.draft_id != column::<String>(&row, "draft_id")?
                || format!("github:issue:{}", proof.core.provider_id)
                    != column::<String>(&row, "entity_id")?
                || proof.core.provider_id != column::<String>(&row, "provider_id")?
                || proof.core.number != column::<String>(&row, "number")?
                || proof.core.web_url != column::<String>(&row, "url")?
            {
                return Err(invalid_backup());
            }
        }
    }
    Ok(())
}
