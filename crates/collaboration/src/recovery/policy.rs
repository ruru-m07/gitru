//! Read-only acceptance of authored state. Unknown operation payloads are opaque;
//! their framing, immutable envelope and relational identity are still verified.
use super::*;
use crate::{LocalRemoteEndpoint, ProviderInstance, ProviderKind};

pub(super) async fn generation(db: &mut SqliteConnection, version: i64) -> Result<i64> {
    if version < 15 {
        return Ok(0);
    }
    sqlx::query_scalar("SELECT generation FROM recovery_meta WHERE singleton=1")
        .fetch_one(db)
        .await
        .map_err(|_| invalid_backup())
}

pub(super) async fn verify_authored(db: &mut SqliteConnection, version: i64) -> Result<()> {
    let meta: (i64, i64, i64) = sqlx::query_as(
        "SELECT revision,authorization_view,log_floor FROM runtime_meta WHERE singleton=1",
    )
    .fetch_one(&mut *db)
    .await
    .map_err(|_| invalid_backup())?;
    if meta.0 < 0 || meta.1 < 0 || meta.2 < 0 || meta.2 > meta.0 {
        return Err(invalid_backup());
    }
    if version >= 3 {
        verify_identities(db).await?;
    }
    if version >= 6 {
        verify_links(db).await?;
    }
    if version >= 10 {
        refuse_rows(db, "SELECT 1 FROM cache_pins p JOIN resource_identities i ON i.account_id=p.account_id AND i.instance_id=p.instance_id AND i.entity_id=p.entity_id WHERE p.kind<>i.kind LIMIT 1").await?;
    }
    if version >= 12 {
        let mut rows = sqlx::query(
            "SELECT notification_id,activity_updated_at,snoozed_until FROM local_inbox_state",
        )
        .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            identifier(&column::<String>(&row, "notification_id")?)?;
            timestamp(&column::<String>(&row, "activity_updated_at")?)?;
            if let Some(value) = column::<Option<String>>(&row, "snoozed_until")? {
                timestamp(&value)?;
            }
        }
    }
    if version >= 13 {
        verify_commands(db).await?;
    }
    if version >= 15 {
        let current = generation(db, version).await?;
        if current < 0 {
            return Err(invalid_backup());
        }
        refuse_rows(db, "SELECT 1 FROM command_recovery_quarantine WHERE recovery_generation>(SELECT generation FROM recovery_meta WHERE singleton=1) LIMIT 1").await?;
    }
    if version >= 16 {
        verify_delivery(db).await?;
    }
    if version >= 17 {
        let mut rows = sqlx::query("SELECT e.version,e.patch_json,c.target_kind FROM command_effects e JOIN commands c USING(account_id,command_id)").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let json: String = column(&row, "patch_json")?;
            if column::<i64>(&row, "version")? != crate::effective::EFFECT_VERSION
                || json.len() > 131072
            {
                return Err(invalid_backup());
            }
            let kind = match column::<String>(&row, "target_kind")?.as_str() {
                "pull_request" => crate::RemoteItemKind::PullRequest,
                "issue" => crate::RemoteItemKind::Issue,
                "notification" => crate::RemoteItemKind::Notification,
                _ => return Err(invalid_backup()),
            };
            let patch: crate::effective::ItemIntentPatch =
                serde_json::from_str(&json).map_err(|_| invalid_backup())?;
            patch.validate(&kind).map_err(|_| invalid_backup())?;
        }
    }
    if version >= 19 {
        verify_recovery_actions(db).await?;
    }
    if version >= 22 {
        verify_issue_creation(db).await?;
    }
    if version >= 20 {
        verify_comments(db).await?;
    }
    if version >= 23 {
        super::pull_creation::verify(db).await?;
    }
    Ok(())
}

async fn refuse_rows(db: &mut SqliteConnection, sql: &'static str) -> Result<()> {
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
fn identifier(value: &str) -> Result<()> {
    validate_identifier(value).map_err(|_| invalid_backup())
}
fn endpoint_path(value: &str) -> Result<()> {
    // Native remote paths/prefixes permit 2048 bytes, unlike local identity keys.
    // Fresh registration/remote proofs still decide whether a link is usable.
    if !plain(value, 2048)
        || value.contains(['%', '\\', '?', '#'])
        || !value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.starts_with('~'))
    {
        return Err(invalid_backup());
    }
    Ok(())
}
fn plain(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn host(value: &str) -> Result<()> {
    if !plain(value, 255)
        || value.contains(['@', '/', '?', '#', '%', '\\'])
        || !url::Url::parse(&format!("https://{value}/"))
            .is_ok_and(|url| url.host_str() == Some(value) && url.port().is_none())
    {
        return Err(invalid_backup());
    }
    Ok(())
}
fn timestamp(value: &str) -> Result<()> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| invalid_backup())
}
fn kind(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_lowercase()
        || !value.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
    {
        return Err(invalid_backup());
    }
    Ok(())
}
fn uuid(value: &str) -> Result<()> {
    if Uuid::parse_str(value)
        .map_err(|_| invalid_backup())?
        .hyphenated()
        .to_string()
        != value
    {
        return Err(invalid_backup());
    }
    Ok(())
}

async fn verify_identities(db: &mut SqliteConnection) -> Result<()> {
    {
        let mut rows =
            sqlx::query("SELECT id,provider,base_url FROM provider_instances").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let provider: ProviderKind =
                serde_json::from_value(serde_json::Value::String(column(&row, "provider")?))
                    .map_err(|_| invalid_backup())?;
            let base: String = column(&row, "base_url")?;
            let instance = ProviderInstance::new(provider, &base).map_err(|_| invalid_backup())?;
            if instance.id != column::<String>(&row, "id")? || instance.base_url != base {
                return Err(invalid_backup());
            }
        }
    }
    {
        let mut rows = sqlx::query("SELECT a.json,b.instance_id FROM accounts a LEFT JOIN account_instances b ON b.account_id=a.id").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let account: RemoteAccount = serde_json::from_str(&column::<String>(&row, "json")?)
                .map_err(|_| invalid_backup())?;
            identifier(&account.id)?;
            identifier(&account.actor_id)?;
            if ProviderInstance::for_account(&account)
                .map_err(|_| invalid_backup())?
                .id
                != column::<String>(&row, "instance_id")?
            {
                return Err(invalid_backup());
            }
        }
    }
    {
        let mut rows = sqlx::query(
            "SELECT entity_id,provider_id,repository_provider_id FROM resource_identities",
        )
        .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            identifier(&column::<String>(&row, "entity_id")?)?;
            identifier(&column::<String>(&row, "provider_id")?)?;
            let repository: String = column(&row, "repository_provider_id")?;
            if !repository.is_empty() {
                identifier(&repository)?;
            }
        }
    }
    refuse_rows(db,"SELECT 1 FROM resource_aliases a JOIN resource_identities i ON i.account_id=a.account_id AND i.instance_id=a.instance_id AND i.entity_id=a.entity_id WHERE a.kind<>i.kind LIMIT 1").await
}

async fn verify_links(db: &mut SqliteConnection) -> Result<()> {
    {
        let mut rows = sqlx::query("SELECT * FROM local_repository_links").fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            for column_name in [
                "id",
                "local_repository_id",
                "account_id",
                "instance_id",
                "actor_id",
                "repository_id",
                "repository_provider_id",
                "registration_proof",
                "remote_digest",
            ] {
                identifier(&column::<String>(&row, column_name)?)?;
            }
            let encoded: String = column(&row, "endpoint_json")?;
            if encoded.len() > 16_384 {
                return Err(invalid_backup());
            }
            let endpoint: LocalRemoteEndpoint =
                serde_json::from_str(&encoded).map_err(|_| invalid_backup())?;
            if !plain(&endpoint.remote_name, 255)
                || endpoint.ordinal >= 32
                || !plain(&column::<String>(&row, "registration_proof")?, 256)
                || !plain(&column::<String>(&row, "remote_digest")?, 256)
            {
                return Err(invalid_backup());
            }
            host(&endpoint.host)?;
            endpoint_path(&endpoint.path)?;
            if endpoint.port == 0 {
                return Err(invalid_backup());
            }
        }
    }
    refuse_rows(db,"SELECT 1 FROM local_repository_links l JOIN accounts a ON a.id=l.account_id WHERE l.actor_id<>a.actor_id LIMIT 1").await?;
    let mut rows =
        sqlx::query("SELECT id,host,path_prefix FROM local_transport_bindings").fetch(&mut *db);
    while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
        identifier(&column::<String>(&row, "id")?)?;
        host(&column::<String>(&row, "host")?)?;
        let prefix: String = column(&row, "path_prefix")?;
        if !prefix.is_empty() {
            endpoint_path(&prefix)?;
        }
    }
    Ok(())
}

/// The frozen v1 framing is tag:u16be, length:u32be, bytes. This reads without
/// reserializing any payload or interpreting unsupported operation versions.
fn fields(bytes: &[u8], max_fields: usize) -> Result<Vec<&[u8]>> {
    if bytes.len() > 262_144 {
        return Err(invalid_backup());
    }
    let mut remaining = bytes;
    let mut values = Vec::new();
    let mut last = 0u16;
    while !remaining.is_empty() {
        if remaining.len() < 6 || values.len() >= max_fields {
            return Err(invalid_backup());
        }
        let tag = u16::from_be_bytes([remaining[0], remaining[1]]);
        let length =
            u32::from_be_bytes(remaining[2..6].try_into().map_err(|_| invalid_backup())?) as usize;
        if tag <= last || length > remaining.len() - 6 {
            return Err(invalid_backup());
        }
        last = tag;
        values.push(&remaining[6..6 + length]);
        remaining = &remaining[6 + length..];
    }
    Ok(values)
}
fn exact_fields(bytes: &[u8], count: usize) -> Result<Vec<&[u8]>> {
    let values = fields(bytes, count)?;
    if values.len() != count {
        return Err(invalid_backup());
    }
    // Outer envelope/target/guard tags and sequence ordinals are contiguous.
    let mut offset = 0;
    for (i, value) in values.iter().enumerate() {
        if u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) != i as u16 + 1 {
            return Err(invalid_backup());
        }
        offset += 6 + value.len();
    }
    Ok(values)
}
fn text(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|_| invalid_backup())
}
fn u32_value(bytes: &[u8]) -> Result<u32> {
    Ok(u32::from_be_bytes(
        bytes.try_into().map_err(|_| invalid_backup())?,
    ))
}

async fn verify_commands(db: &mut SqliteConnection) -> Result<()> {
    // Keyset one bounded envelope at a time; no payload-wide aggregation.
    let mut cursor: Option<(String, String)> = None;
    loop {
        let row=sqlx::query("SELECT * FROM commands WHERE (? IS NULL OR (account_id,command_id)>(?,?)) ORDER BY account_id,command_id LIMIT 1")
            .bind(cursor.as_ref().map(|c|&c.0)).bind(cursor.as_ref().map(|c|&c.0)).bind(cursor.as_ref().map(|c|&c.1))
            .fetch_optional(&mut *db).await.map_err(|_| invalid_backup())?;
        let Some(row) = row else {
            break;
        };
        let account: String = column(&row, "account_id")?;
        let command: String = column(&row, "command_id")?;
        identifier(&account)?;
        uuid(&command)?;
        timestamp(&column::<String>(&row, "admitted_at")?)?;
        let envelope: Vec<u8> = column(&row, "canonical_envelope")?;
        let f = exact_fields(&envelope, 9)?;
        if u32_value(f[0])? != 1
            || column::<i64>(&row, "envelope_version")? != 1
            || text(f[1])? != account
            || f[2] != column::<i64>(&row, "authorization_epoch")?.to_be_bytes()
            || text(f[3])? != column::<String>(&row, "operation_kind")?
            || i64::from(u32_value(f[4])?) != column::<i64>(&row, "payload_version")?
            || f[6] != column::<Vec<u8>>(&row, "payload_bytes")?
            || f[7] != column::<Vec<u8>>(&row, "guard_bytes")?
        {
            return Err(invalid_backup());
        }
        kind(text(f[3])?)?;
        fields(f[6], 43_690)?;
        let target = exact_fields(f[5], 3)?;
        let target_kind = match target[0] {
            [1] => "repository",
            [2] => "pull_request",
            [3] => "issue",
            [4] => "notification",
            _ => return Err(invalid_backup()),
        };
        if target_kind != column::<String>(&row, "target_kind")?
            || text(target[1])? != column::<String>(&row, "target_id")?
        {
            return Err(invalid_backup());
        }
        identifier(text(target[1])?)?;
        let repository = match target[2].split_first() {
            Some((0, [])) => None,
            Some((1, rest)) => {
                let id = text(rest)?;
                identifier(id)?;
                Some(id)
            }
            _ => return Err(invalid_backup()),
        };
        if repository != column::<Option<String>>(&row, "repository_id")?.as_deref() {
            return Err(invalid_backup());
        }
        let guards = fields(f[7], 32)?;
        for guard in exact_fields(f[7], guards.len())? {
            let guard = exact_fields(guard, 3)?;
            kind(text(guard[0])?)?;
            if u32_value(guard[1])? == 0 {
                return Err(invalid_backup());
            }
            fields(guard[2], 43_690)?;
        }
        let dependencies = fields(f[8], 64)?;
        let dependencies = exact_fields(f[8], dependencies.len())?;
        let rows=sqlx::query("SELECT d.ordinal,d.predecessor_id,c.enqueue_order FROM command_dependencies d JOIN commands c ON c.account_id=d.account_id AND c.command_id=d.predecessor_id WHERE d.account_id=? AND d.command_id=? ORDER BY d.ordinal LIMIT 65")
            .bind(&account).bind(&command).fetch_all(&mut *db).await.map_err(|_| invalid_backup())?;
        if rows.len() != dependencies.len() {
            return Err(invalid_backup());
        }
        for (ordinal, (predecessor, row)) in dependencies.iter().zip(rows.iter()).enumerate() {
            let predecessor = text(predecessor)?;
            uuid(predecessor)?;
            if predecessor == command
                || column::<i64>(row, "ordinal")? != ordinal as i64
                || predecessor != column::<String>(row, "predecessor_id")?
            {
                return Err(invalid_backup());
            }
        }
        // Hash corroborates exact decomposed bytes; it is never the sole check.
        let domain = b"gitru.collaboration.command-submission.sha256.v1";
        let mut hash = Sha256::new();
        hash.update((domain.len() as u32).to_be_bytes());
        hash.update(domain);
        hash.update((envelope.len() as u64).to_be_bytes());
        hash.update(&envelope);
        if hash.finalize().as_slice() != column::<Vec<u8>>(&row, "submission_hash")? {
            return Err(invalid_backup());
        }
        cursor = Some((account, command));
    }
    refuse_rows(db,"SELECT 1 FROM command_dependencies d JOIN commands c ON c.account_id=d.account_id AND c.command_id=d.command_id JOIN commands p ON p.account_id=d.account_id AND p.command_id=d.predecessor_id WHERE p.enqueue_order>=c.enqueue_order OR d.required<>(c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM command_target_protections p WHERE p.required<>(EXISTS(SELECT 1 FROM commands c WHERE c.account_id=p.account_id AND c.command_id=p.command_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) OR EXISTS(SELECT 1 FROM command_dependencies d WHERE d.account_id=p.account_id AND d.predecessor_id=p.command_id AND d.required=1)) LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM command_target_protections GROUP BY account_id,command_id HAVING count(*)>128 LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM delivery_attempts GROUP BY account_id,command_id HAVING count(*)<>max(attempt_number) LIMIT 1").await?;
    for sql in [
        "SELECT started_at AS value FROM delivery_attempts UNION ALL SELECT completed_at FROM delivery_attempts WHERE completed_at IS NOT NULL",
        "SELECT recorded_at AS value FROM command_evidence",
    ] {
        let mut rows = sqlx::query(sql).fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            timestamp(&column::<String>(&row, "value")?)?;
        }
    }
    Ok(())
}

async fn verify_delivery(db: &mut SqliteConnection) -> Result<()> {
    // Every admitted command has exactly one bounded scheduling record. Older
    // attempt history may legitimately predate execution-context storage.
    refuse_rows(db, "SELECT 1 FROM commands c LEFT JOIN command_delivery d USING(account_id,command_id) WHERE d.command_id IS NULL LIMIT 1").await?;
    refuse_rows(db, "SELECT 1 FROM delivery_attempt_context c JOIN account_instances a ON a.account_id=c.account_id WHERE c.instance_id<>a.instance_id LIMIT 1").await?;
    refuse_rows(db, "SELECT 1 FROM delivery_resolutions r JOIN command_delivery d USING(account_id,command_id) WHERE r.delivery_generation>d.generation LIMIT 1").await?;
    let mut rows =
        sqlx::query("SELECT next_action_at FROM command_delivery WHERE next_action_at IS NOT NULL")
            .fetch(&mut *db);
    while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
        timestamp(&column::<String>(&row, "next_action_at")?)?;
    }
    // Execution bases and resolution evidence remain opaque for unsupported
    // codecs. SQL CHECK/FK constraints enforce bounded framing and provenance;
    // restore retains them byte-for-byte and only resets scheduling authority.
    Ok(())
}

async fn verify_recovery_actions(db: &mut SqliteConnection) -> Result<()> {
    use crate::command_recovery::*;
    // Recompute immutable edge affinity independently of creation-time triggers.
    // Current accounts can be deauthorized and replacements can themselves have
    // finished or been superseded since this historical receipt was authored.
    refuse_rows(db, "SELECT 1 FROM command_supersessions s JOIN commands o ON o.account_id=s.account_id AND o.command_id=s.original_id JOIN commands n ON n.account_id=s.account_id AND n.command_id=s.replacement_id LEFT JOIN command_supersessions p ON p.account_id=s.account_id AND p.replacement_id=s.original_id JOIN command_recovery_actions a ON a.account_id=s.account_id AND a.action_id=s.action_id WHERE o.state<>'superseded' OR o.authorization_epoch<>n.authorization_epoch OR o.target_kind<>n.target_kind OR o.target_id<>n.target_id OR o.repository_id IS NOT n.repository_id OR o.operation_kind<>n.operation_kind OR o.payload_version<>n.payload_version OR o.enqueue_order>=n.enqueue_order OR s.execution_order<>coalesce(p.execution_order,o.enqueue_order) OR s.depth<>coalesce(p.depth,0)+1 OR a.command_id<>s.original_id OR json_extract(a.request_json,'$.new_command_id') IS NOT s.replacement_id OR json_extract(a.receipt_json,'$.replacement_id') IS NOT s.replacement_id OR EXISTS(SELECT 1 FROM command_dependencies d JOIN commands p ON p.account_id=d.account_id AND p.command_id=d.predecessor_id LEFT JOIN command_supersessions ps ON ps.account_id=p.account_id AND ps.replacement_id=p.command_id WHERE d.account_id=s.account_id AND d.command_id=s.replacement_id AND coalesce(ps.execution_order,p.enqueue_order)>=s.execution_order) LIMIT 1").await?;
    refuse_rows(db, "SELECT 1 FROM command_recovery_actions GROUP BY account_id,command_id HAVING min(ordinal)<>0 OR max(ordinal)+1<>count(*) OR sum(octet_length(request_json))>1048576 LIMIT 1").await?;
    refuse_rows(db, "SELECT 1 FROM command_recovery_actions a WHERE json_extract(a.receipt_json,'$.replacement_id') IS NOT NULL AND NOT EXISTS(SELECT 1 FROM command_supersessions s WHERE s.account_id=a.account_id AND s.action_id=a.action_id) LIMIT 1").await?;
    refuse_rows(db, "SELECT 1 FROM command_user_controls u WHERE u.paused IS NOT (SELECT json_extract(a.receipt_json,'$.paused') FROM command_recovery_actions a WHERE a.account_id=u.account_id AND a.command_id=u.command_id ORDER BY ordinal DESC LIMIT 1) LIMIT 1").await?;
    let mut rows = sqlx::query("SELECT account_id,command_id,action_id,request_json,receipt_json FROM command_recovery_actions").fetch(&mut *db);
    while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
        let account: String = column(&row, "account_id")?;
        let command: String = column(&row, "command_id")?;
        let action: String = column(&row, "action_id")?;
        let request: String = column(&row, "request_json")?;
        let receipt_json: String = column(&row, "receipt_json")?;
        let receipt: CommandRecoveryReceipt =
            serde_json::from_str(&receipt_json).map_err(|_| invalid_backup())?;
        if serde_json::to_string(&receipt).map_err(|_| invalid_backup())? != receipt_json
            || crate::delivery::DeliveryState::parse(&receipt.state).is_err()
            || receipt.account_id != account
            || receipt.command_id != command
            || receipt.action_id != action
            || receipt.revision.parse::<u64>().ok().is_none_or(|n| n == 0)
        {
            return Err(invalid_backup());
        }
        let context = if let Some(replacement) = &receipt.replacement_id {
            let value: CommandRecoveryReplaceRequest =
                serde_json::from_str(&request).map_err(|_| invalid_backup())?;
            if serde_json::to_string(&value).map_err(|_| invalid_backup())? != request
                || value.action_id != action
                || value.new_command_id != *replacement
                || receipt.state != "superseded"
                || receipt.paused
                || value.fields.len() > 5
                || value.fields.iter().enumerate().any(|(i, f)| {
                    value.fields[..i].iter().any(|other| other.field == f.field)
                        || f.field == CommandReviewField::Head
                        || f.choice != CommandResolutionChoice::Edited && f.value.is_some()
                        || f.value
                            .as_ref()
                            .is_some_and(|v| v.len() > 65536 || v.contains('\0'))
                })
            {
                return Err(invalid_backup());
            }
            Uuid::parse_str(replacement).map_err(|_| invalid_backup())?;
            value.context
        } else {
            let value: CommandRecoveryActionRequest =
                serde_json::from_str(&request).map_err(|_| invalid_backup())?;
            if serde_json::to_string(&value).map_err(|_| invalid_backup())? != request
                || value.action_id != action
                || match value.action {
                    CommandRecoveryAction::Cancel => {
                        receipt.state != "cancelled"
                            || receipt.paused
                            || receipt.remote_may_have_happened
                    }
                    CommandRecoveryAction::Pause => {
                        !receipt.paused || !receipt.remote_may_have_happened
                    }
                    CommandRecoveryAction::Resume => receipt.paused,
                }
            {
                return Err(invalid_backup());
            }
            value.context
        };
        if context.account_id != account
            || context.command_id != command
            || context
                .expected_epoch
                .parse::<u64>()
                .ok()
                .is_none_or(|n| n == 0)
            || context
                .expected_generation
                .parse::<u64>()
                .ok()
                .is_none_or(|n| n == 0)
            || context
                .authorization_view
                .parse::<u64>()
                .ok()
                .is_none_or(|n| n == 0)
            || context.review_token.len() != 64
            || !context
                .review_token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid_backup());
        }
        if Uuid::parse_str(&action)
            .map_err(|_| invalid_backup())?
            .hyphenated()
            .to_string()
            != action
        {
            return Err(invalid_backup());
        }
    }
    Ok(())
}

async fn verify_comments(db: &mut SqliteConnection) -> Result<()> {
    use crate::comment_send::native as n;
    {
        let mut rows =
            sqlx::query("SELECT account_id,subject_id,body,generation FROM comment_drafts")
                .fetch(&mut *db);
        while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
            identifier(&column::<String>(&row, "account_id")?)?;
            identifier(&column::<String>(&row, "subject_id")?)?;
            n::validate_body(&column::<String>(&row, "body")?).map_err(|_| invalid_backup())?;
            if column::<i64>(&row, "generation")? <= 0 {
                return Err(invalid_backup());
            }
        }
    }
    refuse_rows(db,"SELECT 1 FROM comment_submissions s JOIN commands c USING(account_id,command_id) JOIN comment_drafts d ON d.account_id=s.account_id AND d.subject_id=s.subject_id WHERE c.operation_kind<>'github.create_comment' OR c.payload_version<>1 OR c.target_id<>s.subject_id OR c.target_kind NOT IN ('issue','pull_request') OR d.generation<s.draft_generation LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM commands c LEFT JOIN comment_submissions s USING(account_id,command_id) WHERE c.operation_kind='github.create_comment' AND c.payload_version=1 AND s.command_id IS NULL LIMIT 1").await?;
    let mut rows=sqlx::query("SELECT s.account_id,s.subject_id,s.command_id,s.draft_generation,s.body_hash,c.authorization_epoch,c.payload_bytes,d.body,d.generation FROM comment_submissions s JOIN commands c USING(account_id,command_id) JOIN comment_drafts d ON d.account_id=s.account_id AND d.subject_id=s.subject_id").fetch(&mut *db);
    while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
        let p = n::decode_parts(
            &column::<Vec<u8>>(&row, "payload_bytes")?,
            &column::<String>(&row, "account_id")?,
            &column::<String>(&row, "command_id")?,
            &column::<String>(&row, "subject_id")?,
            &column::<i64>(&row, "authorization_epoch")?.to_string(),
        )
        .map_err(|_| invalid_backup())?;
        if p.request.draft_generation != column::<i64>(&row, "draft_generation")?.to_string()
            || n::body_hash(&p.body) != column::<Vec<u8>>(&row, "body_hash")?
            || column::<i64>(&row, "generation")? == column::<i64>(&row, "draft_generation")?
                && p.body != column::<String>(&row, "body")?
        {
            return Err(invalid_backup());
        }
    }
    drop(rows);
    let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.target_kind,c.repository_id,c.submission_hash,c.authorization_epoch,c.payload_bytes,e.payload,a.json AS account_json FROM command_evidence e JOIN commands c USING(account_id,command_id) JOIN accounts a ON a.id=c.account_id WHERE e.kind='github.comment_created' AND e.version=1").fetch(&mut *db);
    while let Some(row) = rows.try_next().await.map_err(|_| invalid_backup())? {
        let payload = n::decode_parts(
            &column::<Vec<u8>>(&row, "payload_bytes")?,
            &column::<String>(&row, "account_id")?,
            &column::<String>(&row, "command_id")?,
            &column::<String>(&row, "target_id")?,
            &column::<i64>(&row, "authorization_epoch")?.to_string(),
        )
        .map_err(|_| invalid_backup())?;
        let receipt: n::ReceiptEvidence =
            n::decode_json(&column::<Vec<u8>>(&row, "payload")?).map_err(|_| invalid_backup())?;
        let account: RemoteAccount = serde_json::from_str(&column::<String>(&row, "account_json")?)
            .map_err(|_| invalid_backup())?;
        let hash: Vec<u8> = column(&row, "submission_hash")?;
        let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
        let target_kind: String = column(&row, "target_kind")?;
        let frame = &receipt.preparation.frame;
        if !n::receipt_matches(&receipt, &payload)
            || receipt.preparation.command_hash != hash
            || receipt.preparation.actor != account.actor_id
            || Some(frame.repository.id.as_str())
                != column::<Option<String>>(&row, "repository_id")?.as_deref()
            || !matches!(
                (target_kind.as_str(), &frame.subject.kind),
                ("pull_request", crate::RemoteItemKind::PullRequest)
                    | ("issue", crate::RemoteItemKind::Issue)
            )
        {
            return Err(invalid_backup());
        }
    }

    Ok(())
}

async fn verify_issue_creation(db: &mut SqliteConnection) -> Result<()> {
    use crate::issue_creation::native as n;
    {
        let mut rows = sqlx::query(
            "SELECT account_id,draft_id,repository_id,title,body,generation FROM issue_drafts",
        )
        .fetch(&mut *db);
        while let Some(r) = rows.try_next().await.map_err(|_| invalid_backup())? {
            identifier(&column::<String>(&r, "account_id")?)?;
            identifier(&column::<String>(&r, "repository_id")?)?;
            n::validate_uuid(&column::<String>(&r, "draft_id")?).map_err(|_| invalid_backup())?;
            n::validate_title(&column::<String>(&r, "title")?, false)
                .map_err(|_| invalid_backup())?;
            n::validate_body(&column::<String>(&r, "body")?).map_err(|_| invalid_backup())?;
            if column::<i64>(&r, "generation")? <= 0 {
                return Err(invalid_backup());
            }
        }
    }
    refuse_rows(db,"SELECT 1 FROM issue_submissions s JOIN commands c USING(account_id,command_id) JOIN issue_drafts d USING(account_id,draft_id) WHERE c.operation_kind<>'github.create_issue' OR c.payload_version<>1 OR c.target_kind<>'repository' OR c.target_id<>d.repository_id OR c.repository_id<>d.repository_id OR d.generation<s.draft_generation LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM commands c LEFT JOIN issue_submissions s USING(account_id,command_id) WHERE c.operation_kind='github.create_issue' AND c.payload_version=1 AND s.command_id IS NULL LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT s.account_id,s.draft_id,s.command_id,s.draft_generation,s.content_hash,c.target_id,c.authorization_epoch,c.payload_bytes,d.title,d.body,d.generation FROM issue_submissions s JOIN commands c USING(account_id,command_id) JOIN issue_drafts d USING(account_id,draft_id)").fetch(&mut *db);
        while let Some(r) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let p = n::decode_parts(
                &column::<Vec<u8>>(&r, "payload_bytes")?,
                &column::<String>(&r, "account_id")?,
                &column::<String>(&r, "command_id")?,
                &column::<String>(&r, "target_id")?,
                &column::<i64>(&r, "authorization_epoch")?.to_string(),
            )
            .map_err(|_| invalid_backup())?;
            if p.request.draft_id != column::<String>(&r, "draft_id")?
                || p.request.draft_generation != column::<i64>(&r, "draft_generation")?.to_string()
                || n::content_hash(&p.title, &p.body) != column::<Vec<u8>>(&r, "content_hash")?
                || column::<i64>(&r, "generation")? == column::<i64>(&r, "draft_generation")?
                    && (p.title != column::<String>(&r, "title")?
                        || p.body != column::<String>(&r, "body")?)
            {
                return Err(invalid_backup());
            }
        }
    }
    refuse_rows(db,"SELECT 1 FROM issue_resolutions r JOIN commands c USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=r.account_id AND d.command_id=r.command_id AND d.purpose='confirmed' LEFT JOIN command_evidence e ON e.account_id=d.account_id AND e.command_id=d.command_id AND e.ordinal=d.evidence_ordinal AND e.kind='github.issue_created' AND e.version=1 WHERE c.state<>'confirmed' OR d.command_id IS NULL OR e.command_id IS NULL LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM commands c LEFT JOIN issue_resolutions r USING(account_id,command_id) WHERE c.operation_kind='github.create_issue' AND c.payload_version=1 AND c.state='confirmed' AND r.command_id IS NULL LIMIT 1").await?;
    refuse_rows(db,"SELECT 1 FROM command_evidence e JOIN commands c USING(account_id,command_id) LEFT JOIN issue_resolutions r USING(account_id,command_id) LEFT JOIN delivery_resolutions d ON d.account_id=e.account_id AND d.command_id=e.command_id AND d.evidence_ordinal=e.ordinal AND d.purpose='confirmed' WHERE e.kind='github.issue_created' AND e.version=1 AND (c.operation_kind<>'github.create_issue' OR c.payload_version<>1 OR c.state<>'confirmed' OR r.command_id IS NULL OR d.command_id IS NULL) LIMIT 1").await?;
    {
        let mut rows=sqlx::query("SELECT c.account_id,c.command_id,c.target_id,c.authorization_epoch,c.submission_hash,c.payload_bytes,e.payload,a.json AS account_json,r.draft_id,r.entity_id,r.provider_id,r.number,r.url FROM command_evidence e JOIN commands c USING(account_id,command_id) JOIN accounts a ON a.id=c.account_id LEFT JOIN issue_resolutions r USING(account_id,command_id) WHERE e.kind='github.issue_created' AND e.version=1").fetch(&mut *db);
        while let Some(r) = rows.try_next().await.map_err(|_| invalid_backup())? {
            let p = n::decode_parts(
                &column::<Vec<u8>>(&r, "payload_bytes")?,
                &column::<String>(&r, "account_id")?,
                &column::<String>(&r, "command_id")?,
                &column::<String>(&r, "target_id")?,
                &column::<i64>(&r, "authorization_epoch")?.to_string(),
            )
            .map_err(|_| invalid_backup())?;
            let e: n::ReceiptEvidence =
                n::decode_json(&column::<Vec<u8>>(&r, "payload")?).map_err(|_| invalid_backup())?;
            let a: RemoteAccount = serde_json::from_str(&column::<String>(&r, "account_json")?)
                .map_err(|_| invalid_backup())?;
            let hash: Vec<u8> = column(&r, "submission_hash")?;
            let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
            if !n::receipt_matches(&e, &p)
                || e.preparation.actor != a.actor_id
                || e.preparation.command_hash != hash
            {
                return Err(invalid_backup());
            }
            if column::<String>(&r, "draft_id")? != p.request.draft_id
                || column::<String>(&r, "entity_id")? != e.receipt.item.id
                || column::<String>(&r, "provider_id")? != e.receipt.item.provider_id
                || Some(column::<String>(&r, "number")?) != e.receipt.item.number
                || Some(column::<String>(&r, "url")?) != e.receipt.item.web_url
            {
                return Err(invalid_backup());
            }
        }
    }
    refuse_rows(db,"SELECT 1 FROM issue_creation_visibility v LEFT JOIN issue_resolutions r ON r.account_id=v.account_id AND r.command_id=v.command_id JOIN commands c ON c.account_id=v.account_id AND c.command_id=v.command_id JOIN items i ON i.account_id=v.account_id AND i.id=v.entity_id WHERE r.entity_id IS NULL OR r.entity_id<>v.entity_id OR c.state<>'confirmed' OR CAST(c.authorization_epoch AS TEXT)<>v.authorization_epoch OR i.kind<>'issue' LIMIT 1").await?;
    Ok(())
}
