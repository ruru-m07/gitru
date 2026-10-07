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
