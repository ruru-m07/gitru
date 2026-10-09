//! Revalidate authored PR creation without reconstructing an online send grant.
use super::*;
use crate::{ProviderKind, PullDraftKey, PullDraftValues, pull_creation::native as n};

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

fn values(row: &SqliteRow) -> Result<PullDraftValues> {
    let is_draft: i64 = column(row, "is_draft")?;
    if !matches!(is_draft, 0 | 1) {
        return Err(invalid_backup());
    }
    Ok(PullDraftValues {
        title: column(row, "title")?,
        body: column(row, "body")?,
        source_branch: column(row, "source_branch")?,
        base_branch: column(row, "base_branch")?,
        local_repository_id: column(row, "local_repository_id")?,
        link_id: column(row, "link_id")?,
        link_generation: column(row, "link_generation")?,
        is_draft: is_draft == 1,
    })
}

pub(super) async fn verify(db: &mut SqliteConnection) -> Result<()> {
    {
        let mut rows = sqlx::query("SELECT * FROM pull_drafts").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            n::validate_key(&PullDraftKey {
                account_id: column(&row, "account_id")?,
                draft_id: column(&row, "draft_id")?,
                repository_id: column(&row, "repository_id")?,
            })
            .map_err(|_| invalid_backup())?;
            n::validate_values(&values(&row)?, false).map_err(|_| invalid_backup())?;
            if column::<i64>(&row, "generation")? <= 0 {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db,"SELECT 1 FROM pull_submissions s JOIN commands c USING(account_id,command_id) JOIN pull_drafts d USING(account_id,draft_id) WHERE c.operation_kind<>'github.create_pull_request' OR c.payload_version<>1 OR c.target_kind<>'repository' OR c.target_id<>d.repository_id OR c.repository_id IS NOT d.repository_id OR d.generation<s.draft_generation LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM commands c LEFT JOIN pull_submissions s USING(account_id,command_id) WHERE c.operation_kind='github.create_pull_request' AND c.payload_version=1 AND s.command_id IS NULL LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT d.*,s.command_id,s.draft_generation,s.content_hash,c.target_id,c.authorization_epoch,c.payload_bytes,a.json AS account_json FROM pull_submissions s JOIN commands c USING(account_id,command_id) JOIN pull_drafts d USING(account_id,draft_id) JOIN accounts a ON a.id=c.account_id").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let payload = n::decode_parts(
                &column::<Vec<u8>>(&row, "payload_bytes")?,
                &column::<String>(&row, "account_id")?,
                &column::<String>(&row, "command_id")?,
                &column::<String>(&row, "target_id")?,
                &column::<i64>(&row, "authorization_epoch")?.to_string(),
            )
            .map_err(|_| invalid_backup())?;
            let account: RemoteAccount =
                serde_json::from_str(&column::<String>(&row, "account_json")?)
                    .map_err(|_| invalid_backup())?;
            if account.provider != ProviderKind::Github
                || account.host != "github.com"
                || payload.actor_id != account.actor_id
                || payload.request.context.key.draft_id != column::<String>(&row, "draft_id")?
                || payload.request.context.draft_generation
                    != column::<i64>(&row, "draft_generation")?.to_string()
                || n::content_hash(&payload.values).map_err(|_| invalid_backup())?
                    != column::<Vec<u8>>(&row, "content_hash")?
                || column::<i64>(&row, "generation")? == column::<i64>(&row, "draft_generation")?
                    && payload.values != values(&row)?
            {
                return Err(invalid_backup());
            }
        }
    }
    // This operation was introduced after attempt-context storage. Unlike an
    // unknown historical codec, every possible POST has an exact typed base.
    refuse(db,"SELECT 1 FROM delivery_attempts a JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_attempt_context x USING(account_id,command_id,attempt_number) WHERE c.operation_kind='github.create_pull_request' AND c.payload_version=1 AND (x.command_id IS NULL OR a.authorization_epoch<>c.authorization_epoch) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.payload_bytes,c.submission_hash,x.execution_base FROM delivery_attempt_context x JOIN commands c USING(account_id,command_id) WHERE c.operation_kind='github.create_pull_request' AND c.payload_version=1").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let payload = n::decode_parts(
                &column::<Vec<u8>>(&row, "payload_bytes")?,
                &column::<String>(&row, "account_id")?,
                &column::<String>(&row, "command_id")?,
                &column::<String>(&row, "target_id")?,
                &column::<i64>(&row, "authorization_epoch")?.to_string(),
            )
            .map_err(|_| invalid_backup())?;
            let prep: n::Preparation = n::decode_json(&column::<Vec<u8>>(&row, "execution_base")?)
                .map_err(|_| invalid_backup())?;
            let hash: Vec<u8> = column(&row, "submission_hash")?;
            let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
            if !n::preparation_matches(&prep, &payload) || prep.command_hash != hash {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db,"SELECT 1 FROM delivery_resolutions r JOIN commands c USING(account_id,command_id) JOIN command_evidence e ON e.account_id=r.account_id AND e.command_id=r.command_id AND e.ordinal=r.evidence_ordinal WHERE c.operation_kind='github.create_pull_request' AND c.payload_version=1 AND (r.purpose NOT IN ('confirmed','conflict') OR r.purpose='confirmed' AND (e.kind<>'github.pull_created' OR e.version<>1) OR r.purpose='conflict' AND (e.kind<>'github.pull_creation_declined' OR e.version<>1)) LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions r ON r.account_id=e.account_id AND r.command_id=e.command_id AND r.evidence_ordinal=e.ordinal AND r.purpose='conflict' WHERE e.kind='github.pull_creation_declined' AND e.version=1 AND (c.operation_kind<>'github.create_pull_request' OR c.payload_version<>1 OR r.command_id IS NULL) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.payload_bytes,c.submission_hash,e.payload,e.attempt_number,x.execution_base FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_attempt_context x ON x.account_id=e.account_id AND x.command_id=e.command_id AND x.attempt_number=e.attempt_number WHERE e.kind='github.pull_creation_declined' AND e.version=1").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let payload = n::decode_parts(
                &column::<Vec<u8>>(&row, "payload_bytes")?,
                &column::<String>(&row, "account_id")?,
                &column::<String>(&row, "command_id")?,
                &column::<String>(&row, "target_id")?,
                &column::<i64>(&row, "authorization_epoch")?.to_string(),
            )
            .map_err(|_| invalid_backup())?;
            let proof: n::DeclinedEvidence = n::decode_json(&column::<Vec<u8>>(&row, "payload")?)
                .map_err(|_| invalid_backup())?;
            let hash: Vec<u8> = column(&row, "submission_hash")?;
            let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
            if !n::declined_matches(&proof, &payload)
                || proof.preparation.command_hash != hash
                || column::<Option<i64>>(&row, "attempt_number")?.is_some()
                    && n::encode(&proof.preparation).map_err(|_| invalid_backup())?
                        != column::<Vec<u8>>(&row, "execution_base")?
            {
                return Err(invalid_backup());
            }
        }
    }
    // All three objects are produced atomically: terminal command, exact strong
    // evidence ordinal, and authored identity mapping. Check both directions.
    refuse(db,"SELECT 1 FROM pull_resolutions r JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=r.account_id AND d.command_id=r.command_id AND d.purpose='confirmed' LEFT JOIN command_evidence e ON e.account_id=d.account_id AND e.command_id=d.command_id AND e.ordinal=d.evidence_ordinal AND e.kind='github.pull_created' AND e.version=1 WHERE c.state<>'confirmed' OR d.command_id IS NULL OR e.command_id IS NULL LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM commands c LEFT JOIN pull_resolutions r USING(account_id,command_id) WHERE c.operation_kind='github.create_pull_request' AND c.payload_version=1 AND c.state='confirmed' AND r.command_id IS NULL LIMIT 1").await?;
    refuse(db,"SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN pull_resolutions r USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=e.account_id AND d.command_id=e.command_id AND d.evidence_ordinal=e.ordinal AND d.purpose='confirmed' WHERE e.kind='github.pull_created' AND e.version=1 AND (c.operation_kind<>'github.create_pull_request' OR c.payload_version<>1 OR c.state<>'confirmed' OR r.command_id IS NULL OR d.command_id IS NULL) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.submission_hash,c.payload_bytes,e.payload,x.execution_base,r.draft_id,r.entity_id,r.provider_id,r.number,r.url,r.observed_source_oid,r.observed_base_oid FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN pull_resolutions r USING(account_id,command_id) LEFT JOIN delivery_attempt_context x ON x.account_id=e.account_id AND x.command_id=e.command_id AND x.attempt_number=e.attempt_number WHERE e.kind='github.pull_created' AND e.version=1").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let payload = n::decode_parts(
                &column::<Vec<u8>>(&row, "payload_bytes")?,
                &column::<String>(&row, "account_id")?,
                &column::<String>(&row, "command_id")?,
                &column::<String>(&row, "target_id")?,
                &column::<i64>(&row, "authorization_epoch")?.to_string(),
            )
            .map_err(|_| invalid_backup())?;
            let evidence: n::ReceiptEvidence = n::decode_json(&column::<Vec<u8>>(&row, "payload")?)
                .map_err(|_| invalid_backup())?;
            let hash: Vec<u8> = column(&row, "submission_hash")?;
            let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
            if !n::receipt_matches(&evidence, &payload)
                || n::encode(&evidence.preparation).map_err(|_| invalid_backup())?
                    != column::<Vec<u8>>(&row, "execution_base")?
                || evidence.preparation.command_hash != hash
                || column::<String>(&row, "draft_id")? != payload.request.context.key.draft_id
                || column::<String>(&row, "entity_id")? != evidence.receipt.item.id
                || column::<String>(&row, "provider_id")? != evidence.receipt.item.provider_id
                || Some(column::<String>(&row, "number")?) != evidence.receipt.item.number
                || Some(column::<String>(&row, "url")?) != evidence.receipt.item.web_url
                || Some(column::<String>(&row, "observed_source_oid")?).as_deref()
                    != evidence
                        .receipt
                        .metadata
                        .values
                        .head
                        .as_ref()
                        .map(|b| b.oid.as_str())
                || Some(column::<String>(&row, "observed_base_oid")?).as_deref()
                    != evidence
                        .receipt
                        .metadata
                        .values
                        .base
                        .as_ref()
                        .map(|b| b.oid.as_str())
            {
                return Err(invalid_backup());
            }
        }
    }
    refuse(db,"SELECT 1 FROM pull_creation_visibility v LEFT JOIN pull_resolutions r ON r.account_id=v.account_id AND r.command_id=v.command_id JOIN commands c ON c.account_id=v.account_id AND c.command_id=v.command_id JOIN items i ON i.account_id=v.account_id AND i.id=v.entity_id WHERE r.entity_id IS NULL OR r.entity_id<>v.entity_id OR c.state<>'confirmed' OR CAST(c.authorization_epoch AS TEXT)<>v.authorization_epoch OR i.kind<>'pull_request' OR json_extract(i.json,'$.provider_id') IS NOT r.provider_id OR i.repository_id IS NOT c.repository_id LIMIT 1").await?;
    Ok(())
}
