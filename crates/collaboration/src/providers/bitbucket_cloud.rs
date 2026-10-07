//! Public Bitbucket Cloud manual API tokens and bounded member repositories.
use super::*;
use serde::Deserialize;

mod commits;
mod discovery;
mod feeds;
mod participants;
mod resource_details;
mod tasks;
mod transport;
use transport::{BitbucketHttp, Route, invalid, quota};

pub struct BitbucketCloudProvider {
    http: BitbucketHttp,
}

impl BitbucketCloudProvider {
    pub fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: BitbucketHttp::new()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture(base: reqwest::Url) -> Self {
        Self {
            http: BitbucketHttp::fixture(base),
        }
    }
}

#[async_trait]
impl CollaborationProvider for BitbucketCloudProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::BitbucketCloud
    }

    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        ProviderProfile {
            inbox_semantics: InboxSemantics::None,
            facets: super::FACETS
                .into_iter()
                .map(|facet| FacetCapability {
                    facet,
                    state: if matches!(
                        facet,
                        ResourceFacet::Repositories
                            | ResourceFacet::PullRequests
                            | ResourceFacet::PullDetails
                            | ResourceFacet::Participants
                            | ResourceFacet::Tasks
                            | ResourceFacet::PullCommits
                    ) {
                        CapabilityState::Supported
                    } else {
                        CapabilityState::Unsupported
                    },
                    reason: match facet {
                        ResourceFacet::Repositories
                        | ResourceFacet::PullRequests
                        | ResourceFacet::PullDetails
                        | ResourceFacet::Participants
                        | ResourceFacet::Tasks
                        | ResourceFacet::PullCommits => None,
                        ResourceFacet::Issues | ResourceFacet::Inbox => {
                            Some(CapabilityReason::ProviderSemantics)
                        }
                        _ => Some(CapabilityReason::NotImplemented),
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
        let unproven = |error| ProbeFailure {
            error,
            verified_actor_id: None,
        };
        let response = self
            .http
            .get(
                self.http.endpoint(&Route::User).map_err(unproven)?,
                &Route::User,
                token,
            )
            .await
            .map_err(unproven)?;
        let actor: Actor = serde_json::from_slice(&response.body)
            .map_err(|_| unproven(quota(invalid(), response.cooldown)))?;
        if actor.kind != "user"
            || actor
                .account_status
                .as_deref()
                .is_some_and(|state| state != "active")
        {
            return Err(unproven(quota(
                ProviderError::new(ProviderErrorKind::Authentication),
                response.cooldown,
            )));
        }
        let actor_id = canonical_uuid(&actor.uuid)
            .map_err(|error| unproven(quota(error, response.cooldown)))?;
        let proven = |error| ProbeFailure {
            error,
            verified_actor_id: Some(actor_id.clone()),
        };
        let login = actor
            .nickname
            .map(|value| text(value, 255, false))
            .transpose()
            .map_err(|error| proven(quota(error, response.cooldown)))?
            .unwrap_or_else(|| actor_id.clone());
        let display_name = actor
            .display_name
            .map(|value| text(value, 1024, false))
            .transpose()
            .map_err(|error| proven(quota(error, response.cooldown)))?;
        if let Some(wait) = response.cooldown {
            return Err(proven(transport::rate_limited(wait)));
        }
        let route = Route::Workspaces;
        let page = self
            .http
            .get(self.http.endpoint(&route).map_err(proven)?, &route, token)
            .await
            .map_err(proven)?;
        let workspaces = discovery::workspaces(&page, &self.http, &route)
            .map_err(|error| proven(quota(error, page.cooldown)))?;
        let cooldown = if let Some(workspace) = workspaces.first() {
            if let Some(wait) = page.cooldown {
                return Err(proven(transport::rate_limited(wait)));
            }
            let route = Route::Repositories(workspace.clone());
            let repositories = self
                .http
                .get(self.http.endpoint(&route).map_err(proven)?, &route, token)
                .await
                .map_err(proven)?;
            // All probes complete before the runtime stages a secret. Probe
            // observations do not establish cached membership or coverage.
            discovery::repositories(&repositories, "probe", workspace, &self.http, &route)
                .map_err(|error| proven(quota(error, repositories.cooldown)))?;
            repositories.cooldown
        } else {
            page.cooldown
        };
        Ok(VerifiedAccount {
            actor_id,
            login,
            display_name,
            notifications_supported: false,
            cooldown_seconds: cooldown,
        })
    }

    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        match request.kind {
            FeedKind::Repositories => self.discover(token, request).await,
            FeedKind::PullRequests => self.resource_feed(token, request).await,
            _ => Err(ProviderError::new(ProviderErrorKind::Unsupported)),
        }
    }

    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        match request.facet {
            DetailFacet::Participants => self.participants(token, request).await,
            DetailFacet::Tasks => self.tasks(token, request).await,
            _ => self.resource_details(token, request).await,
        }
    }

    async fn fetch_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        self.pull_commits(token, request).await
    }
}

#[derive(Deserialize)]
struct Actor {
    #[serde(rename = "type")]
    kind: String,
    uuid: String,
    nickname: Option<String>,
    display_name: Option<String>,
    account_status: Option<String>,
}

pub(super) fn canonical_uuid(raw: &str) -> Result<String, ProviderError> {
    let unwrapped = raw
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(raw);
    if unwrapped.len() != 36 {
        return Err(invalid());
    }
    let value = uuid::Uuid::parse_str(unwrapped).map_err(|_| invalid())?;
    if value.is_nil() || !value.to_string().eq_ignore_ascii_case(unwrapped) {
        return Err(invalid());
    }
    Ok(value.to_string())
}

pub(super) fn text(value: String, max: usize, empty: bool) -> Result<String, ProviderError> {
    if value.len() > max || (!empty && value.is_empty()) || value.chars().any(char::is_control) {
        Err(invalid())
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reads_tests;

#[cfg(test)]
mod participants_tests;

#[cfg(test)]
mod commits_tests;

#[cfg(test)]
mod tasks_tests;
