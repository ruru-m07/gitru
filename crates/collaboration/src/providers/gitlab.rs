//! GitLab.com manual PAT, member repositories and cached MR/issue reads.
use super::*;
use serde::Deserialize;
mod checks;
mod commits;
mod feeds;
mod resource_details;
mod transport;
use transport::{GitlabHttp, invalid, max_wait, positive_id, project_after, with_quota};

pub struct GitlabProvider {
    http: GitlabHttp,
}
impl GitlabProvider {
    pub fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GitlabHttp::new()?,
        })
    }
    #[cfg(test)]
    pub(crate) fn fixture(base: reqwest::Url) -> Self {
        Self {
            http: GitlabHttp::fixture(base),
        }
    }
    fn repositories(
        &self,
        body: &[u8],
        account: &str,
        after: Option<u64>,
    ) -> Result<Vec<RemoteRepository>, ProviderError> {
        let projects: Vec<Project> = serde_json::from_slice(body).map_err(|_| invalid())?;
        if projects.len() > 50 {
            return Err(invalid());
        }
        let mut last = after.unwrap_or(0);
        projects
            .into_iter()
            .map(|project| {
                if project.id <= last {
                    return Err(invalid());
                }
                last = project.id;
                project.remote(account)
            })
            .collect()
    }
    async fn projects(
        &self,
        token: &SecretToken,
        account: &str,
        cursor: Option<&str>,
    ) -> Result<FetchPage, ProviderError> {
        let endpoint = match cursor {
            Some(raw) => self.http.continuation(raw)?,
            None => self.http.projects(),
        };
        let after = project_after(&endpoint)?;
        let response = self.http.get(endpoint, token).await?;
        let repositories = self
            .repositories(&response.body, account, after)
            .map_err(|e| with_quota(e, response.cooldown))?;
        if let Some(next) = &response.next {
            let next_after = project_after(&self.http.continuation(next)?)?.ok_or_else(invalid)?;
            // The documented keyset filter excludes the last observed ID. A
            // non-progressing or fabricated skip cannot establish coverage.
            if repositories
                .last()
                .and_then(|r| positive_id(&r.provider_id))
                != Some(next_after)
                || next_after <= after.unwrap_or(0)
            {
                return Err(with_quota(invalid(), response.cooldown));
            }
        }
        Ok(FetchPage {
            repositories,
            items: vec![],
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: response.next,
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: response.cooldown,
        })
    }
}

fn unproven(error: ProviderError) -> ProbeFailure {
    ProbeFailure {
        error,
        verified_actor_id: None,
    }
}

#[async_trait]
impl CollaborationProvider for GitlabProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gitlab
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        ProviderProfile {
            inbox_semantics: InboxSemantics::None,
            facets: super::FACETS
                .into_iter()
                .map(|facet| FacetCapability {
                    facet,
                    state: if implemented(facet) {
                        CapabilityState::Supported
                    } else {
                        CapabilityState::Unsupported
                    },
                    reason: if implemented(facet) {
                        None
                    } else if facet == ResourceFacet::Inbox {
                        Some(CapabilityReason::ProviderSemantics)
                    } else {
                        Some(CapabilityReason::NotImplemented)
                    },
                })
                .collect(),
        }
    }
    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        self.probe_with_backoff(token)
            .await
            .map_err(|failure| failure.error)
    }
    async fn probe_with_backoff(
        &self,
        token: &SecretToken,
    ) -> Result<VerifiedAccount, ProbeFailure> {
        let response = self
            .http
            .get(self.http.user(), token)
            .await
            .map_err(unproven)?;
        let actor: Actor = serde_json::from_slice(&response.body)
            .map_err(|_| unproven(with_quota(invalid(), response.cooldown)))?;
        if actor.id == 0
            || actor
                .state
                .as_deref()
                .is_some_and(|state| state != "active")
        {
            return Err(unproven(with_quota(
                ProviderError::new(ProviderErrorKind::Authentication),
                response.cooldown,
            )));
        }
        let login =
            bounded(actor.username, 255).map_err(|e| unproven(with_quota(e, response.cooldown)))?;
        if login.is_empty() {
            return Err(unproven(with_quota(invalid(), response.cooldown)));
        }
        let display_name = actor
            .name
            .map(|name| bounded(name, 1024))
            .transpose()
            .map_err(|e| unproven(with_quota(e, response.cooldown)))?;
        let actor_id = actor.id.to_string();
        let proven = |error| ProbeFailure {
            error,
            verified_actor_id: Some(actor_id.clone()),
        };
        if let Some(wait) = response.cooldown {
            return Err(proven(ProviderError {
                kind: ProviderErrorKind::RateLimited,
                retry_after_seconds: Some(wait),
                account_cooldown_seconds: Some(wait),
            }));
        }
        // Both actual reads must succeed before runtime stages any credential.
        // An empty member list is valid and does not create cache coverage.
        let projects = self.projects(token, "probe", None).await.map_err(proven)?;
        Ok(VerifiedAccount {
            actor_id,
            login,
            display_name,
            notifications_supported: false,
            cooldown_seconds: max_wait(response.cooldown, projects.cooldown_seconds),
        })
    }
    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        if request.account.provider != ProviderKind::Gitlab || request.account.host != "gitlab.com"
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        if request.account.state != AccountState::Active {
            return Err(ProviderError::new(ProviderErrorKind::Authentication));
        }
        if request.kind != FeedKind::Repositories {
            return self.resource_feed(token, request).await;
        }
        if request.repository.is_some() {
            return Err(invalid());
        }
        // GitLab keyset reads remain unconditional: page-one validators cannot
        // prove later pages unchanged and totals are not completeness evidence.
        self.projects(token, &request.account.id, request.cursor.as_deref())
            .await
    }
    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        self.resource_details(token, request).await
    }

    async fn fetch_checks(
        &self,
        token: &SecretToken,
        request: CheckRequest,
    ) -> Result<DetailPage, ProviderError> {
        self.request_checks(token, request).await
    }

    async fn fetch_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        self.request_pull_commits(token, request).await
    }
}

fn implemented(facet: ResourceFacet) -> bool {
    matches!(
        facet,
        ResourceFacet::Repositories
            | ResourceFacet::PullRequests
            | ResourceFacet::Issues
            | ResourceFacet::PullDetails
            | ResourceFacet::IssueDetails
            | ResourceFacet::PullCommits
            | ResourceFacet::Checks
    )
}

#[derive(Deserialize)]
struct Actor {
    id: u64,
    username: String,
    name: Option<String>,
    state: Option<String>,
}
#[derive(Deserialize)]
struct Project {
    id: u64,
    path_with_namespace: String,
    path: String,
    name: String,
    web_url: String,
    description: Option<String>,
    default_branch: Option<String>,
}
impl Project {
    fn remote(self, account: &str) -> Result<RemoteRepository, ProviderError> {
        if self.id == 0
            || !valid_path(&self.path_with_namespace)
            || !valid_segment(&self.path)
            || self.path_with_namespace.rsplit('/').next() != Some(self.path.as_str())
        {
            return Err(invalid());
        }
        let url = reqwest::Url::parse(&self.web_url).map_err(|_| invalid())?;
        let expected = format!("https://gitlab.com/{}", self.path_with_namespace);
        if self.web_url.len() > 2048
            || url.as_str() != expected
            || url.scheme() != "https"
            || url.host_str() != Some("gitlab.com")
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid());
        }
        Ok(RemoteRepository {
            id: format!("gitlab:repository:{}", self.id),
            account_id: account.into(),
            provider_id: self.id.to_string(),
            full_name: self.path_with_namespace,
            name: bounded(self.name, 1024)?,
            web_url: expected,
            description: self
                .description
                .map(|value| bounded(value, 16 * 1024))
                .transpose()?,
            default_branch: self
                .default_branch
                .map(|value| bounded(value, 1024))
                .transpose()?,
            selected: false,
        })
    }
}
fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
fn valid_path(value: &str) -> bool {
    value.len() <= 1024 && value.contains('/') && value.split('/').all(valid_segment)
}
fn bounded(value: String, max: usize) -> Result<String, ProviderError> {
    if value.len() > max {
        Err(invalid())
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod checks_tests;
#[cfg(test)]
mod commits_tests;
#[cfg(test)]
pub(crate) mod reads_tests;
#[cfg(test)]
pub(crate) mod tests;
