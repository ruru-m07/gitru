//! GitHub.com REST adapter, pinned to API version 2026-03-10.
//! Notification support requires a scoped classic PAT or imported OAuth token.

use super::{
    transport::{GithubHttp, HttpValidators},
    *,
};
use serde::Deserialize;
mod activity;
#[cfg(test)]
mod activity_tests;
mod checks;
pub(crate) mod comment_send;
mod comments;
mod commits;
mod files;
mod issue_details;
mod notification_subject_discovery;
pub mod notification_subjects;
mod pull_details;
mod resource_details;
mod reviews;
pub(crate) mod text_edits;

pub struct GithubProvider {
    http: GithubHttp,
}

impl GithubProvider {
    pub fn new() -> Result<Self, CollaborationError> {
        Ok(Self {
            http: GithubHttp::new()?,
        })
    }
    #[cfg(test)]
    pub(crate) fn for_test_base(base: reqwest::Url) -> Self {
        Self {
            http: GithubHttp::for_test_base(base).expect("fixture transport"),
        }
    }
}

#[async_trait]
impl CollaborationProvider for GithubProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }

    fn notification_subject_support(
        &self,
        account: &RemoteAccount,
        _: crate::NotificationSubjectKind,
    ) -> CapabilityState {
        if account.provider != ProviderKind::Github || account.host != "github.com" {
            CapabilityState::Unsupported
        } else if account.state != AccountState::Active || !account.notifications_supported {
            CapabilityState::Unavailable
        } else {
            CapabilityState::Supported
        }
    }

    async fn discover_notification_subject(
        &self,
        token: &SecretToken,
        request: TrustedNotificationSubjectRequest,
    ) -> Result<NotificationSubjectDiscovery, ProviderError> {
        self.request_notification_subject_discovery(token, request)
            .await
    }

    fn profile(&self, account: &RemoteAccount) -> ProviderProfile {
        let mut profile = ProviderProfile::read_only(
            InboxSemantics::NativeNotifications,
            account.notifications_supported,
        );
        for facet in &mut profile.facets {
            if matches!(
                facet.facet,
                ResourceFacet::PullDetails | ResourceFacet::IssueDetails
            ) || matches!(
                facet.facet,
                ResourceFacet::Comments
                    | ResourceFacet::Activity
                    | ResourceFacet::PullCommits
                    | ResourceFacet::PullFiles
                    | ResourceFacet::Reviews
                    | ResourceFacet::Checks
            ) && account.provider == ProviderKind::Github
                && account.host == "github.com"
            {
                facet.state = CapabilityState::Supported;
                facet.reason = None;
            }
        }
        profile
    }

    async fn fetch_detail(
        &self,
        token: &SecretToken,
        request: DetailRequest,
    ) -> Result<DetailPage, ProviderError> {
        if request.facet == DetailFacet::Activity {
            return self.request_activity(token, request).await;
        }
        if request.facet == DetailFacet::Comments {
            return self.request_comments(token, request).await;
        }
        match &request.subject.kind {
            RemoteItemKind::PullRequest => {
                self.request_resource_details(
                    token,
                    request,
                    resource_details::PULL_SOURCE,
                    pull_details::normalize,
                )
                .await
            }
            RemoteItemKind::Issue => {
                self.request_resource_details(
                    token,
                    request,
                    issue_details::ISSUE_SOURCE,
                    issue_details::normalize,
                )
                .await
            }
            RemoteItemKind::Notification => Err(ProviderError::new(ProviderErrorKind::Unsupported)),
        }
    }

    async fn fetch_pull_files(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileProviderPage, ProviderError> {
        self.request_pull_files(token, request).await
    }
    async fn fetch_pull_file_artifact(
        &self,
        token: &SecretToken,
        request: PullFileSelectedRequest,
    ) -> Result<PullFileArtifactRead, ProviderError> {
        self.request_selected_pull_file(token, request).await
    }
    async fn validate_selected_pull_file_range(
        &self,
        token: &SecretToken,
        request: PullFileSelectedRequest,
    ) -> Result<PullFileRangeValidationResult, ProviderError> {
        request
            .validate()
            .map_err(|_| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
        self.request_file_source_range(token, request.resource)
            .await
    }
    async fn validate_pull_file_range(
        &self,
        token: &SecretToken,
        request: PullFileCollectionRequest,
    ) -> Result<PullFileRangeValidationResult, ProviderError> {
        self.request_pull_file_range(token, request).await
    }
    async fn fetch_checks(
        &self,
        token: &SecretToken,
        request: CheckRequest,
    ) -> Result<DetailPage, ProviderError> {
        self.request_checks(token, request).await
    }

    async fn fetch_reviews(
        &self,
        token: &SecretToken,
        request: ReviewRequest,
    ) -> Result<DetailPage, ProviderError> {
        self.request_reviews(token, request).await
    }

    async fn fetch_pull_commits(
        &self,
        token: &SecretToken,
        request: PullCommitRequest,
    ) -> Result<PullCommitProviderPage, ProviderError> {
        self.request_pull_commits(token, request).await
    }

    async fn probe(&self, token: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        let response = self
            .http
            .get(
                self.http.endpoint("user")?,
                token,
                &HttpValidators::default(),
            )
            .await?;
        let actor: GithubActor = decode(&response.body)?;
        let scopes = response.oauth_scopes.as_deref().unwrap_or("");
        let notifications_supported = notification_access(token, scopes);
        Ok(VerifiedAccount {
            actor_id: actor.id.to_string(),
            login: bounded(actor.login, 255)?,
            display_name: actor.name.map(|name| bounded(name, 1024)).transpose()?,
            notifications_supported,
            cooldown_seconds: None,
        })
    }

    async fn fetch_page(
        &self,
        token: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        if request.account.provider != ProviderKind::Github || request.account.host != "github.com"
        {
            return Err(ProviderError::new(ProviderErrorKind::Unsupported));
        }
        let mut endpoint = match request.kind {
            FeedKind::Repositories => self.http.endpoint("user/repos")?,
            FeedKind::Notifications => {
                if !request.account.notifications_supported {
                    return Err(ProviderError::new(ProviderErrorKind::Unsupported));
                }
                self.http.endpoint("notifications")?
            }
            FeedKind::PullRequests | FeedKind::Issues => {
                let repo = request
                    .repository
                    .as_ref()
                    .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
                if repo.account_id != request.account.id || !valid_repository_path(&repo.full_name)
                {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                let resource = if request.kind == FeedKind::PullRequests {
                    "pulls"
                } else {
                    "issues"
                };
                self.http
                    .endpoint(&format!("repos/{}/{resource}", repo.full_name))?
            }
        };
        let mut allowed_paths = vec![endpoint.path().to_string()];
        if matches!(request.kind, FeedKind::PullRequests | FeedKind::Issues) {
            let repo = request
                .repository
                .as_ref()
                .expect("repository validated above");
            let native_id = repo
                .provider_id
                .parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
            let resource = if request.kind == FeedKind::PullRequests {
                "pulls"
            } else {
                "issues"
            };
            allowed_paths.push(format!("/repositories/{native_id}/{resource}"));
        }
        {
            let mut query = endpoint.query_pairs_mut();
            match request.kind {
                FeedKind::Repositories => {
                    query
                        .append_pair("sort", "updated")
                        .append_pair("direction", "desc")
                        .append_pair("per_page", "100");
                }
                FeedKind::Notifications => {
                    query
                        .append_pair("all", "true")
                        .append_pair("per_page", "50");
                }
                FeedKind::PullRequests | FeedKind::Issues => {
                    query
                        .append_pair("state", "all")
                        .append_pair("sort", "updated")
                        .append_pair("direction", "desc")
                        .append_pair("per_page", "100");
                }
            }
        }
        if let Some(cursor) = &request.cursor {
            endpoint = if allowed_paths.len() == 1 {
                self.http.check_page_url(cursor, &allowed_paths[0])?
            } else {
                self.http
                    .check_page_url_with_paths(cursor, &allowed_paths)?
            };
        }
        let response = self
            .http
            .get_with_paths(
                endpoint,
                token,
                &HttpValidators {
                    etag: request.etag,
                    last_modified: request.last_modified,
                },
                &allowed_paths,
            )
            .await?;
        let mut page = FetchPage {
            repositories: Vec::new(),
            items: Vec::new(),
            endpoint_aliases: Vec::new(),
            notification_subjects: Vec::new(),
            next_cursor: response.next_url,
            etag: response.validators.etag,
            last_modified: response.validators.last_modified,
            not_modified: response.not_modified,
            poll_interval_seconds: response.poll_interval_seconds,
            cooldown_seconds: response.cooldown_seconds,
        };
        if page.not_modified {
            return Ok(page);
        }
        match request.kind {
            FeedKind::Repositories => {
                let repos: Vec<GithubRepository> = decode(&response.body)?;
                if repos.len() > 100 {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                page.repositories = repos
                    .into_iter()
                    .map(|repo| repo.into_remote(&request.account.id))
                    .collect::<Result<_, _>>()?;
            }
            FeedKind::PullRequests => {
                let pulls: Vec<GithubPull> = decode(&response.body)?;
                if pulls.len() > 100 {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                let repo = request
                    .repository
                    .as_ref()
                    .expect("repository validated above");
                page.items = pulls
                    .into_iter()
                    .map(|pull| pull.into_remote(&request.account.id, repo))
                    .collect::<Result<_, _>>()?;
            }
            FeedKind::Issues => {
                let issues: Vec<GithubIssue> = decode(&response.body)?;
                if issues.len() > 100 {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                let repo = request
                    .repository
                    .as_ref()
                    .expect("repository validated above");
                for issue in issues {
                    if issue.pull_request.is_some() {
                        page.endpoint_aliases.push(EndpointAlias {
                            kind: ResourceKind::PullRequest,
                            repository_provider_id: repo.provider_id.clone(),
                            number: issue.number.to_string(),
                            native_identity: format!("issue:{}", issue.id),
                            web_url: Some(bounded(issue.html_url, 2048)?),
                        });
                    } else {
                        page.items
                            .push(issue.into_remote(&request.account.id, repo)?);
                    }
                }
            }
            FeedKind::Notifications => {
                let notifications: Vec<GithubNotification> = decode(&response.body)?;
                if notifications.len() > 50 {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                for notification in notifications {
                    let repo = notification.repository.into_remote(&request.account.id)?;
                    let mapping = notification_subjects::normalize(
                        &reqwest::Url::parse("https://api.github.com/")
                            .expect("constant API origin"),
                        &repo,
                        &notification.subject,
                    );
                    let item = RemoteItem {
                        id: format!(
                            "github:notification:{}",
                            bounded(notification.id.clone(), 128)?
                        ),
                        account_id: request.account.id.clone(),
                        repository_id: Some(repo.id.clone()),
                        provider_id: notification.id,
                        kind: RemoteItemKind::Notification,
                        number: None,
                        title: bounded(
                            notification
                                .subject
                                .get("title")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("Notification")
                                .to_string(),
                            16 * 1024,
                        )?,
                        body: None,
                        body_omitted: true,
                        author: None,
                        // API subject URLs are not web URLs. Until subject hydration
                        // provides an html_url, use a safe repository destination.
                        web_url: Some(repo.web_url.clone()),
                        state: notification
                            .subject
                            .get("type")
                            .and_then(serde_json::Value::as_str)
                            .filter(|kind| kind.len() <= 128)
                            .unwrap_or("Unknown")
                            .to_string(),
                        updated_at: bounded(notification.updated_at, 128)?,
                        head_oid: None,
                        is_draft: None,
                        reason: Some(bounded(notification.reason, 255)?),
                        unread: Some(notification.unread),
                        native_inbox: Some(NativeInboxState::Notification {
                            unread: notification.unread,
                        }),
                    };
                    page.notification_subjects
                        .push(crate::NotificationSubjectObservation {
                            notification_id: item.id.clone(),
                            mapping,
                        });
                    page.repositories.push(repo);
                    page.items.push(item);
                }
            }
        }
        Ok(page)
    }
}

#[cfg(test)]
mod checks_tests;
#[cfg(test)]
mod comments_tests;
#[cfg(test)]
mod commits_tests;
#[cfg(test)]
mod reviews_tests;

fn notification_access(token: &SecretToken, scopes: &str) -> bool {
    // gh commonly stores an existing OAuth user token. It is not a PAT, but
    // both known token families use this classic scope contract. Token prefixes
    // alone never imply permissions; fine-grained PATs remain unsupported here.
    (token.expose().starts_with("ghp_") || token.expose().starts_with("gho_"))
        && scopes
            .split(',')
            .any(|scope| matches!(scope.trim(), "notifications" | "repo"))
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ProviderError> {
    serde_json::from_slice(bytes)
        .map_err(|_| ProviderError::new(ProviderErrorKind::InvalidResponse))
}

fn bounded(value: String, max_bytes: usize) -> Result<String, ProviderError> {
    if value.len() > max_bytes {
        Err(ProviderError::new(ProviderErrorKind::InvalidResponse))
    } else {
        Ok(value)
    }
}

fn summary_body(body: Option<String>) -> (Option<String>, bool) {
    if body.as_ref().is_some_and(|body| body.len() > 64 * 1024) {
        (None, true)
    } else {
        (body, false)
    }
}

fn valid_repository_path(path: &str) -> bool {
    let segments: Vec<_> = path.split('/').collect();
    segments.len() == 2
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && *segment != "."
                && *segment != ".."
                && segment.len() <= 255
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}

#[derive(Deserialize)]
struct GithubActor {
    id: u64,
    login: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct GithubRepository {
    id: u64,
    full_name: String,
    name: String,
    html_url: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    default_branch: Option<String>,
}

impl GithubRepository {
    fn into_remote(self, account_id: &str) -> Result<RemoteRepository, ProviderError> {
        if !valid_repository_path(&self.full_name) {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        Ok(RemoteRepository {
            id: format!("github:repository:{}", self.id),
            account_id: account_id.to_string(),
            provider_id: self.id.to_string(),
            full_name: self.full_name,
            name: bounded(self.name, 255)?,
            web_url: bounded(self.html_url, 2048)?,
            description: self.description.filter(|value| value.len() <= 16 * 1024),
            default_branch: self
                .default_branch
                .map(|branch| bounded(branch, 1024))
                .transpose()?,
            selected: false,
        })
    }
}

#[derive(Deserialize)]
struct GithubPull {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    #[serde(default)]
    draft: Option<bool>,
    user: Option<GithubActor>,
    html_url: String,
    updated_at: String,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    head: Option<GithubHead>,
}

#[derive(Deserialize)]
struct GithubHead {
    sha: String,
}

impl GithubPull {
    fn into_remote(
        self,
        account: &str,
        repo: &RemoteRepository,
    ) -> Result<RemoteItem, ProviderError> {
        let (body, body_omitted) = summary_body(self.body);
        Ok(RemoteItem {
            native_inbox: None,
            id: format!("github:pull:{}", self.id),
            account_id: account.to_string(),
            repository_id: Some(repo.id.clone()),
            provider_id: self.id.to_string(),
            kind: RemoteItemKind::PullRequest,
            number: Some(self.number.to_string()),
            title: bounded(self.title, 16 * 1024)?,
            body,
            body_omitted,
            author: self
                .user
                .map(|actor| bounded(actor.login, 255))
                .transpose()?,
            web_url: Some(bounded(self.html_url, 2048)?),
            state: if self.merged_at.is_some() {
                "merged".to_string()
            } else {
                bounded(self.state, 128)?
            },
            updated_at: bounded(self.updated_at, 128)?,
            head_oid: self.head.map(|head| bounded(head.sha, 128)).transpose()?,
            is_draft: self.draft,
            reason: None,
            unread: None,
        })
    }
}

#[derive(Deserialize)]
struct GithubIssue {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    user: Option<GithubActor>,
    html_url: String,
    updated_at: String,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

impl GithubIssue {
    fn into_remote(
        self,
        account: &str,
        repo: &RemoteRepository,
    ) -> Result<RemoteItem, ProviderError> {
        let (body, body_omitted) = summary_body(self.body);
        Ok(RemoteItem {
            native_inbox: None,
            id: format!("github:issue:{}", self.id),
            account_id: account.to_string(),
            repository_id: Some(repo.id.clone()),
            provider_id: self.id.to_string(),
            kind: RemoteItemKind::Issue,
            number: Some(self.number.to_string()),
            title: bounded(self.title, 16 * 1024)?,
            body,
            body_omitted,
            author: self
                .user
                .map(|actor| bounded(actor.login, 255))
                .transpose()?,
            web_url: Some(bounded(self.html_url, 2048)?),
            state: bounded(self.state, 128)?,
            updated_at: bounded(self.updated_at, 128)?,
            head_oid: None,
            is_draft: None,
            reason: None,
            unread: None,
        })
    }
}

#[derive(Deserialize)]
struct GithubNotification {
    id: String,
    repository: GithubRepository,
    subject: serde_json::Value,
    reason: String,
    unread: bool,
    updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_access_requires_known_scoped_user_credentials() {
        for family in ["ghp_fixture", "gho_fixture"] {
            let token = SecretToken::new(family.into()).unwrap();
            assert!(notification_access(&token, "repo, read:org"));
            assert!(notification_access(&token, "read:user, notifications"));
            assert!(!notification_access(&token, "read:user"));
            assert!(!notification_access(&token, ""));
            assert!(!notification_access(&token, "repository, notification"));
        }
        for family in ["github_pat_fixture", "ghs_fixture", "unknown_fixture"] {
            assert!(!notification_access(
                &SecretToken::new(family.into()).unwrap(),
                "repo, notifications"
            ));
        }
    }

    #[tokio::test]
    async fn issue_feed_keeps_pull_endpoint_identity_without_creating_an_issue() {
        let (provider, server) = fixture_server(|_| {
            let body = r#"[{"id":9007199254740993,"number":67,"title":"pull representation","body":null,"state":"open","user":null,"html_url":"https://github.com/old/repo/pull/67","updated_at":"2026-10-03T12:00:00Z","pull_request":{}},{"id":9007199254740994,"number":68,"title":"true issue","body":null,"state":"open","user":null,"html_url":"https://github.com/old/repo/issues/68","updated_at":"2026-10-03T12:00:00Z"}]"#;
            vec![format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )]
        });
        let mut request = fixture_request();
        request.kind = FeedKind::Issues;
        let page = provider
            .fetch_page(&SecretToken::new("fixture".into()).unwrap(), request)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].kind, RemoteItemKind::Issue);
        assert_eq!(page.endpoint_aliases.len(), 1);
        assert_eq!(
            page.endpoint_aliases[0].native_identity,
            "issue:9007199254740993"
        );
        assert_eq!(page.endpoint_aliases[0].repository_provider_id, "123");
        server.join().unwrap();
    }
    use std::io::{Read, Write};

    fn fixture_request() -> FeedRequest {
        FeedRequest {
            account: RemoteAccount {
                id: "account".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "1".into(),
                login: "actor".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            },
            kind: FeedKind::PullRequests,
            repository: Some(RemoteRepository {
                id: "github:repository:123".into(),
                account_id: "account".into(),
                provider_id: "123".into(),
                full_name: "old/repo".into(),
                name: "repo".into(),
                web_url: "https://github.com/old/repo".into(),
                description: None,
                default_branch: Some("main".into()),
                selected: true,
            }),
            cursor: None,
            etag: None,
            last_modified: None,
        }
    }

    fn fixture_server(
        responses: impl FnOnce(&str) -> Vec<String>,
    ) -> (GithubProvider, std::thread::JoinHandle<Vec<String>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let responses = responses(&base);
        let server = std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let mut requests = Vec::new();
            for response in responses {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "expected an adapter request"
                            );
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        Err(error) => panic!("fixture listener failed: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0u8; 1024];
                    let length = stream.read(&mut buffer).unwrap();
                    bytes.extend_from_slice(&buffer[..length]);
                    if length == 0 || bytes.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        break;
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                stream.write_all(response.as_bytes()).unwrap();
            }
            requests
        });
        let provider = GithubProvider {
            http: GithubHttp::for_test_base(reqwest::Url::parse(&base).unwrap()).unwrap(),
        };
        (provider, server)
    }

    #[test]
    fn repository_path_cannot_change_the_request_endpoint() {
        for path in ["owner/repo", "ruru-m07/gitru", "owner/repo.git"] {
            assert!(valid_repository_path(path));
        }
        for path in [
            "../user",
            "a/../user",
            "a/b?token=secret",
            "a/b#x",
            "a/b/c",
            "/a",
            "a/",
        ] {
            assert!(!valid_repository_path(path));
        }
    }

    #[test]
    fn merged_and_closed_remain_distinct_and_large_ids_are_strings() {
        let repo = GithubRepository {
            id: u64::MAX,
            full_name: "a/b".into(),
            name: "b".into(),
            html_url: "https://github.com/a/b".into(),
            description: None,
            default_branch: None,
        }
        .into_remote("account")
        .unwrap();
        let pull: GithubPull = decode(br#"{"id":18446744073709551615,"number":67,"title":"done","body":null,"state":"closed","draft":false,"user":null,"html_url":"https://github.com/a/b/pull/67","updated_at":"2026-10-02T00:00:00Z","merged_at":"2026-10-02T00:00:00Z","head":{"sha":"abc"}}"#).unwrap();
        let item = pull.into_remote("account", &repo).unwrap();
        assert_eq!(item.state, "merged");
        assert_eq!(item.provider_id, u64::MAX.to_string());
    }

    #[test]
    fn github_issues_that_are_prs_are_detected() {
        let issue: GithubIssue = decode(br#"{"id":1,"number":1,"title":"pr","body":null,"state":"open","user":null,"html_url":"https://github.com/a/b/pull/1","updated_at":"2026-10-02T00:00:00Z","pull_request":{"url":"https://api.github.com/repos/a/b/pulls/1"}}"#).unwrap();
        assert!(issue.pull_request.is_some());
        assert_eq!(summary_body(Some("x".repeat(65 * 1024))), (None, true));
        assert_eq!(summary_body(None), (None, false));
    }

    #[test]
    fn transferring_an_issue_changes_its_locator_but_keeps_native_identity() {
        let first_repo = GithubRepository {
            id: 1,
            full_name: "a/first".into(),
            name: "first".into(),
            html_url: "https://github.com/a/first".into(),
            description: None,
            default_branch: None,
        }
        .into_remote("account")
        .unwrap();
        let second_repo = GithubRepository {
            id: 2,
            full_name: "a/second".into(),
            name: "second".into(),
            html_url: "https://github.com/a/second".into(),
            description: None,
            default_branch: None,
        }
        .into_remote("account")
        .unwrap();
        let first: GithubIssue = decode(br#"{"id":987,"number":1,"title":"issue","body":null,"state":"open","user":null,"html_url":"https://github.com/a/first/issues/1","updated_at":"2026-10-02T00:00:00Z"}"#).unwrap();
        let second: GithubIssue = decode(br#"{"id":987,"number":19,"title":"issue","body":null,"state":"open","user":null,"html_url":"https://github.com/a/second/issues/19","updated_at":"2026-10-02T00:00:01Z"}"#).unwrap();
        let first = first.into_remote("account", &first_repo).unwrap();
        let second = second.into_remote("account", &second_repo).unwrap();
        assert_eq!(first.id, second.id);
        assert_ne!(first.repository_id, second.repository_id);
        assert_ne!(first.number, second.number);
    }

    #[tokio::test]
    async fn renamed_repository_redirect_and_canonical_page_cursor_share_exact_native_identity() {
        let (provider, server) = fixture_server(|base| {
            vec![
                format!(
                    "HTTP/1.1 301 Moved Permanently\r\nLocation: {base}repositories/123/pulls?state=all&per_page=100\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                ),
                format!(
                    "HTTP/1.1 200 OK\r\nLink: <{base}repositories/123/pulls?state=all&per_page=100&page=2>; rel=\"next\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]"
                ),
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]".into(),
            ]
        });
        let token = SecretToken::new("secret_token".into()).unwrap();
        let first = provider
            .fetch_page(&token, fixture_request())
            .await
            .unwrap();
        let mut continuation = fixture_request();
        continuation.cursor = first.next_cursor;
        assert!(
            continuation
                .cursor
                .as_ref()
                .unwrap()
                .contains("/repositories/123/pulls?")
        );
        let final_page = provider.fetch_page(&token, continuation).await.unwrap();
        assert!(final_page.next_cursor.is_none());
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /repos/old/repo/pulls?"));
        assert!(requests[1].starts_with("GET /repositories/123/pulls?"));
        assert!(requests[2].starts_with("GET /repositories/123/pulls?"));
        assert!(requests[2].lines().next().unwrap().contains("page=2"));
    }

    #[tokio::test]
    async fn wrong_repository_id_or_resource_cannot_become_a_persisted_continuation() {
        let provider = GithubProvider::new().unwrap();
        let token = SecretToken::new("secret_token".into()).unwrap();
        for path in [
            "repositories/999/pulls",
            "repositories/123/issues",
            "repos/another/repo/pulls",
            "user",
        ] {
            let mut request = fixture_request();
            request.cursor = Some(format!("https://api.github.com/{path}?page=2"));
            let error = provider.fetch_page(&token, request).await.unwrap_err();
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        }
        for path in ["repositories/999/pulls", "repositories/123/issues"] {
            let (provider, server) = fixture_server(|base| {
                vec![format!(
                    "HTTP/1.1 200 OK\r\nLink: <{base}{path}?page=2>; rel=\"next\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]"
                )]
            });
            let error = provider
                .fetch_page(&token, fixture_request())
                .await
                .unwrap_err();
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            assert_eq!(server.join().unwrap().len(), 1);
            let (provider, server) = fixture_server(|base| {
                vec![format!(
                    "HTTP/1.1 301 Moved Permanently\r\nLocation: {base}{path}?page=2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )]
            });
            let error = provider
                .fetch_page(&token, fixture_request())
                .await
                .unwrap_err();
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            assert_eq!(server.join().unwrap().len(), 1);
        }
    }
}

#[cfg(test)]
mod files_tests;

pub(crate) mod issue_creation;
pub(crate) mod workflow_state;
