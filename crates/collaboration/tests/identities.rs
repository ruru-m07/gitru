use collaboration::*;

fn account(id: &str, provider: ProviderKind, host: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider,
        host: host.into(),
        actor_id: id.into(),
        login: id.into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: true,
    }
}
fn repo(account: &str, id: &str, native: &str, path: &str, base: &str) -> RemoteRepository {
    RemoteRepository {
        id: id.into(),
        account_id: account.into(),
        provider_id: native.into(),
        full_name: path.into(),
        name: path.rsplit('/').next().unwrap().into(),
        web_url: format!("{base}{path}"),
        description: None,
        default_branch: None,
        selected: true,
    }
}
fn item(
    account: &str,
    repo: &str,
    id: &str,
    native: &str,
    kind: RemoteItemKind,
    number: &str,
) -> RemoteItem {
    RemoteItem {
        id: id.into(),
        account_id: account.into(),
        repository_id: Some(repo.into()),
        provider_id: native.into(),
        kind,
        number: Some(number.into()),
        title: "saved title".into(),
        body: Some("private provider body never returned by resolution".into()),
        body_omitted: false,
        author: None,
        web_url: None,
        state: "open".into(),
        updated_at: "2026-10-03T12:00:00Z".into(),
        head_oid: None,
        is_draft: None,
        reason: None,
        unread: None,
    }
}
async fn page(
    store: &Store,
    account: &str,
    scope: &str,
    repositories: Vec<RemoteRepository>,
    items: Vec<RemoteItem>,
    endpoint_aliases: Vec<EndpointAlias>,
) -> Result<String, CollaborationError> {
    let epoch = store.account(account).await?.authorization_epoch;
    let run_id = store.begin_sync(account, &epoch, scope).await?;
    store
        .apply_page(PageCommit {
            account_id: account.into(),
            authorization_epoch: epoch,
            scope: scope.into(),
            run_id,
            repositories,
            items,
            endpoint_aliases,
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-03T12:00:00Z".into(),
        })
        .await
}
async fn resolve(
    store: &Store,
    account: &str,
    kind: ResourceKind,
    locator_kind: LocatorKind,
    value: &str,
    path: Option<&str>,
) -> ResourceResolution {
    let instance_id = store.provider_instance(account).await.unwrap().id;
    store
        .resolve_resource(
            account,
            ResourceLocator {
                instance_id,
                kind,
                locator_kind,
                value: value.into(),
                repository_path: path.map(str::to_owned),
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn issue_side_pr_aliases_converge_in_both_orders_without_relabeling_issues_or_drafts() {
    for issue_first in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("db")).await.unwrap();
        store
            .upsert_account(account("a", ProviderKind::Github, "github.com"))
            .await
            .unwrap();
        page(
            &store,
            "a",
            "repositories",
            vec![repo(
                "a",
                "repo",
                "42",
                "owner/project",
                "https://github.com/",
            )],
            vec![],
            vec![],
        )
        .await
        .unwrap();
        let alias = EndpointAlias {
            kind: ResourceKind::PullRequest,
            repository_provider_id: "42".into(),
            number: "67".into(),
            native_identity: "issue:1000".into(),
            web_url: Some("https://github.com/owner/project/pull/67".into()),
        };
        if issue_first {
            page(
                &store,
                "a",
                "repo:repo:issue",
                vec![],
                vec![item(
                    "a",
                    "repo",
                    "true-issue",
                    "3000",
                    RemoteItemKind::Issue,
                    "67",
                )],
                vec![alias.clone()],
            )
            .await
            .unwrap();
            assert_eq!(
                resolve(
                    &store,
                    "a",
                    ResourceKind::PullRequest,
                    LocatorKind::Native,
                    "issue:1000",
                    None
                )
                .await
                .state,
                ResolutionState::Unresolved
            );
        }
        page(
            &store,
            "a",
            "repo:repo:pull_request",
            vec![],
            vec![item(
                "a",
                "repo",
                "canonical-pull",
                "2000",
                RemoteItemKind::PullRequest,
                "67",
            )],
            vec![],
        )
        .await
        .unwrap();
        let draft = store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "canonical-pull".into(),
                body: "keep my text".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        if !issue_first {
            page(
                &store,
                "a",
                "repo:repo:issue",
                vec![],
                vec![item(
                    "a",
                    "repo",
                    "true-issue",
                    "3000",
                    RemoteItemKind::Issue,
                    "67",
                )],
                vec![alias],
            )
            .await
            .unwrap();
        }
        for (locator_kind, value, path) in [
            (LocatorKind::Native, "issue:1000", None),
            (LocatorKind::Native, "pull_request:2000", None),
            (
                LocatorKind::WebUrl,
                "https://github.com/owner/project/pull/67",
                None,
            ),
            (LocatorKind::RepositoryNumber, "67", Some("owner/project")),
        ] {
            let resolved = resolve(
                &store,
                "a",
                ResourceKind::PullRequest,
                locator_kind,
                value,
                path,
            )
            .await;
            assert_eq!(resolved.state, ResolutionState::Resolved);
            assert_eq!(resolved.resource.unwrap().id, "canonical-pull");
            assert!(
                !serde_json::to_string(&resolved.candidates)
                    .unwrap()
                    .contains("private provider body")
            );
        }
        assert_eq!(
            resolve(
                &store,
                "a",
                ResourceKind::Issue,
                LocatorKind::RepositoryNumber,
                "67",
                Some("owner/project")
            )
            .await
            .resource
            .unwrap()
            .id,
            "true-issue"
        );
        assert_eq!(
            store.draft("a", "canonical-pull").await.unwrap().unwrap(),
            draft
        );
        assert_eq!(
            store
                .save_draft(LocalDraft {
                    generation: "0".into(),
                    ..draft.clone()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
        assert_eq!(
            store
                .item("a", "true-issue")
                .await
                .unwrap()
                .item
                .unwrap()
                .kind,
            RemoteItemKind::Issue
        );
    }
}

#[tokio::test]
async fn rename_transfer_restart_and_path_reuse_preserve_identity_and_report_ambiguity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    {
        let store = Store::open(&path).await.unwrap();
        store
            .upsert_account(account("a", ProviderKind::Github, "github.com"))
            .await
            .unwrap();
        page(
            &store,
            "a",
            "repositories",
            vec![repo(
                "a",
                "stable-repo",
                "42",
                "old/project",
                "https://github.com/",
            )],
            vec![],
            vec![],
        )
        .await
        .unwrap();
        page(
            &store,
            "a",
            "repo:stable-repo:pull_request",
            vec![],
            vec![RemoteItem {
                web_url: Some("https://github.com/old/project/pull/67".into()),
                ..item(
                    "a",
                    "stable-repo",
                    "stable-pull",
                    "999",
                    RemoteItemKind::PullRequest,
                    "67",
                )
            }],
            vec![],
        )
        .await
        .unwrap();
        store
            .save_draft(LocalDraft {
                account_id: "a".into(),
                subject_id: "stable-pull".into(),
                body: "survive rename".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        page(
            &store,
            "a",
            "repositories",
            vec![repo(
                "a",
                "stable-repo",
                "42",
                "new/project",
                "https://github.com/",
            )],
            vec![],
            vec![],
        )
        .await
        .unwrap();
        for name in ["old/project", "new/project"] {
            assert_eq!(
                resolve(
                    &store,
                    "a",
                    ResourceKind::Repository,
                    LocatorKind::RepositoryPath,
                    name,
                    None
                )
                .await
                .resource
                .unwrap()
                .id,
                "stable-repo"
            );
            assert_eq!(
                resolve(
                    &store,
                    "a",
                    ResourceKind::PullRequest,
                    LocatorKind::RepositoryNumber,
                    "67",
                    Some(name)
                )
                .await
                .resource
                .unwrap()
                .id,
                "stable-pull"
            );
        }
        // Resource transfer changes repository and display number, not native ID.
        page(
            &store,
            "a",
            "repositories",
            vec![
                repo(
                    "a",
                    "stable-repo",
                    "42",
                    "new/project",
                    "https://github.com/",
                ),
                repo("a", "target", "43", "target/project", "https://github.com/"),
            ],
            vec![],
            vec![],
        )
        .await
        .unwrap();
        page(
            &store,
            "a",
            "repo:target:pull_request",
            vec![],
            vec![item(
                "a",
                "target",
                "stable-pull",
                "999",
                RemoteItemKind::PullRequest,
                "5",
            )],
            vec![],
        )
        .await
        .unwrap();
        assert_eq!(
            resolve(
                &store,
                "a",
                ResourceKind::PullRequest,
                LocatorKind::RepositoryNumber,
                "5",
                Some("target/project")
            )
            .await
            .resource
            .unwrap()
            .id,
            "stable-pull"
        );
        page(
            &store,
            "a",
            "repositories",
            vec![
                repo(
                    "a",
                    "stable-repo",
                    "42",
                    "new/project",
                    "https://github.com/",
                ),
                repo(
                    "a",
                    "replacement",
                    "44",
                    "old/project",
                    "https://github.com/",
                ),
                repo("a", "target", "43", "target/project", "https://github.com/"),
            ],
            vec![],
            vec![],
        )
        .await
        .unwrap();
        let ambiguous = resolve(
            &store,
            "a",
            ResourceKind::Repository,
            LocatorKind::RepositoryPath,
            "old/project",
            None,
        )
        .await;
        assert_eq!(ambiguous.state, ResolutionState::Ambiguous);
        assert_eq!(ambiguous.candidates.len(), 2);
        assert!(ambiguous.resource.is_none());
        assert_eq!(
            resolve(
                &store,
                "a",
                ResourceKind::PullRequest,
                LocatorKind::WebUrl,
                "https://GITHUB.com:443/old/project/pull/67/",
                None
            )
            .await
            .state,
            ResolutionState::Ambiguous
        );
        assert_eq!(
            resolve(
                &store,
                "a",
                ResourceKind::PullRequest,
                LocatorKind::RepositoryNumber,
                "67",
                Some("old/project")
            )
            .await
            .state,
            ResolutionState::Ambiguous
        );
        store.close().await.unwrap();
    }
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        resolve(
            &store,
            "a",
            ResourceKind::Repository,
            LocatorKind::RepositoryPath,
            "old/project",
            None
        )
        .await
        .state,
        ResolutionState::Ambiguous
    );
    let draft = store.draft("a", "stable-pull").await.unwrap().unwrap();
    assert_eq!(
        (draft.body.as_str(), draft.generation.as_str()),
        ("survive rename", "1")
    );
}

#[tokio::test]
async fn accounts_providers_hosts_ports_and_base_paths_never_share_resolution_or_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    for (id, provider, host, base) in [
        (
            "a",
            ProviderKind::Github,
            "github.com",
            "https://github.com/",
        ),
        (
            "b",
            ProviderKind::Github,
            "github.com",
            "https://github.com/",
        ),
        (
            "c",
            ProviderKind::Gitlab,
            "github.com",
            "https://github.com/",
        ),
        (
            "d",
            ProviderKind::Gitlab,
            "git.example:8443/gitlab",
            "https://git.example:8443/gitlab/",
        ),
        (
            "e",
            ProviderKind::Gitlab,
            "git.example:8443/other",
            "https://git.example:8443/other/",
        ),
    ] {
        store
            .upsert_account(account(id, provider, host))
            .await
            .unwrap();
        page(
            &store,
            id,
            "repositories",
            vec![repo(
                id,
                "same-id",
                "9007199254740993",
                "group/subgroup/project",
                base,
            )],
            vec![],
            vec![],
        )
        .await
        .unwrap();
        let own = resolve(
            &store,
            id,
            ResourceKind::Repository,
            LocatorKind::RepositoryPath,
            "group/subgroup/project",
            None,
        )
        .await;
        assert_eq!(own.resource.unwrap().account_id, id);
    }
    let d = store.provider_instance("d").await.unwrap();
    let e = store.provider_instance("e").await.unwrap();
    assert_ne!(d.id, e.id);
    assert_eq!(
        store
            .resolve_resource(
                "e",
                ResourceLocator {
                    instance_id: d.id,
                    kind: ResourceKind::Repository,
                    locator_kind: LocatorKind::Canonical,
                    value: "same-id".into(),
                    repository_path: None
                }
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        resolve(
            &store,
            "b",
            ResourceKind::PullRequest,
            LocatorKind::Native,
            "pull_request:9007199254740993",
            None
        )
        .await
        .state,
        ResolutionState::Unresolved
    );
}

#[tokio::test]
async fn retained_aliases_do_not_resurrect_denied_or_disconnected_content() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    store
        .upsert_account(account("a", ProviderKind::Github, "github.com"))
        .await
        .unwrap();
    page(
        &store,
        "a",
        "repositories",
        vec![repo(
            "a",
            "repo",
            "42",
            "owner/project",
            "https://github.com/",
        )],
        vec![],
        vec![],
    )
    .await
    .unwrap();
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![item(
            "a",
            "repo",
            "pull",
            "123",
            RemoteItemKind::PullRequest,
            "67",
        )],
        vec![],
    )
    .await
    .unwrap();
    let draft = store
        .save_draft(LocalDraft {
            account_id: "a".into(),
            subject_id: "pull".into(),
            body: "keep".into(),
            generation: "0".into(),
        })
        .await
        .unwrap();
    store
        .set_sync_status(
            "a",
            "1",
            "repo:repo:pull_request",
            SyncStatus {
                state: SyncState::Error,
                error: Some(CollaborationError::new(
                    ErrorCode::PermissionDenied,
                    "denied",
                )),
                ..SyncStatus::default()
            },
        )
        .await
        .unwrap();
    let denied = resolve(
        &store,
        "a",
        ResourceKind::PullRequest,
        LocatorKind::Canonical,
        "pull",
        None,
    )
    .await;
    assert_eq!(denied.state, ResolutionState::Unavailable);
    assert!(denied.resource.is_none());
    assert!(denied.candidates.is_empty());
    store.disconnect("a").await.unwrap();
    assert_eq!(
        resolve(
            &store,
            "a",
            ResourceKind::PullRequest,
            LocatorKind::Canonical,
            "pull",
            None
        )
        .await
        .state,
        ResolutionState::Unavailable
    );
    assert_eq!(store.draft("a", "pull").await.unwrap().unwrap(), draft);
    let mut replacement = account("a", ProviderKind::Github, "github.com");
    replacement.authorization_epoch = "3".into();
    store.upsert_account(replacement).await.unwrap();
    assert_eq!(
        resolve(
            &store,
            "a",
            ResourceKind::PullRequest,
            LocatorKind::Canonical,
            "pull",
            None
        )
        .await
        .state,
        ResolutionState::Unavailable
    );
}

#[tokio::test]
async fn canonical_reuse_and_foreign_endpoint_observations_roll_back_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    store
        .upsert_account(account("a", ProviderKind::Github, "github.com"))
        .await
        .unwrap();
    page(
        &store,
        "a",
        "repositories",
        vec![repo(
            "a",
            "repo",
            "42",
            "owner/project",
            "https://github.com/",
        )],
        vec![],
        vec![],
    )
    .await
    .unwrap();
    page(
        &store,
        "a",
        "repo:repo:pull_request",
        vec![],
        vec![item(
            "a",
            "repo",
            "pull",
            "123",
            RemoteItemKind::PullRequest,
            "67",
        )],
        vec![],
    )
    .await
    .unwrap();
    let before = store.item("a", "pull").await.unwrap().item.unwrap();
    assert_eq!(
        page(
            &store,
            "a",
            "repo:repo:pull_request",
            vec![],
            vec![item(
                "a",
                "repo",
                "pull",
                "different",
                RemoteItemKind::PullRequest,
                "67"
            )],
            vec![]
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.item("a", "pull").await.unwrap().item.unwrap(), before);
    assert_eq!(
        page(
            &store,
            "a",
            "repo:repo:issue",
            vec![],
            vec![item(
                "a",
                "repo",
                "should-roll-back",
                "567",
                RemoteItemKind::Issue,
                "9"
            )],
            vec![EndpointAlias {
                kind: ResourceKind::PullRequest,
                repository_provider_id: "wrong".into(),
                number: "67".into(),
                native_identity: "issue:123".into(),
                web_url: None
            }]
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::InvalidInput
    );
    assert!(
        store
            .item("a", "should-roll-back")
            .await
            .unwrap()
            .item
            .is_none()
    );
}

#[tokio::test]
async fn todo_inbox_states_do_not_invent_native_notification_unread_flags() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("db")).await.unwrap();
    let mut a = account("a", ProviderKind::Gitlab, "gitlab.com");
    a.notifications_supported = false;
    store.upsert_account(a).await.unwrap();
    let mut todo = item(
        "a",
        "unused",
        "todo",
        "88",
        RemoteItemKind::Notification,
        "7",
    );
    todo.repository_id = None;
    todo.number = None;
    todo.body = None;
    todo.unread = None;
    todo.state = "pending".into();
    page(&store, "a", "notifications", vec![], vec![todo], vec![])
        .await
        .unwrap();
    for (state, expected) in [
        (None, 1),
        (Some("pending"), 1),
        (Some("read"), 0),
        (Some("unread"), 0),
    ] {
        let rows = store
            .query_items(ItemQuery {
                account_id: "a".into(),
                kind: RemoteItemKind::Notification,
                repository_id: None,
                state: state.map(str::to_owned),
                search: None,
                cursor: None,
                limit: 10,
            })
            .await
            .unwrap()
            .items;
        assert_eq!(rows.len(), expected);
        assert!(rows.iter().all(|i| i.unread.is_none()));
    }
}
