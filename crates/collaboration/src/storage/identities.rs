//! Identity metadata shares the writer transaction with each observation.
use super::*;

pub(super) async fn bind_account_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
) -> Result<()> {
    let instance = ProviderInstance::for_account(account)?;
    sqlx::query("INSERT INTO provider_instances(id,provider,base_url) VALUES(?,?,?) ON CONFLICT(id) DO NOTHING")
        .bind(&instance.id).bind(tag(&instance.provider)?).bind(&instance.base_url)
        .execute(&mut **tx).await.map_err(storage_error)?;
    sqlx::query("INSERT INTO account_instances(account_id,instance_id) VALUES(?,?) ON CONFLICT(account_id) DO NOTHING")
        .bind(&account.id).bind(&instance.id).execute(&mut **tx).await.map_err(storage_error)?;
    let stored = instance_in(tx, account).await?;
    if stored != instance {
        return Err(CollaborationError::invalid(
            "Account provider instance cannot be changed",
        ));
    }
    Ok(())
}

async fn instance_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
) -> Result<ProviderInstance> {
    let row = sqlx::query("SELECT p.id,p.provider,p.base_url FROM provider_instances p JOIN account_instances a ON a.instance_id=p.id WHERE a.account_id=?")
        .bind(&account.id).fetch_one(&mut **tx).await.map_err(storage_error)?;
    let instance = ProviderInstance::new(account.provider, row.get("base_url"))?;
    if instance.id != row.get::<String, _>("id")
        || tag(&instance.provider)? != row.get::<String, _>("provider")
        || instance != ProviderInstance::for_account(account)?
    {
        return Err(CollaborationError::storage());
    }
    Ok(instance)
}

async fn alias_in(
    tx: &mut Transaction<'_, Sqlite>,
    resource: &CanonicalResource,
    alias_kind: LocatorKind,
    value: &str,
    repository_path: &str,
) -> Result<()> {
    if value.is_empty()
        || value.chars().any(char::is_control)
        || value.len() > 2048
        || repository_path.len() > 2048
    {
        return Err(CollaborationError::invalid(
            "Resource alias exceeds the limit",
        ));
    }
    let normalized = if alias_kind == LocatorKind::WebUrl {
        Some(normalize_web_url(value)?)
    } else {
        None
    };
    let value = normalized.as_deref().unwrap_or(value);
    sqlx::query("INSERT OR IGNORE INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,repository_path,entity_id) VALUES(?,?,?,?,?,?,?)")
        .bind(&resource.account_id).bind(&resource.instance_id).bind(tag(&resource.kind)?)
        .bind(tag(&alias_kind)?).bind(value).bind(repository_path).bind(&resource.id)
        .execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

async fn identity_in(
    tx: &mut Transaction<'_, Sqlite>,
    resource: &CanonicalResource,
    repository_provider_id: &str,
    number: Option<&str>,
) -> Result<()> {
    validate_identifier(&resource.provider_id)?;
    // Adapters must keep their existing opaque projection ID for the same native
    // identity. An endpoint representation is an alias, never a replacement ID.
    let existing: Option<String> = sqlx::query_scalar("SELECT entity_id FROM resource_identities WHERE account_id=? AND instance_id=? AND kind=? AND provider_id=?")
        .bind(&resource.account_id).bind(&resource.instance_id).bind(tag(&resource.kind)?).bind(&resource.provider_id)
        .fetch_optional(&mut **tx).await.map_err(storage_error)?;
    if existing.as_ref().is_some_and(|id| id != &resource.id) {
        return Err(CollaborationError::invalid(
            "Provider changed a canonical identity",
        ));
    }
    let prior: Option<(String,String)> = sqlx::query_as("SELECT kind,provider_id FROM resource_identities WHERE account_id=? AND instance_id=? AND entity_id=?")
        .bind(&resource.account_id).bind(&resource.instance_id).bind(&resource.id).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    if prior.is_some_and(|(kind, id)| {
        kind != tag(&resource.kind).unwrap_or_default() || id != resource.provider_id
    }) {
        return Err(CollaborationError::invalid(
            "Canonical identity cannot be repurposed",
        ));
    }
    sqlx::query("INSERT INTO resource_identities(account_id,instance_id,entity_id,kind,provider_id,repository_provider_id,number) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,instance_id,entity_id) DO UPDATE SET repository_provider_id=excluded.repository_provider_id,number=excluded.number")
        .bind(&resource.account_id).bind(&resource.instance_id).bind(&resource.id).bind(tag(&resource.kind)?).bind(&resource.provider_id).bind(repository_provider_id).bind(number)
        .execute(&mut **tx).await.map_err(storage_error)?;
    alias_in(tx, resource, LocatorKind::Canonical, &resource.id, "").await?;
    alias_in(
        tx,
        resource,
        LocatorKind::Native,
        &format!("{}:{}", tag(&resource.kind)?, resource.provider_id),
        "",
    )
    .await?;
    Ok(())
}

pub(super) async fn repository_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    repo: &RemoteRepository,
) -> Result<()> {
    let instance = instance_in(tx, account).await?;
    let resource = CanonicalResource {
        account_id: account.id.clone(),
        instance_id: instance.id,
        id: repo.id.clone(),
        kind: ResourceKind::Repository,
        provider_id: repo.provider_id.clone(),
    };
    identity_in(tx, &resource, "", None).await?;
    alias_in(
        tx,
        &resource,
        LocatorKind::RepositoryPath,
        &repo.full_name,
        "",
    )
    .await?;
    alias_in(tx, &resource, LocatorKind::WebUrl, &repo.web_url, "").await?;
    // A rename creates aliases for already cached resource numbers under the new
    // path; old aliases stay tied to their immutable identities.
    let prior_path: Option<String> =
        sqlx::query_scalar("SELECT full_name FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(&repo.id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    if prior_path
        .as_ref()
        .is_some_and(|path| path != &repo.full_name)
    {
        // Let SQLite update a large repository without materializing every
        // resource in native memory during each discovery refresh.
        sqlx::query("INSERT OR IGNORE INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,repository_path,entity_id) SELECT account_id,instance_id,kind,'repository_number',number,?,entity_id FROM resource_identities WHERE account_id=? AND instance_id=? AND repository_provider_id=? AND number IS NOT NULL")
            .bind(&repo.full_name).bind(&account.id).bind(&resource.instance_id).bind(&repo.provider_id).execute(&mut **tx).await.map_err(storage_error)?;
    }
    Ok(())
}

pub(super) async fn item_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    item: &RemoteItem,
) -> Result<()> {
    let instance = instance_in(tx, account).await?;
    let kind = match item.kind {
        RemoteItemKind::PullRequest => ResourceKind::PullRequest,
        RemoteItemKind::Issue => ResourceKind::Issue,
        RemoteItemKind::Notification => ResourceKind::Notification,
    };
    let repo: Option<(String, String)> = if let Some(id) = &item.repository_id {
        sqlx::query_as("SELECT provider_id,full_name FROM repositories WHERE account_id=? AND id=?")
            .bind(&account.id)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?
    } else {
        None
    };
    let resource = CanonicalResource {
        account_id: account.id.clone(),
        instance_id: instance.id,
        id: item.id.clone(),
        kind,
        provider_id: item.provider_id.clone(),
    };
    identity_in(
        tx,
        &resource,
        repo.as_ref().map(|r| r.0.as_str()).unwrap_or(""),
        item.number.as_deref(),
    )
    .await?;
    if let Some(web_url) = &item.web_url {
        alias_in(tx, &resource, LocatorKind::WebUrl, web_url, "").await?;
    }
    if let (Some((native, path)), Some(number)) = (repo, &item.number) {
        alias_in(tx, &resource, LocatorKind::RepositoryNumber, number, &path).await?;
        if kind == ResourceKind::PullRequest {
            bind_pending_in(tx, account, &resource.instance_id, &native, number).await?;
        }
    }
    Ok(())
}

pub(super) async fn endpoint_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    alias: &EndpointAlias,
    scope: &str,
) -> Result<()> {
    // Only explicitly marked pull representations can converge with pulls.
    if alias.kind != ResourceKind::PullRequest {
        return Err(CollaborationError::invalid(
            "Invalid endpoint representation",
        ));
    }
    validate_identifier(&alias.native_identity)?;
    validate_identifier(&alias.repository_provider_id)?;
    validate_number(&alias.number)?;
    if alias
        .web_url
        .as_ref()
        .is_some_and(|url| url.len() > 2048 || url.chars().any(char::is_control))
    {
        return Err(CollaborationError::invalid(
            "Endpoint alias exceeds the limit",
        ));
    }
    let repository: Option<String> =
        sqlx::query_scalar("SELECT id FROM repositories WHERE account_id=? AND provider_id=?")
            .bind(&account.id)
            .bind(&alias.repository_provider_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage_error)?;
    if repository.is_none_or(|id| scope != format!("repo:{id}:issue")) {
        return Err(CollaborationError::invalid(
            "Endpoint alias has the wrong repository scope",
        ));
    }
    let instance = instance_in(tx, account).await?;
    sqlx::query("INSERT INTO pending_endpoint_aliases(account_id,instance_id,kind,repository_provider_id,number,native_identity,web_url) VALUES(?,?,?,?,?,?,?) ON CONFLICT(account_id,instance_id,kind,repository_provider_id,number,native_identity) DO UPDATE SET web_url=excluded.web_url")
        .bind(&account.id).bind(&instance.id).bind(tag(&alias.kind)?).bind(&alias.repository_provider_id).bind(&alias.number).bind(&alias.native_identity).bind(&alias.web_url)
        .execute(&mut **tx).await.map_err(storage_error)?;
    bind_pending_in(
        tx,
        account,
        &instance.id,
        &alias.repository_provider_id,
        &alias.number,
    )
    .await
}

async fn bind_pending_in(
    tx: &mut Transaction<'_, Sqlite>,
    account: &RemoteAccount,
    instance_id: &str,
    native_repo: &str,
    number: &str,
) -> Result<()> {
    let resources = sqlx::query("SELECT entity_id,provider_id FROM resource_identities WHERE account_id=? AND instance_id=? AND kind='pull_request' AND repository_provider_id=? AND number=?")
        .bind(&account.id).bind(instance_id).bind(native_repo).bind(number).fetch_all(&mut **tx).await.map_err(storage_error)?;
    // Contradictory identities remain unresolved rather than choosing a winner.
    if resources.len() != 1 {
        return Ok(());
    }
    let resource = CanonicalResource {
        account_id: account.id.clone(),
        instance_id: instance_id.into(),
        id: resources[0].get("entity_id"),
        kind: ResourceKind::PullRequest,
        provider_id: resources[0].get("provider_id"),
    };
    let aliases = sqlx::query("SELECT native_identity,web_url FROM pending_endpoint_aliases WHERE account_id=? AND instance_id=? AND kind='pull_request' AND repository_provider_id=? AND number=?")
        .bind(&account.id).bind(instance_id).bind(native_repo).bind(number).fetch_all(&mut **tx).await.map_err(storage_error)?;
    for row in aliases {
        alias_in(
            tx,
            &resource,
            LocatorKind::Native,
            row.get("native_identity"),
            "",
        )
        .await?;
        if let Some(url) = row.get::<Option<String>, _>("web_url") {
            alias_in(tx, &resource, LocatorKind::WebUrl, &url, "").await?;
        }
    }
    // Keep the representation evidence for later transfer and diagnostics.
    Ok(())
}

fn validate_number(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.starts_with('0')
    {
        return Err(CollaborationError::invalid(
            "Invalid resource display number",
        ));
    }
    Ok(())
}

fn normalize_web_url(value: &str) -> Result<String> {
    let mut url =
        url::Url::parse(value).map_err(|_| CollaborationError::invalid("Invalid resource URL"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CollaborationError::invalid("Invalid resource URL"));
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url.to_string())
}

async fn accessible(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    id: &str,
    kind: ResourceKind,
) -> Result<bool> {
    if kind == ResourceKind::Repository {
        let mut query = sqlx::QueryBuilder::<Sqlite>::new(
            "SELECT EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=",
        );
        query
            .push_bind(account)
            .push(" AND r.id=")
            .push_bind(id)
            .push(" AND ")
            .push(VISIBLE_REPOSITORY)
            .push(")");
        query
            .build_query_scalar()
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)
    } else {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items i WHERE i.account_id=? AND i.id=? AND (i.kind='notification' OR EXISTS(SELECT 1 FROM repositories r WHERE r.account_id=i.account_id AND r.id=i.repository_id AND r.selected=1)) AND NOT EXISTS(SELECT 1 FROM sync_scopes s WHERE s.account_id=i.account_id AND s.scope=CASE WHEN i.kind='notification' THEN 'notifications' ELSE 'repo:'||i.repository_id||':'||i.kind END AND s.access_denied=1) AND (i.kind='notification' OR NOT EXISTS(SELECT 1 FROM sync_scopes s JOIN scope_membership m ON m.account_id=s.account_id AND m.scope=s.scope WHERE s.account_id=i.account_id AND s.scope='repositories' AND s.access_denied=1 AND m.entity_id=i.repository_id)))")
            .bind(account).bind(id).fetch_one(&mut **tx).await.map_err(storage_error)
    }
}

impl Store {
    pub async fn provider_instance(&self, account_id: &str) -> Result<ProviderInstance> {
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let instance = instance_in(&mut tx, &account).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(instance)
    }

    /// Metadata-only local resolution. Alias retention never grants access.
    pub async fn resolve_resource(
        &self,
        account_id: &str,
        locator: ResourceLocator,
    ) -> Result<ResourceResolution> {
        if locator.value.is_empty()
            || locator.value.chars().any(char::is_control)
            || locator.value.len() > 2048
            || locator
                .repository_path
                .as_ref()
                .is_some_and(|p| p.is_empty() || p.chars().any(char::is_control) || p.len() > 2048)
        {
            return Err(CollaborationError::invalid(
                "Resource locator exceeds the limit",
            ));
        }
        if locator.locator_kind == LocatorKind::RepositoryNumber {
            validate_number(&locator.value)?;
        }
        if (locator.locator_kind == LocatorKind::RepositoryNumber)
            != locator.repository_path.is_some()
            || (locator.locator_kind == LocatorKind::RepositoryPath
                && locator.kind != ResourceKind::Repository)
        {
            return Err(CollaborationError::invalid(
                "Invalid structured resource locator",
            ));
        }
        let mut tx = self.inner.readers.begin().await.map_err(storage_error)?;
        let account = account_in(&mut tx, account_id, false).await?;
        let instance = instance_in(&mut tx, &account).await?;
        if instance.id != locator.instance_id {
            return Err(CollaborationError::invalid(
                "Locator belongs to another provider instance",
            ));
        }
        let locator_value = if locator.locator_kind == LocatorKind::WebUrl {
            let url = url::Url::parse(&locator.value)
                .map_err(|_| CollaborationError::invalid("Invalid resource URL"))?;
            let base =
                url::Url::parse(&instance.base_url).map_err(|_| CollaborationError::storage())?;
            if url.origin() != base.origin()
                || !url.path().starts_with(base.path())
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(CollaborationError::invalid(
                    "Resource URL belongs to another provider instance",
                ));
            }
            normalize_web_url(&locator.value)?
        } else {
            locator.value
        };
        let (revision, authorization_view) = metadata(&mut tx).await?;
        let mut resolution = ResourceResolution {
            state: ResolutionState::Unavailable,
            resource: None,
            candidates: vec![],
            revision,
            authorization_view,
        };
        if account.state != AccountState::Active {
            tx.commit().await.map_err(storage_error)?;
            return Ok(resolution);
        }
        let repository_path = locator.repository_path.unwrap_or_default();
        let path_claims: i64 = if locator.locator_kind == LocatorKind::RepositoryNumber {
            sqlx::query_scalar("SELECT COUNT(*) FROM resource_aliases WHERE account_id=? AND instance_id=? AND kind='repository' AND alias_kind='repository_path' AND value=?")
                .bind(account_id).bind(&instance.id).bind(&repository_path).fetch_one(&mut *tx).await.map_err(storage_error)?
        } else if locator.locator_kind == LocatorKind::WebUrl {
            // Do not parse provider-specific route layouts here. A persisted
            // repository URL prefix carries the parent installation/path claim.
            sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT value FROM resource_aliases WHERE account_id=? AND instance_id=? AND kind='repository' AND alias_kind='web_url' AND (value=? OR substr(?,1,length(value)+1)=value||'/') GROUP BY value HAVING COUNT(*)>1)")
                .bind(account_id).bind(&instance.id).bind(&locator_value).bind(&locator_value).fetch_one(&mut *tx).await.map_err(storage_error)?
        } else {
            0
        };
        let rows = sqlx::query("SELECT r.entity_id,r.provider_id FROM resource_aliases a JOIN resource_identities r ON r.account_id=a.account_id AND r.instance_id=a.instance_id AND r.entity_id=a.entity_id WHERE a.account_id=? AND a.instance_id=? AND a.kind=? AND a.alias_kind=? AND a.value=? AND a.repository_path=? ORDER BY r.entity_id LIMIT 101")
            .bind(account_id).bind(&instance.id).bind(tag(&locator.kind)?).bind(tag(&locator.locator_kind)?).bind(&locator_value).bind(repository_path)
            .fetch_all(&mut *tx).await.map_err(storage_error)?;
        if rows.is_empty() {
            resolution.state = ResolutionState::Unresolved;
        }
        let ambiguous = rows.len() > 1
            || (locator.locator_kind == LocatorKind::RepositoryNumber && path_claims > 1)
            || (locator.locator_kind == LocatorKind::WebUrl && path_claims > 0);
        for row in rows.into_iter().take(100) {
            let id: String = row.get("entity_id");
            if accessible(&mut tx, account_id, &id, locator.kind).await? {
                resolution.candidates.push(CanonicalResource {
                    account_id: account_id.into(),
                    instance_id: instance.id.clone(),
                    id,
                    kind: locator.kind,
                    provider_id: row.get("provider_id"),
                });
            }
        }
        if !resolution.candidates.is_empty() {
            if ambiguous {
                resolution.state = ResolutionState::Ambiguous;
            } else {
                resolution.state = ResolutionState::Resolved;
                resolution.resource = resolution.candidates.pop();
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(resolution)
    }
}
