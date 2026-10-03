//! Divergent fixtures validate the contract, not production GitLab support.
use super::*;
use serde::Deserialize;
use std::{
    io::{Read, Write},
    sync::Arc,
};

#[derive(Deserialize)]
struct GlActor {
    id: u64,
    username: String,
    name: Option<String>,
}
#[derive(Deserialize)]
struct GlProject {
    id: u64,
    path_with_namespace: String,
    name: String,
    web_url: String,
}
#[derive(Deserialize)]
struct GlItem {
    id: u64,
    iid: u64,
    project_id: u64,
    title: String,
    description: Option<String>,
    state: String,
    draft: Option<bool>,
    web_url: String,
    updated_at: String,
}
#[derive(Deserialize)]
struct GlTodo {
    id: u64,
    action_name: String,
    target_type: String,
    state: String,
    created_at: String,
    target: GlTarget,
}
#[derive(Deserialize)]
struct GlTarget {
    title: String,
}
#[derive(Deserialize)]
struct GlFixture {
    actor: GlActor,
    projects: Vec<GlProject>,
    merge_requests: Vec<GlItem>,
    issues: Vec<GlItem>,
    todos: Vec<GlTodo>,
}

struct GitlabFixture {
    instance: ProviderInstance,
    error: Option<ProviderError>,
}
impl GitlabFixture {
    fn new(instance: ProviderInstance) -> Self {
        Self {
            instance,
            error: None,
        }
    }
    fn fixture() -> GlFixture {
        serde_json::from_str(include_str!("../../tests/fixtures/gitlab_contract.json")).unwrap()
    }
}
#[async_trait]
impl CollaborationProvider for GitlabFixture {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gitlab
    }
    fn instance(&self) -> ProviderInstance {
        self.instance.clone()
    }
    fn profile(&self, _: &RemoteAccount) -> ProviderProfile {
        ProviderProfile::read_only(InboxSemantics::Todos, true)
    }
    async fn probe(&self, _: &SecretToken) -> Result<VerifiedAccount, ProviderError> {
        let actor = Self::fixture().actor;
        Ok(VerifiedAccount {
            actor_id: actor.id.to_string(),
            login: actor.username,
            display_name: actor.name,
            notifications_supported: false,
        })
    }
    async fn fetch_page(
        &self,
        _: &SecretToken,
        request: FeedRequest,
    ) -> Result<FetchPage, ProviderError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if ProviderInstance::for_account(&request.account)
            .ok()
            .as_ref()
            != Some(&self.instance)
        {
            return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
        }
        // GitLab pagination's numeric next-page header becomes an adapter-owned
        // opaque cursor, rather than a credential-bearing provider URL.
        let page = match request.cursor.as_deref() {
            None => 1,
            Some("gl-page:2") => 2,
            _ => return Err(ProviderError::new(ProviderErrorKind::InvalidResponse)),
        };
        let mut fixture = Self::fixture();
        let mut result = FetchPage {
            repositories: vec![],
            items: vec![],
            endpoint_aliases: vec![],
            notification_subjects: vec![],
            next_cursor: (page == 1).then(|| "gl-page:2".into()),
            etag: None,
            last_modified: None,
            not_modified: false,
            poll_interval_seconds: None,
            cooldown_seconds: None,
        };
        let a = request.account.id.clone();
        match request.kind {
            FeedKind::Repositories => {
                result.repositories = fixture
                    .projects
                    .into_iter()
                    .map(|p| RemoteRepository {
                        id: format!("gitlab:repository:{}", p.id),
                        account_id: a.clone(),
                        provider_id: p.id.to_string(),
                        full_name: p.path_with_namespace,
                        name: p.name,
                        web_url: p.web_url,
                        description: None,
                        default_branch: None,
                        selected: false,
                    })
                    .collect();
            }
            FeedKind::PullRequests | FeedKind::Issues => {
                let repository = request
                    .repository
                    .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidResponse))?;
                if repository.account_id != a {
                    return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                }
                let (items, kind) = if request.kind == FeedKind::PullRequests {
                    (fixture.merge_requests, RemoteItemKind::PullRequest)
                } else {
                    (fixture.issues, RemoteItemKind::Issue)
                };
                for i in items {
                    if repository.provider_id != i.project_id.to_string() {
                        return Err(ProviderError::new(ProviderErrorKind::InvalidResponse));
                    }
                    result.items.push(RemoteItem {
                        id: format!(
                            "gitlab:{}:{}",
                            if kind == RemoteItemKind::PullRequest {
                                "pull"
                            } else {
                                "issue"
                            },
                            i.id
                        ),
                        account_id: a.clone(),
                        repository_id: Some(repository.id.clone()),
                        provider_id: i.id.to_string(),
                        kind: kind.clone(),
                        number: Some(i.iid.to_string()),
                        title: i.title,
                        body: i.description,
                        body_omitted: false,
                        author: None,
                        web_url: Some(i.web_url),
                        state: if i.state == "opened" {
                            "open".into()
                        } else {
                            i.state
                        },
                        updated_at: i.updated_at,
                        head_oid: None,
                        is_draft: if kind == RemoteItemKind::PullRequest {
                            i.draft
                        } else {
                            None
                        },
                        reason: None,
                        unread: None,
                    });
                }
            }
            FeedKind::Notifications => {
                for todo in fixture.todos.drain(..) {
                    result.items.push(RemoteItem {
                        id: format!("gitlab:todo:{}", todo.id),
                        account_id: a.clone(),
                        repository_id: None,
                        provider_id: todo.id.to_string(),
                        kind: RemoteItemKind::Notification,
                        number: None,
                        title: todo.target.title,
                        body: None,
                        body_omitted: true,
                        author: None,
                        web_url: None,
                        state: todo.state,
                        updated_at: todo.created_at,
                        head_oid: None,
                        is_draft: None,
                        reason: Some(format!("{}:{}", todo.action_name, todo.target_type)),
                        unread: None,
                    });
                }
            }
        }
        Ok(result)
    }
}

fn account(provider: ProviderKind, host: &str) -> RemoteAccount {
    RemoteAccount {
        id: format!("{:?}-{host}", provider),
        provider,
        host: host.into(),
        actor_id: "9007199254740993".into(),
        login: "fixture".into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}

fn github_fixture() -> (
    Arc<dyn CollaborationProvider>,
    std::thread::JoinHandle<Vec<String>>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let server_base = base.clone();
    let handle = std::thread::spawn(move || {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/github_contract.json"))
                .unwrap();
        let mut requests = vec![];
        for key in ["actor", "repositories", "pulls", "pulls", "notifications"] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut request = vec![];
            let mut chunk = [0; 2048];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let read = socket.read(&mut chunk).unwrap();
                assert!(read > 0 && request.len() + read < 8192);
                request.extend_from_slice(&chunk[..read]);
            }
            let request = String::from_utf8(request).unwrap();
            let continuation = request.lines().next().unwrap().contains("page=2");
            let body = serde_json::to_string(&fixture[key]).unwrap();
            let next = if key == "pulls" && !continuation {
                format!("Link: <{server_base}repos/owner/project/pulls?page=2>; rel=\"next\"\r\n")
            } else {
                String::new()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-OAuth-Scopes: notifications\r\n{next}Connection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
            requests.push(request);
        }
        requests
    });
    (
        Arc::new(github::GithubProvider::for_test_base(
            reqwest::Url::parse(&base).unwrap(),
        )),
        handle,
    )
}

#[tokio::test]
async fn github_and_gitlab_share_the_contract_without_sharing_identity_or_inbox_semantics() {
    let (github, server) = github_fixture();
    let gl_instance =
        ProviderInstance::new(ProviderKind::Gitlab, "https://git.example:8443/gitlab/").unwrap();
    let gitlab: Arc<dyn CollaborationProvider> = Arc::new(GitlabFixture::new(gl_instance.clone()));
    let mut registry = ProviderRegistry::default();
    registry.register(github).unwrap();
    registry.register(gitlab).unwrap();
    let token = SecretToken::new("ghp_fixture".into()).unwrap();
    for a in [
        account(ProviderKind::Github, "github.com"),
        account(ProviderKind::Gitlab, "git.example:8443/gitlab"),
    ] {
        let instance = ProviderInstance::for_account(&a).unwrap();
        let adapter = registry.adapter(&instance).unwrap();
        assert_eq!(
            adapter.probe(&token).await.unwrap().actor_id,
            "9007199254740993"
        );
        let request = FeedRequest {
            account: a.clone(),
            kind: FeedKind::Repositories,
            repository: None,
            cursor: None,
            etag: None,
            last_modified: None,
        };
        let repositories = adapter
            .fetch_page(&token, request.clone())
            .await
            .unwrap()
            .repositories;
        let repository = repositories[0].clone();
        assert_eq!(repository.provider_id, "9007199254740993");
        if a.provider == ProviderKind::Gitlab {
            assert_eq!(repository.full_name, "team/platform/project");
        }
        let pulls = FeedRequest {
            kind: FeedKind::PullRequests,
            repository: Some(repository),
            ..request.clone()
        };
        let first = adapter.fetch_page(&token, pulls.clone()).await.unwrap();
        assert_eq!(first.items[0].provider_id, "9007199254740994");
        assert_eq!(first.items[0].number.as_deref(), Some("7"));
        assert_eq!(first.items[0].kind, RemoteItemKind::PullRequest);
        let next = first.next_cursor.unwrap();
        let second = adapter
            .fetch_page(
                &token,
                FeedRequest {
                    cursor: Some(next),
                    ..pulls
                },
            )
            .await
            .unwrap();
        assert!(second.next_cursor.is_none());
        let inbox = adapter
            .fetch_page(
                &token,
                FeedRequest {
                    kind: FeedKind::Notifications,
                    ..request
                },
            )
            .await
            .unwrap();
        let profile = adapter.profile(&a);
        match profile.inbox_semantics {
            InboxSemantics::NativeNotifications => assert_eq!(inbox.items[0].unread, Some(true)),
            InboxSemantics::Todos => {
                assert_eq!(inbox.items[0].unread, None);
                assert_eq!(inbox.items[0].state, "pending");
            }
            InboxSemantics::None => panic!("fixture inbox must declare semantics"),
        }
    }
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests[3].starts_with("GET /repos/owner/project/pulls?page=2 "));
    assert!(
        registry
            .adapter(
                &ProviderInstance::new(ProviderKind::Gitlab, "https://git.example:8443/other/")
                    .unwrap()
            )
            .is_err()
    );
    assert!(
        registry
            .adapter(
                &ProviderInstance::new(ProviderKind::Github, "https://git.example:8443/gitlab/")
                    .unwrap()
            )
            .is_err()
    );
}

#[tokio::test]
async fn divergent_fixture_errors_and_bad_continuations_use_typed_safe_outcomes() {
    let instance = ProviderInstance::public(ProviderKind::Gitlab);
    let a = account(ProviderKind::Gitlab, "gitlab.com");
    let request = FeedRequest {
        account: a,
        kind: FeedKind::Repositories,
        repository: None,
        cursor: None,
        etag: None,
        last_modified: None,
    };
    let token = SecretToken::new("fixture".into()).unwrap();
    for (kind, code) in [
        (ProviderErrorKind::Authentication, ErrorCode::AuthRequired),
        (ProviderErrorKind::Permission, ErrorCode::PermissionDenied),
        (ProviderErrorKind::RateLimited, ErrorCode::RateLimited),
        (ProviderErrorKind::Unavailable, ErrorCode::Provider),
        (ProviderErrorKind::Unsupported, ErrorCode::Unsupported),
    ] {
        let provider = GitlabFixture {
            instance: instance.clone(),
            error: Some(ProviderError {
                kind,
                retry_after_seconds: (kind == ProviderErrorKind::RateLimited).then_some(90),
                account_cooldown_seconds: None,
            }),
        };
        let error = provider
            .fetch_page(&token, request.clone())
            .await
            .unwrap_err();
        let safe: CollaborationError = error.into();
        assert_eq!(safe.code, code);
        assert!(!safe.message.contains("fixture"));
        if kind == ProviderErrorKind::RateLimited {
            assert_eq!(safe.retry_after_seconds, Some(90));
        }
    }
    let provider = GitlabFixture::new(instance);
    assert_eq!(
        provider
            .fetch_page(
                &token,
                FeedRequest {
                    cursor: Some("https://another.example/page".into()),
                    ..request
                }
            )
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
}

#[test]
fn instance_normalization_and_capability_states_are_explicit_and_fail_closed() {
    let a = ProviderInstance::new(ProviderKind::Gitlab, "https://GIT.EXAMPLE:443/gitlab").unwrap();
    assert_eq!(
        a,
        ProviderInstance::new(ProviderKind::Gitlab, "https://git.example/gitlab/").unwrap()
    );
    for url in [
        "http://git.example/",
        "https://user:secret@git.example/",
        "https://git.example/?token=secret",
        "https://git.example/#fragment",
        "https://git.example/a/../b/",
        "https://git.example/%2fother/",
        "https://git.example/a/%2e%2e/b/",
        "https://git.example//other/",
        " https://git.example/",
        "https://git.example\\other/",
    ] {
        assert!(
            ProviderInstance::new(ProviderKind::Gitlab, url).is_err(),
            "{url}"
        );
    }
    let missing_scope = ProviderProfile::read_only(InboxSemantics::NativeNotifications, false)
        .facet(ResourceFacet::Inbox);
    assert_eq!(
        (missing_scope.state, missing_scope.reason),
        (
            CapabilityState::Unavailable,
            Some(CapabilityReason::MissingScope)
        )
    );
    let unsupported =
        ProviderProfile::read_only(InboxSemantics::None, false).facet(ResourceFacet::Inbox);
    assert_eq!(
        (unsupported.state, unsupported.reason),
        (
            CapabilityState::Unsupported,
            Some(CapabilityReason::ProviderSemantics)
        )
    );
    assert_eq!(
        ProviderProfile::unavailable(CapabilityReason::AdapterUnavailable)
            .facet(ResourceFacet::Inbox)
            .state,
        CapabilityState::Unavailable
    );
}
