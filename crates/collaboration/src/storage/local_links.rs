//! One SQLite snapshot resolves remote observations and authored link intent.
use super::*;
use crate::local_links::*;

const MAX_LINKS: usize = 128;
const MAX_ENDPOINTS: usize = 256;

impl Store {
    pub async fn local_link_snapshot(&self, query: LocalLinkQuery) -> Result<LocalLinkSnapshot> {
        validate_query(&query)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let snapshot = snapshot_in(&mut tx, &query).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(snapshot)
    }

    pub async fn confirm_local_link(
        &self,
        request: ConfirmLocalLink,
    ) -> Result<LocalLinkWriteReceipt> {
        self.confirm_local_link_checked(request, || Ok(())).await
    }

    /// Native caller lifetime validation runs synchronously under the writer
    /// lock and before commit. It must never await or dispatch to the UI thread.
    pub async fn confirm_local_link_checked<F>(
        &self,
        request: ConfirmLocalLink,
        validate_owner: F,
    ) -> Result<LocalLinkWriteReceipt>
    where
        F: Fn() -> Result<()> + Send,
    {
        validate_query(&request.query)?;
        if request.query.registration_proof.is_none() {
            return Err(stale_link());
        }
        let mut writer = self.inner.writer.lock().await;
        validate_owner()?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let snapshot = snapshot_in(&mut tx, &request.query).await?;
        if snapshot.authorization_view != request.expected_authorization_view
            || snapshot.bindings_generation != request.expected_bindings_generation
        {
            return Err(stale_link());
        }
        let candidate = snapshot
            .resolutions
            .iter()
            .flat_map(|r| &r.candidates)
            .find(|c| c.id == request.candidate_id)
            .ok_or_else(stale_link)?;
        let (id, generation, previous_account) = if let Some(version) = &request.replace {
            let previous = snapshot
                .links
                .iter()
                .find(|l| l.id == version.id && l.generation == version.generation)
                .ok_or_else(stale_link)?;
            (
                previous.id.clone(),
                positive_revision(&previous.generation)?
                    .checked_add(1)
                    .ok_or_else(CollaborationError::storage)?,
                Some(previous.account_id.clone()),
            )
        } else {
            if snapshot.links.len() >= MAX_LINKS {
                return Err(CollaborationError::invalid(
                    "Too many local repository links",
                ));
            }
            (Uuid::new_v4().to_string(), 1, None)
        };
        let endpoint = serde_json::to_string(&candidate.endpoint)
            .map_err(|_| CollaborationError::storage())?;
        let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_repository_links WHERE local_repository_id=? AND endpoint_json=? AND account_id=? AND id<>?)")
            .bind(&request.query.local_repository_id).bind(&endpoint).bind(&candidate.account_id).bind(&id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        if duplicate {
            return Err(CollaborationError::invalid(
                "This local repository link already exists",
            ));
        }
        sqlx::query("INSERT INTO local_repository_links(id,local_repository_id,endpoint_json,account_id,instance_id,actor_id,repository_id,repository_provider_id,registration_proof,remote_digest,generation) VALUES(?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET endpoint_json=excluded.endpoint_json,account_id=excluded.account_id,instance_id=excluded.instance_id,actor_id=excluded.actor_id,repository_id=excluded.repository_id,repository_provider_id=excluded.repository_provider_id,registration_proof=excluded.registration_proof,remote_digest=excluded.remote_digest,generation=excluded.generation")
            .bind(&id).bind(&request.query.local_repository_id).bind(endpoint).bind(&candidate.account_id).bind(&candidate.instance_id).bind(&candidate.actor_id).bind(&candidate.repository.id).bind(&candidate.repository.provider_id)
            .bind(request.query.registration_proof.as_deref()).bind(request.query.remote_digest.as_deref()).bind(generation).execute(&mut *tx).await.map_err(storage_error)?;
        if let Some(previous_account) = previous_account.filter(|id| id != &candidate.account_id) {
            let previous = account_in(&mut tx, &previous_account, false).await?;
            record_change(
                &mut tx,
                &previous_account,
                positive_revision(&previous.authorization_epoch)?,
                &format!("local_link:{}", request.query.local_repository_id),
                false,
            )
            .await?;
        }
        let revision = record_change(
            &mut tx,
            &candidate.account_id,
            positive_revision(&candidate.authorization_epoch)?,
            &format!("local_link:{}", request.query.local_repository_id),
            false,
        )
        .await?;
        let link = LocalRepositoryLink {
            id,
            local_repository_id: request.query.local_repository_id,
            endpoint: candidate.endpoint.clone(),
            account_id: candidate.account_id.clone(),
            actor_id: candidate.actor_id.clone(),
            instance_id: candidate.instance_id.clone(),
            repository_provider_id: candidate.repository.provider_id.clone(),
            repository_id: candidate.repository.id.clone(),
            generation: generation.to_string(),
            state: LocalLinkState::Linked,
            repository: Some(candidate.repository.clone()),
        };
        validate_owner()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(LocalLinkWriteReceipt {
            link,
            revision,
            authorization_view: snapshot.authorization_view,
        })
    }

    /// Local authored removal stays available after provider disconnection/denial.
    pub async fn remove_local_link(&self, id: &str, generation: &str) -> Result<String> {
        self.remove_local_link_checked(id, generation, || Ok(()))
            .await
    }
    pub async fn remove_local_link_checked<F>(
        &self,
        id: &str,
        generation: &str,
        validate_owner: F,
    ) -> Result<String>
    where
        F: Fn() -> Result<()> + Send,
    {
        validate_identifier(id)?;
        let generation = positive_revision(generation)?;
        let mut writer = self.inner.writer.lock().await;
        validate_owner()?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        let row = sqlx::query("SELECT account_id,local_repository_id FROM local_repository_links WHERE id=? AND generation=?").bind(id).bind(generation).fetch_optional(&mut *tx).await.map_err(storage_error)?.ok_or_else(stale_link)?;
        let account = account_in(&mut tx, row.get("account_id"), false).await?;
        sqlx::query("DELETE FROM local_repository_links WHERE id=? AND generation=?")
            .bind(id)
            .bind(generation)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        let revision = record_change(
            &mut tx,
            &account.id,
            positive_revision(&account.authorization_epoch)?,
            &format!("local_link:{}", row.get::<String, _>("local_repository_id")),
            false,
        )
        .await?;
        validate_owner()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    pub async fn save_transport_binding(
        &self,
        request: SaveTransportBinding,
    ) -> Result<LocalTransportBinding> {
        self.save_transport_binding_checked(request, || Ok(()))
            .await
    }
    pub async fn save_transport_binding_checked<F>(
        &self,
        request: SaveTransportBinding,
        validate_owner: F,
    ) -> Result<LocalTransportBinding>
    where
        F: Fn() -> Result<()> + Send,
    {
        validate_binding(&request)?;
        let mut writer = self.inner.writer.lock().await;
        validate_owner()?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        if binding_generation(&mut tx).await? != request.expected_bindings_generation {
            return Err(stale_link());
        }
        let instance_row =
            sqlx::query("SELECT provider,base_url FROM provider_instances WHERE id=?")
                .bind(&request.instance_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?
                .ok_or_else(not_found)?;
        let provider: ProviderKind = decode(&format!(
            "\"{}\"",
            instance_row.get::<String, _>("provider")
        ))?;
        let instance = ProviderInstance::new(provider, instance_row.get("base_url"))?;
        if instance.id != request.instance_id {
            return Err(CollaborationError::storage());
        }
        let registered: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM account_instances WHERE instance_id=?)",
        )
        .bind(&instance.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        if !registered {
            return Err(CollaborationError::invalid(
                "Transport bindings require a registered provider account",
            ));
        }
        if public_bindings().iter().any(|b| {
            b.transport == request.transport
                && b.host == request.host
                && b.port == request.port
                && b.path_prefix == request.path_prefix
        }) {
            return Err(CollaborationError::invalid(
                "Built-in transport bindings cannot be redefined",
            ));
        }
        let (id, generation) = if let Some(version) = &request.replace {
            let generation = positive_revision(&version.generation)?;
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM local_transport_bindings WHERE id=? AND generation=?)",
            )
            .bind(&version.id)
            .bind(generation)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
            if !exists {
                return Err(stale_link());
            }
            (
                version.id.clone(),
                generation
                    .checked_add(1)
                    .ok_or_else(CollaborationError::storage)?,
            )
        } else {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_transport_bindings")
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
            if count >= 128 {
                return Err(CollaborationError::invalid("Too many transport bindings"));
            }
            (Uuid::new_v4().to_string(), 1)
        };
        let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_transport_bindings WHERE transport=? AND host=? AND port=? AND path_prefix=? AND id<>?)").bind(tag(&request.transport)?).bind(&request.host).bind(i64::from(request.port)).bind(&request.path_prefix).bind(&id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        if duplicate {
            return Err(CollaborationError::invalid(
                "This transport binding already exists",
            ));
        }
        sqlx::query("INSERT INTO local_transport_bindings(id,instance_id,transport,host,port,path_prefix,layout,generation) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET instance_id=excluded.instance_id,transport=excluded.transport,host=excluded.host,port=excluded.port,path_prefix=excluded.path_prefix,layout=excluded.layout,generation=excluded.generation")
            .bind(&id).bind(&request.instance_id).bind(tag(&request.transport)?).bind(&request.host).bind(i64::from(request.port)).bind(&request.path_prefix).bind(tag(&request.layout)?).bind(generation).execute(&mut *tx).await.map_err(storage_error)?;
        binding_changed(&mut tx).await?;
        validate_owner()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(LocalTransportBinding {
            id,
            instance_id: request.instance_id,
            transport: request.transport,
            host: request.host,
            port: request.port,
            path_prefix: request.path_prefix,
            layout: request.layout,
            generation: generation.to_string(),
        })
    }

    pub async fn remove_transport_binding(
        &self,
        id: &str,
        generation: &str,
        expected_bindings_generation: &str,
    ) -> Result<String> {
        self.remove_transport_binding_checked(id, generation, expected_bindings_generation, || {
            Ok(())
        })
        .await
    }
    pub async fn remove_transport_binding_checked<F>(
        &self,
        id: &str,
        generation: &str,
        expected_bindings_generation: &str,
        validate_owner: F,
    ) -> Result<String>
    where
        F: Fn() -> Result<()> + Send,
    {
        validate_identifier(id)?;
        let generation = positive_revision(generation)?;
        let mut writer = self.inner.writer.lock().await;
        validate_owner()?;
        let mut tx = writer.begin().await.map_err(storage_error)?;
        if binding_generation(&mut tx).await? != expected_bindings_generation {
            return Err(stale_link());
        }
        let result =
            sqlx::query("DELETE FROM local_transport_bindings WHERE id=? AND generation=?")
                .bind(id)
                .bind(generation)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(stale_link());
        }
        binding_changed(&mut tx).await?;
        let (revision, _) = metadata(&mut tx).await?;
        validate_owner()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(revision)
    }

    /// Discover authored clone choices after the current remote read gate. The
    /// caller must inspect each trusted local registration before navigation.
    pub async fn local_links_for_resource(
        &self,
        account_id: &str,
        instance_id: &str,
        repository_id: &str,
        epoch: &str,
    ) -> Result<Vec<LocalRepositoryLink>> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, true).await?;
        if account.authorization_epoch != epoch
            || identities::instance_in(&mut tx, &account).await?.id != instance_id
        {
            return Err(stale_link());
        }
        if !identities::accessible(&mut tx, account_id, repository_id, ResourceKind::Repository)
            .await?
        {
            return Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "Saved repository access is unavailable",
            ));
        }
        let repository = repository_from_row(
            &sqlx::query("SELECT json,selected FROM repositories WHERE account_id=? AND id=?")
                .bind(account_id)
                .bind(repository_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?,
        )?;
        let rows = sqlx::query("SELECT * FROM local_repository_links WHERE account_id=? AND instance_id=? AND repository_provider_id=? AND actor_id=? ORDER BY id LIMIT 129").bind(account_id).bind(instance_id).bind(&repository.provider_id).bind(&account.actor_id).fetch_all(&mut *tx).await.map_err(storage_error)?;
        if rows.len() > MAX_LINKS {
            return Err(CollaborationError::invalid(
                "Too many linked local repositories",
            ));
        }
        let links = rows
            .iter()
            .map(|row| read_link(row, LocalLinkState::RemoteChanged, Some(repository.clone())))
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await.map_err(storage_error)?;
        Ok(links)
    }

    /// Resolve a saved PR head repository to one existing, effective fetch
    /// remote after revalidating the authored base-repository link. The caller
    /// supplies the head identity from this Store's own typed detail snapshot.
    pub async fn pull_checkout_link_source(
        &self,
        request: PullCheckoutLinkRequest,
    ) -> Result<PullCheckoutLinkSource> {
        validate_query(&request.query)?;
        for value in [
            &request.account_id,
            &request.authorization_epoch,
            &request.instance_id,
            &request.repository_id,
            &request.link.id,
            &request.link.generation,
        ] {
            validate_identifier(value)?;
        }
        if !plain(&request.head_repository.provider_id, 256)
            || !path(&request.head_repository.full_name)
        {
            return Err(CollaborationError::invalid(
                "Invalid saved pull request head repository",
            ));
        }
        let remote_digest = request.query.remote_digest.clone().ok_or_else(stale_link)?;
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, &request.account_id, true).await?;
        if account.authorization_epoch != request.authorization_epoch
            || identities::instance_in(&mut tx, &account).await?.id != request.instance_id
        {
            return Err(stale_link());
        }
        if !identities::accessible(
            &mut tx,
            &request.account_id,
            &request.repository_id,
            ResourceKind::Repository,
        )
        .await?
        {
            return Err(CollaborationError::new(
                ErrorCode::PermissionDenied,
                "Saved repository access is unavailable",
            ));
        }
        let base_repository = repository_from_row(
            &sqlx::query("SELECT json,selected FROM repositories WHERE account_id=? AND id=?")
                .bind(&request.account_id)
                .bind(&request.repository_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?,
        )?;
        let snapshot = snapshot_in(&mut tx, &request.query).await?;
        let linked = snapshot.links.iter().any(|link| {
            link.id == request.link.id
                && link.generation == request.link.generation
                && link.state == LocalLinkState::Linked
                && link.local_repository_id == request.query.local_repository_id
                && link.account_id == request.account_id
                && link.actor_id == account.actor_id
                && link.instance_id == request.instance_id
                && link.repository_id == request.repository_id
                && link.repository_provider_id == base_repository.provider_id
        });
        if !linked {
            return Err(stale_link());
        }
        let mut bindings = bindings_in(&mut tx).await?;
        bindings.extend(public_bindings());
        let source_endpoint = checkout_source(
            &request.query.endpoints,
            &bindings,
            &request.instance_id,
            &request.head_repository.full_name,
        )?;
        tx.commit().await.map_err(storage_error)?;
        Ok(PullCheckoutLinkSource {
            local_repository_id: request.query.local_repository_id,
            base_repository,
            source_endpoint,
            remote_digest,
            authorization_view: snapshot.authorization_view,
        })
    }
}

fn checkout_source(
    endpoints: &[LocalRemoteEndpoint],
    bindings: &[LocalTransportBinding],
    instance_id: &str,
    repository_full_name: &str,
) -> Result<LocalRemoteEndpoint> {
    let mut matches = endpoints
        .iter()
        .filter(|endpoint| endpoint.direction == LinkDirection::Fetch && endpoint.ordinal == 0)
        .filter_map(|endpoint| {
            mapped(endpoint, bindings)
                .ok()
                .filter(|(binding, path)| {
                    binding.instance_id == instance_id && path == repository_full_name
                })
                .map(|_| endpoint.clone())
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.remote_name.cmp(&right.remote_name));
    matches.dedup_by(|left, right| left.remote_name == right.remote_name);
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(CollaborationError::new(
            ErrorCode::NotFound,
            "No existing fetch remote matches the saved pull request source repository",
        )),
        _ => Err(CollaborationError::new(
            ErrorCode::NotReady,
            "More than one fetch remote matches the saved pull request source repository",
        )),
    }
}

fn stale_link() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::StaleView,
        "Repository link changed; inspect it again",
    )
}
fn plain(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn path(value: &str) -> bool {
    plain(value, 2048)
        && !value.contains(['%', '\\', '?', '#'])
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.starts_with('~'))
}
fn validate_query(query: &LocalLinkQuery) -> Result<()> {
    validate_identifier(&query.local_repository_id)?;
    if query.endpoints.len() > MAX_ENDPOINTS
        || query.registration_proof.is_some() != query.remote_digest.is_some()
        || (query.registration_proof.is_none() && !query.endpoints.is_empty())
    {
        return Err(CollaborationError::invalid(
            "Invalid local repository observation",
        ));
    }
    for value in [&query.registration_proof, &query.remote_digest]
        .into_iter()
        .flatten()
    {
        if !plain(value, 256) {
            return Err(CollaborationError::invalid(
                "Invalid local repository proof",
            ));
        }
    }
    for endpoint in &query.endpoints {
        if !plain(&endpoint.remote_name, 255)
            || endpoint.ordinal >= 32
            || !path(&endpoint.path)
            || !valid_host(&endpoint.host)
            || endpoint.port == 0
        {
            return Err(CollaborationError::invalid("Invalid safe remote endpoint"));
        }
    }
    let mut keys = std::collections::HashSet::new();
    for endpoint in &query.endpoints {
        if !keys.insert(
            serde_json::to_string(&(&endpoint.remote_name, endpoint.direction, endpoint.ordinal))
                .map_err(|_| CollaborationError::storage())?,
        ) {
            return Err(CollaborationError::invalid(
                "Duplicate safe remote endpoint",
            ));
        }
    }
    Ok(())
}
fn valid_host(host: &str) -> bool {
    if !plain(host, 255) || host.contains(['@', '/', '?', '#', '%', '\\']) {
        return false;
    }
    url::Url::parse(&format!("https://{host}/"))
        .is_ok_and(|u| u.host_str() == Some(host) && u.port().is_none())
}
fn validate_binding(binding: &SaveTransportBinding) -> Result<()> {
    validate_identifier(&binding.instance_id)?;
    if !valid_host(&binding.host)
        || binding.port == 0
        || (!binding.path_prefix.is_empty() && !path(&binding.path_prefix))
    {
        return Err(CollaborationError::invalid("Invalid transport binding"));
    }
    Ok(())
}
async fn binding_generation(tx: &mut Transaction<'_, Sqlite>) -> Result<String> {
    let value: i64 =
        sqlx::query_scalar("SELECT bindings_generation FROM local_link_meta WHERE singleton=1")
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
    Ok(value.to_string())
}
async fn binding_changed(tx: &mut Transaction<'_, Sqlite>) -> Result<()> {
    sqlx::query(
        "UPDATE local_link_meta SET bindings_generation=bindings_generation+1 WHERE singleton=1",
    )
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    let rows = sqlx::query("SELECT id,authorization_epoch FROM accounts ORDER BY id LIMIT 101")
        .fetch_all(&mut **tx)
        .await
        .map_err(storage_error)?;
    if rows.len() > 100 {
        return Err(CollaborationError::storage());
    }
    for row in rows {
        record_change(
            tx,
            row.get("id"),
            row.get("authorization_epoch"),
            "local_transport_bindings",
            false,
        )
        .await?;
    }
    Ok(())
}
async fn bindings_in(tx: &mut Transaction<'_, Sqlite>) -> Result<Vec<LocalTransportBinding>> {
    let rows = sqlx::query("SELECT * FROM local_transport_bindings ORDER BY id LIMIT 129")
        .fetch_all(&mut **tx)
        .await
        .map_err(storage_error)?;
    if rows.len() > 128 {
        return Err(CollaborationError::storage());
    }
    rows.iter()
        .map(|r| {
            Ok(LocalTransportBinding {
                id: r.get("id"),
                instance_id: r.get("instance_id"),
                transport: decode(&format!("\"{}\"", r.get::<String, _>("transport")))?,
                host: r.get("host"),
                port: r
                    .get::<i64, _>("port")
                    .try_into()
                    .map_err(|_| CollaborationError::storage())?,
                path_prefix: r.get("path_prefix"),
                layout: decode(&format!("\"{}\"", r.get::<String, _>("layout")))?,
                generation: r.get::<i64, _>("generation").to_string(),
            })
        })
        .collect()
}
fn read_link(
    row: &sqlx::sqlite::SqliteRow,
    state: LocalLinkState,
    repository: Option<RemoteRepository>,
) -> Result<LocalRepositoryLink> {
    Ok(LocalRepositoryLink {
        id: row.get("id"),
        local_repository_id: row.get("local_repository_id"),
        endpoint: decode(row.get("endpoint_json"))?,
        account_id: row.get("account_id"),
        actor_id: row.get("actor_id"),
        instance_id: row.get("instance_id"),
        repository_provider_id: row.get("repository_provider_id"),
        repository_id: row.get("repository_id"),
        generation: row.get::<i64, _>("generation").to_string(),
        state,
        repository,
    })
}

fn public_bindings() -> Vec<LocalTransportBinding> {
    let mut bindings = Vec::new();
    for (provider, host, layout) in [
        (
            ProviderKind::Github,
            "github.com",
            RepositoryPathLayout::OwnerRepository,
        ),
        (
            ProviderKind::Gitlab,
            "gitlab.com",
            RepositoryPathLayout::Subgroups,
        ),
        (
            ProviderKind::BitbucketCloud,
            "bitbucket.org",
            RepositoryPathLayout::OwnerRepository,
        ),
    ] {
        for transport in [LinkTransport::Https, LinkTransport::Ssh, LinkTransport::Scp] {
            bindings.push(LocalTransportBinding {
                id: format!("builtin:{host}:{}", tag(&transport).unwrap_or_default()),
                instance_id: ProviderInstance::public(provider).id,
                transport,
                host: host.into(),
                port: if transport == LinkTransport::Https {
                    443
                } else {
                    22
                },
                path_prefix: String::new(),
                layout,
                generation: "0".into(),
            });
        }
    }
    bindings
}

fn mapped<'a>(
    endpoint: &LocalRemoteEndpoint,
    bindings: &'a [LocalTransportBinding],
) -> std::result::Result<(&'a LocalTransportBinding, String), LocalLinkState> {
    let matching: Vec<_> = bindings
        .iter()
        .filter(|b| {
            b.transport == endpoint.transport
                && b.host == endpoint.host
                && b.port == endpoint.port
                && (b.path_prefix.is_empty()
                    || endpoint
                        .path
                        .strip_prefix(&b.path_prefix)
                        .is_some_and(|p| p.starts_with('/')))
        })
        .collect();
    let Some(maximum) = matching.iter().map(|b| b.path_prefix.len()).max() else {
        return Err(LocalLinkState::UnconfiguredInstance);
    };
    let matching: Vec<_> = matching
        .into_iter()
        .filter(|b| b.path_prefix.len() == maximum)
        .collect();
    if matching.len() != 1 {
        return Err(LocalLinkState::Ambiguous);
    }
    let binding = matching[0];
    let relative = if binding.path_prefix.is_empty() {
        endpoint.path.as_str()
    } else {
        &endpoint.path[binding.path_prefix.len() + 1..]
    };
    let repository_path = relative.strip_suffix(".git").unwrap_or(relative);
    let parts: Vec<_> = repository_path.split('/').collect();
    if !path(repository_path)
        || parts.len() < 2
        || (binding.layout == RepositoryPathLayout::OwnerRepository && parts.len() != 2)
    {
        return Err(LocalLinkState::UnsupportedTransport);
    }
    Ok((binding, repository_path.to_owned()))
}

async fn resolve_endpoint(
    tx: &mut Transaction<'_, Sqlite>,
    endpoint: &LocalRemoteEndpoint,
    bindings: &[LocalTransportBinding],
) -> Result<LocalEndpointResolution> {
    let mut result = LocalEndpointResolution {
        endpoint: endpoint.clone(),
        state: LocalLinkState::Unresolved,
        candidates: vec![],
    };
    let (binding, repository_path) = match mapped(endpoint, bindings) {
        Ok(value) => value,
        Err(state) => {
            result.state = state;
            return Ok(result);
        }
    };
    let rows = sqlx::query("SELECT a.account_id,a.entity_id,i.provider_id FROM resource_aliases a JOIN resource_identities i ON i.account_id=a.account_id AND i.instance_id=a.instance_id AND i.entity_id=a.entity_id WHERE a.instance_id=? AND a.kind='repository' AND a.alias_kind='repository_path' AND a.value=? ORDER BY a.account_id,a.entity_id LIMIT 129")
        .bind(&binding.instance_id).bind(&repository_path).fetch_all(&mut **tx).await.map_err(storage_error)?;
    if rows.len() > 128 {
        return Err(CollaborationError::invalid(
            "Too many cached repository identity claims",
        ));
    }
    let mut accounts = std::collections::BTreeMap::<String, Vec<&sqlx::sqlite::SqliteRow>>::new();
    for row in &rows {
        accounts.entry(row.get("account_id")).or_default().push(row);
    }
    let mut ambiguous = false;
    for (account_id, claims) in accounts {
        let account = account_in(tx, &account_id, false).await?;
        if identities::instance_in(tx, &account).await?.id != binding.instance_id {
            return Err(CollaborationError::storage());
        }
        if account.state != AccountState::Active {
            result.state = LocalLinkState::Unavailable;
            continue;
        }
        // All historical claims participate, including currently hidden ones.
        if claims.len() > 1 {
            for claim in claims {
                if identities::accessible(
                    tx,
                    &account_id,
                    claim.get("entity_id"),
                    ResourceKind::Repository,
                )
                .await?
                {
                    ambiguous = true;
                    break;
                }
            }
            result.state = LocalLinkState::Unavailable;
            continue;
        }
        let claim = claims[0];
        if !identities::accessible(
            tx,
            &account_id,
            claim.get("entity_id"),
            ResourceKind::Repository,
        )
        .await?
        {
            result.state = LocalLinkState::Unavailable;
            continue;
        }
        let repository = repository_from_row(
            &sqlx::query("SELECT json,selected FROM repositories WHERE account_id=? AND id=?")
                .bind(&account_id)
                .bind(claim.get::<String, _>("entity_id"))
                .fetch_one(&mut **tx)
                .await
                .map_err(storage_error)?,
        )?;
        if repository.id != claim.get::<String, _>("entity_id")
            || repository.provider_id != claim.get::<String, _>("provider_id")
            || repository.account_id != account_id
        {
            return Err(CollaborationError::storage());
        }
        let identity = serde_json::to_vec(&(
            endpoint,
            &account_id,
            &account.actor_id,
            &account.authorization_epoch,
            &binding.instance_id,
            &repository.provider_id,
        ))
        .map_err(|_| CollaborationError::storage())?;
        let digest = Sha256::digest(identity);
        let id = digest.iter().map(|b| format!("{b:02x}")).collect();
        result.candidates.push(LocalLinkCandidate {
            id,
            endpoint: endpoint.clone(),
            account_id,
            actor_id: account.actor_id,
            authorization_epoch: account.authorization_epoch,
            instance_id: binding.instance_id.clone(),
            repository,
        });
    }
    if ambiguous {
        result.state = LocalLinkState::Ambiguous;
    } else if !result.candidates.is_empty() {
        result.state = LocalLinkState::Linked;
    }
    Ok(result)
}

async fn snapshot_in(
    tx: &mut Transaction<'_, Sqlite>,
    query: &LocalLinkQuery,
) -> Result<LocalLinkSnapshot> {
    let (revision, authorization_view) = metadata(tx).await?;
    let bindings_generation = binding_generation(tx).await?;
    let bindings = bindings_in(tx).await?;
    let mut effective_bindings = bindings.clone();
    effective_bindings.extend(public_bindings());
    let mut resolutions = Vec::new();
    let mut candidate_count = 0usize;
    for endpoint in &query.endpoints {
        let resolution = resolve_endpoint(tx, endpoint, &effective_bindings).await?;
        candidate_count += resolution.candidates.len();
        if candidate_count > 128 {
            return Err(CollaborationError::invalid(
                "Too many local repository link choices",
            ));
        }
        resolutions.push(resolution);
    }
    let rows = sqlx::query(
        "SELECT * FROM local_repository_links WHERE local_repository_id=? ORDER BY id LIMIT 129",
    )
    .bind(&query.local_repository_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(storage_error)?;
    if rows.len() > MAX_LINKS {
        return Err(CollaborationError::storage());
    }
    let mut links = Vec::new();
    for row in rows {
        let endpoint: LocalRemoteEndpoint = decode(row.get("endpoint_json"))?;
        let account = account_in(tx, row.get("account_id"), false).await?;
        let resolution = resolutions.iter().find(|r| r.endpoint == endpoint);
        let proof_matches = query.registration_proof.as_deref()
            == Some(row.get("registration_proof"))
            && query.remote_digest.as_deref() == Some(row.get("remote_digest"));
        let candidate = resolution.and_then(|r| {
            r.candidates.iter().find(|c| {
                c.account_id == account.id
                    && c.actor_id == row.get::<String, _>("actor_id")
                    && c.instance_id == row.get::<String, _>("instance_id")
                    && c.repository.provider_id == row.get::<String, _>("repository_provider_id")
                    && c.repository.id == row.get::<String, _>("repository_id")
            })
        });
        let state = if query.registration_proof.is_none() {
            LocalLinkState::LocalRepositoryMissing
        } else if !proof_matches {
            LocalLinkState::RemoteChanged
        } else if account.state != AccountState::Active {
            LocalLinkState::Unavailable
        } else if let Some(candidate) = candidate {
            if candidate.repository.account_id != account.id {
                return Err(CollaborationError::storage());
            }
            LocalLinkState::Linked
        } else {
            resolution.map_or(LocalLinkState::RemoteChanged, |r| {
                if r.state == LocalLinkState::Linked {
                    LocalLinkState::RemoteChanged
                } else {
                    r.state
                }
            })
        };
        // Even a changed local proof cannot expose provider-derived fields unless
        // the currently resolved actor/instance/native identity is authorized.
        let repository = if state == LocalLinkState::Linked {
            candidate.map(|c| c.repository.clone())
        } else {
            None
        };
        links.push(read_link(&row, state, repository)?);
    }
    Ok(LocalLinkSnapshot {
        links,
        resolutions,
        bindings,
        bindings_generation,
        revision,
        authorization_view,
    })
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    // This transaction fixture needs no item/detail projections or HTTP actor.
    async fn seed_repository(store: &Store) {
        let account = store
            .upsert_account(RemoteAccount {
                id: "a".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "a".into(),
                login: "a".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            })
            .await
            .unwrap();
        let scope = "repositories";
        let run_id = store
            .begin_sync("a", &account.authorization_epoch, scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: "a".into(),
                authorization_epoch: account.authorization_epoch,
                scope: scope.into(),
                run_id,
                repositories: vec![RemoteRepository {
                    id: "repo".into(),
                    account_id: "a".into(),
                    provider_id: "1".into(),
                    full_name: "owner/project".into(),
                    name: "project".into(),
                    web_url: "https://github.com/owner/project".into(),
                    description: None,
                    default_branch: None,
                    selected: false,
                }],
                items: vec![],
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-03T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }

    async fn fixture() -> (
        tempfile::TempDir,
        Store,
        ConfirmLocalLink,
        LocalLinkSnapshot,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(directory.path().join("db")).await.unwrap();
        seed_repository(&store).await;
        let query = LocalLinkQuery {
            local_repository_id: "local".into(),
            registration_proof: Some("worktree".into()),
            remote_digest: Some("safe-configuration".into()),
            endpoints: vec![LocalRemoteEndpoint {
                remote_name: "origin".into(),
                direction: LinkDirection::Fetch,
                ordinal: 0,
                transport: LinkTransport::Https,
                host: "github.com".into(),
                port: 443,
                path: "owner/project.git".into(),
            }],
        };
        let snapshot = store.local_link_snapshot(query.clone()).await.unwrap();
        let request = ConfirmLocalLink {
            query,
            candidate_id: snapshot.resolutions[0].candidates[0].id.clone(),
            expected_authorization_view: snapshot.authorization_view.clone(),
            expected_bindings_generation: snapshot.bindings_generation.clone(),
            replace: None,
        };
        (directory, store, request, snapshot)
    }
    #[tokio::test]
    async fn retired_native_caller_waiting_for_writer_cannot_commit() {
        let (_directory, store, request, before) = fixture().await;
        let query = request.query.clone();
        let writer = store.inner.writer.lock().await;
        let authorized = Arc::new(AtomicBool::new(true));
        let owner = authorized.clone();
        let worker_store = store.clone();
        let task = tokio::spawn(async move {
            worker_store
                .confirm_local_link_checked(request, || {
                    if owner.load(Ordering::SeqCst) {
                        Ok(())
                    } else {
                        Err(CollaborationError::new(
                            ErrorCode::PermissionDenied,
                            "Retired synthetic caller",
                        ))
                    }
                })
                .await
        });
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        authorized.store(false, Ordering::SeqCst);
        drop(writer);
        assert_eq!(
            task.await.unwrap().unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        assert_eq!(store.local_link_snapshot(query).await.unwrap(), before);
    }
    #[tokio::test]
    async fn native_owner_recheck_after_authored_write_rolls_back_all_changes() {
        use std::sync::atomic::AtomicUsize;
        let (_directory, store, request, before) = fixture().await;
        let query = request.query.clone();
        let checks = AtomicUsize::new(0);
        let error = store
            .confirm_local_link_checked(request, || {
                if checks.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(())
                } else {
                    Err(CollaborationError::new(
                        ErrorCode::PermissionDenied,
                        "Retired synthetic caller",
                    ))
                }
            })
            .await
            .unwrap_err();
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        assert_eq!(error.code, ErrorCode::PermissionDenied);
        assert_eq!(store.local_link_snapshot(query).await.unwrap(), before);
    }

    #[tokio::test]
    async fn all_native_authored_writes_fence_waiting_and_precommit_caller_retirement() {
        use std::sync::atomic::AtomicUsize;
        for operation in 0..3 {
            for after_write in [false, true] {
                let (_directory, store, request, _) = fixture().await;
                let query = request.query.clone();
                let link = store.confirm_local_link(request).await.unwrap().link;
                let instance = store.provider_instance("a").await.unwrap();
                let request = SaveTransportBinding {
                    instance_id: instance.id,
                    transport: LinkTransport::Scp,
                    host: "fixture-alias".into(),
                    port: 22,
                    path_prefix: String::new(),
                    layout: RepositoryPathLayout::OwnerRepository,
                    expected_bindings_generation: store
                        .local_link_snapshot(query.clone())
                        .await
                        .unwrap()
                        .bindings_generation,
                    replace: None,
                };
                let binding = if operation == 2 {
                    Some(store.save_transport_binding(request.clone()).await.unwrap())
                } else {
                    None
                };
                let before = store.local_link_snapshot(query.clone()).await.unwrap();
                let writer = if after_write {
                    None
                } else {
                    Some(store.inner.writer.lock().await)
                };
                let active = Arc::new(AtomicBool::new(true));
                let owner = active.clone();
                let worker = store.clone();
                let generation = before.bindings_generation.clone();
                let task = tokio::spawn(async move {
                    let checks = AtomicUsize::new(0);
                    let guard = || {
                        if owner.load(Ordering::SeqCst)
                            && (!after_write || checks.fetch_add(1, Ordering::SeqCst) == 0)
                        {
                            Ok(())
                        } else {
                            Err(CollaborationError::new(
                                ErrorCode::PermissionDenied,
                                "Retired synthetic caller",
                            ))
                        }
                    };
                    match operation {
                        0 => worker
                            .remove_local_link_checked(&link.id, &link.generation, guard)
                            .await
                            .map(|_| ()),
                        1 => worker
                            .save_transport_binding_checked(request, guard)
                            .await
                            .map(|_| ()),
                        _ => {
                            let binding = binding.unwrap();
                            worker
                                .remove_transport_binding_checked(
                                    &binding.id,
                                    &binding.generation,
                                    &generation,
                                    guard,
                                )
                                .await
                                .map(|_| ())
                        }
                    }
                });
                if !after_write {
                    tokio::task::yield_now().await;
                    assert!(!task.is_finished());
                    active.store(false, Ordering::SeqCst);
                    drop(writer);
                }
                assert_eq!(
                    task.await.unwrap().unwrap_err().code,
                    ErrorCode::PermissionDenied
                );
                assert_eq!(
                    store.local_link_snapshot(query).await.unwrap(),
                    before,
                    "Every failure preserves authored intent, binding generations and revision"
                );
            }
        }
    }

    #[tokio::test]
    async fn sqlite_interrupt_after_authored_write_rolls_back_link_generation_and_revision() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(directory.path().join("db")).await.unwrap();
        seed_repository(&store).await;
        let query = LocalLinkQuery {
            local_repository_id: "local".into(),
            registration_proof: Some("worktree".into()),
            remote_digest: Some("safe-configuration".into()),
            endpoints: vec![LocalRemoteEndpoint {
                remote_name: "origin".into(),
                direction: LinkDirection::Fetch,
                ordinal: 0,
                transport: LinkTransport::Https,
                host: "github.com".into(),
                port: 443,
                path: "owner/project.git".into(),
            }],
        };
        let before = store.local_link_snapshot(query.clone()).await.unwrap();
        let candidate = before.resolutions[0].candidates[0].id.clone();
        let inserted = Arc::new(AtomicBool::new(false));
        {
            let mut writer = store.inner.writer.lock().await;
            let mut handle = writer.lock_handle().await.unwrap();
            let observed = inserted.clone();
            handle.set_update_hook(move |change| {
                if change.table == "local_repository_links" {
                    observed.store(true, Ordering::SeqCst);
                }
            });
            let observed = inserted.clone();
            let mut interrupted = false;
            handle.set_progress_handler(1, move || {
                if observed.load(Ordering::SeqCst) && !interrupted {
                    interrupted = true;
                    false
                } else {
                    true
                }
            });
        }
        let error = store
            .confirm_local_link(ConfirmLocalLink {
                query: query.clone(),
                candidate_id: candidate,
                expected_authorization_view: before.authorization_view.clone(),
                expected_bindings_generation: before.bindings_generation.clone(),
                replace: None,
            })
            .await
            .unwrap_err();
        {
            let mut writer = store.inner.writer.lock().await;
            let mut handle = writer.lock_handle().await.unwrap();
            handle.remove_update_hook();
            handle.remove_progress_handler();
        }
        assert!(
            inserted.load(Ordering::SeqCst),
            "Interrupt follows a real partial authored write"
        );
        assert_eq!(error.code, ErrorCode::Storage);
        assert_eq!(store.local_link_snapshot(query).await.unwrap(), before);
    }

    #[test]
    fn checkout_source_selects_only_the_forks_primary_fetch_endpoint() {
        let instance = ProviderInstance::public(ProviderKind::Github).id;
        let endpoints = vec![
            LocalRemoteEndpoint {
                remote_name: "origin".into(),
                direction: LinkDirection::Fetch,
                ordinal: 0,
                transport: LinkTransport::Https,
                host: "github.com".into(),
                port: 443,
                path: "base/project.git".into(),
            },
            LocalRemoteEndpoint {
                remote_name: "fork".into(),
                direction: LinkDirection::Fetch,
                ordinal: 0,
                transport: LinkTransport::Ssh,
                host: "github.com".into(),
                port: 22,
                path: "actor/project.git".into(),
            },
            LocalRemoteEndpoint {
                remote_name: "fork".into(),
                direction: LinkDirection::Fetch,
                ordinal: 1,
                transport: LinkTransport::Https,
                host: "github.com".into(),
                port: 443,
                path: "other/project.git".into(),
            },
            LocalRemoteEndpoint {
                remote_name: "push-fork".into(),
                direction: LinkDirection::Push,
                ordinal: 0,
                transport: LinkTransport::Https,
                host: "github.com".into(),
                port: 443,
                path: "actor/project.git".into(),
            },
        ];
        let source =
            checkout_source(&endpoints, &public_bindings(), &instance, "actor/project").unwrap();
        assert_eq!(source.remote_name, "fork");
        assert_eq!(source.direction, LinkDirection::Fetch);
        assert_eq!(source.ordinal, 0);
    }

    #[test]
    fn checkout_source_fails_closed_for_missing_or_ambiguous_fork_remotes() {
        let instance = ProviderInstance::public(ProviderKind::Github).id;
        let endpoint = |name: &str| LocalRemoteEndpoint {
            remote_name: name.into(),
            direction: LinkDirection::Fetch,
            ordinal: 0,
            transport: LinkTransport::Https,
            host: "github.com".into(),
            port: 443,
            path: "actor/project.git".into(),
        };
        assert_eq!(
            checkout_source(
                &[endpoint("origin")],
                &public_bindings(),
                &instance,
                "missing/project",
            )
            .unwrap_err()
            .code,
            ErrorCode::NotFound
        );
        assert_eq!(
            checkout_source(
                &[endpoint("origin"), endpoint("fork")],
                &public_bindings(),
                &instance,
                "actor/project",
            )
            .unwrap_err()
            .code,
            ErrorCode::NotReady
        );
    }
}
