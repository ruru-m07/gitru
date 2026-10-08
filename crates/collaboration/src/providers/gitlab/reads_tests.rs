//! Exercise resource reads through actual HTTP, not a parallel normalizer API.
use super::{
    tests::{project, response, server},
    transport::{ISSUE_QUERY, MERGE_REQUEST_QUERY},
    *,
};
use crate::resource_metadata::MetadataField;
use serde_json::{Value, json};

const PROJECT: u64 = 9007199254740993;
const SOURCE_PROJECT: u64 = 9007199254740997;
const NATIVE: u64 = 9007199254741993;
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "2222222222222222222222222222222222222222";
const MERGE_BASE: &str = "3333333333333333333333333333333333333333";

fn account() -> RemoteAccount {
    RemoteAccount {
        id: "gitlab-account".into(),
        provider: ProviderKind::Gitlab,
        host: "gitlab.com".into(),
        actor_id: "9007199254740993".into(),
        login: "actor".into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}
fn token() -> SecretToken {
    SecretToken::new("synthetic_gitlab_token".into()).unwrap()
}
pub(crate) fn selected_repository(project_id: u64, path: &str) -> RemoteRepository {
    let value = project(project_id, path);
    let mut repository = serde_json::from_value::<Project>(value)
        .unwrap()
        .remote(&account().id)
        .unwrap();
    repository.selected = true;
    repository
}
pub(crate) fn merge_request(native: u64, project: u64, iid: u64) -> Value {
    json!({"id":native,"iid":iid,"project_id":project,"target_project_id":project,"source_project_id":SOURCE_PROJECT,"title":"MR Δ 🚀","description":"independent Body","state":"opened","created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-04T00:00:00Z","web_url":format!("https://gitlab.com/org/subgroup/project/-/merge_requests/{iid}"),"author":{"id":9007199254741111_u64,"username":"author","web_url":"https://gitlab.com/author"},"labels":["bug",{"id":9007199254742222_u64,"name":"native","color":"#aabbcc"}],"assignees":[],"milestone":{"id":9007199254743333_u64,"iid":8,"title":"Next","state":"active","web_url":"https://gitlab.com/org/subgroup/project/-/milestones/8"},"draft":false,"merged_at":null,"source_branch":"feature","target_branch":"main","sha":HEAD,"diff_refs":{"head_sha":HEAD,"start_sha":BASE,"base_sha":MERGE_BASE}})
}
pub(crate) fn issue(native: u64, project: u64, iid: u64) -> Value {
    let mut value = merge_request(native, project, iid);
    let object = value.as_object_mut().unwrap();
    for key in [
        "target_project_id",
        "source_project_id",
        "draft",
        "merged_at",
        "source_branch",
        "target_branch",
        "sha",
        "diff_refs",
    ] {
        object.remove(key);
    }
    object.insert("issue_type".into(), json!("issue"));
    object.insert(
        "web_url".into(),
        json!(format!(
            "https://gitlab.com/org/subgroup/project/-/issues/{iid}"
        )),
    );
    value
}
fn feed(kind: FeedKind) -> FeedRequest {
    FeedRequest {
        account: account(),
        kind,
        repository: Some(selected_repository(PROJECT, "org/subgroup/project")),
        cursor: None,
        etag: Some("ignored-list-etag".into()),
        last_modified: Some("ignored-modified".into()),
    }
}
fn subject(kind: RemoteItemKind) -> RemoteItem {
    RemoteItem {
        native_inbox: None,
        id: format!(
            "gitlab:{}:{NATIVE}",
            if kind == RemoteItemKind::PullRequest {
                "pull"
            } else {
                "issue"
            }
        ),
        account_id: account().id,
        repository_id: Some(format!("gitlab:repository:{PROJECT}")),
        provider_id: NATIVE.to_string(),
        kind,
        number: Some("67".into()),
        title: "old summary".into(),
        body: Some("old summary description".into()),
        body_omitted: false,
        author: None,
        web_url: None,
        state: "open".into(),
        updated_at: "2026-09-01T00:00:00Z".into(),
        head_oid: Some(HEAD.into()),
        is_draft: None,
        reason: None,
        unread: None,
    }
}
fn detail(kind: RemoteItemKind) -> DetailRequest {
    DetailRequest {
        account: account(),
        repository: selected_repository(PROJECT, "org/subgroup/project"),
        subject: subject(kind),
        facet: DetailFacet::Body,
        cursor: None,
        etag: Some("ignored-detail-etag".into()),
        source: None,
    }
}
fn body(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}
fn values(values: &[Value]) -> String {
    serde_json::to_string(values).unwrap()
}
fn observed(page: &DetailPage, field: MetadataField) -> DetailValueState {
    page.metadata
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .find(|f| f.field == field)
        .unwrap()
        .state
}

#[tokio::test]
async fn merge_request_full_offset_feed_keeps_native_ids_states_and_numeric_routes() {
    let states = ["opened", "closed", "merged", "locked"];
    let rows: Vec<_> = states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let mut value = merge_request(NATIVE + index as u64, PROJECT, 67 + index as u64);
            value["state"] = json!(state);
            value["draft"] = json!(index == 0);
            value
        })
        .collect();
    let (provider, task) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}projects/{PROJECT}/merge_requests?{MERGE_REQUEST_QUERY}&page=2>; rel=\"next\"\r\n"
                ),
                &values(&rows),
            ),
            response(200, "", &values(&[merge_request(NATIVE + 4, PROJECT, 71)])),
        ]
    });
    let first = provider
        .fetch_page(&token(), feed(FeedKind::PullRequests))
        .await
        .unwrap();
    assert_eq!(first.items.len(), 4);
    assert_eq!(first.items[0].provider_id, NATIVE.to_string());
    assert_eq!(first.items[0].number.as_deref(), Some("67"));
    assert_eq!(first.items[0].is_draft, Some(true));
    assert_eq!(
        first
            .items
            .iter()
            .map(|i| i.state.as_str())
            .collect::<Vec<_>>(),
        vec!["open", "closed", "merged", "locked"]
    );
    assert!(first.etag.is_none() && first.last_modified.is_none() && !first.not_modified);
    let mut continuation = feed(FeedKind::PullRequests);
    continuation.cursor = first.next_cursor;
    let second = provider.fetch_page(&token(), continuation).await.unwrap();
    assert!(second.next_cursor.is_none());
    let calls = task.join().unwrap();
    assert!(calls[0].starts_with(&format!(
        "GET /api/v4/projects/{PROJECT}/merge_requests?{MERGE_REQUEST_QUERY}&page=1 HTTP/1.1"
    )));
    assert!(calls[1].contains("&page=2 HTTP/1.1"));
    for call in calls {
        assert!(!call.to_ascii_lowercase().contains("if-none-match"));
        assert!(!call.to_ascii_lowercase().contains("if-modified-since"));
        assert!(
            call.to_ascii_lowercase()
                .contains("private-token: synthetic_gitlab_token")
        );
    }
}

#[tokio::test]
async fn issue_opaque_cursor_is_authority_bound_and_orders_global_ids() {
    let (provider, task) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}projects/{PROJECT}/issues?{ISSUE_QUERY}&cursor=eyJpZCI6MX0%3D>; rel=\"next\"\r\n"
                ),
                &values(&[issue(NATIVE, PROJECT, 67)]),
            ),
            response(200, "", &values(&[issue(NATIVE + 1, PROJECT, 68)])),
        ]
    });
    let first = provider
        .fetch_page(&token(), feed(FeedKind::Issues))
        .await
        .unwrap();
    let cursor = first.next_cursor.unwrap();
    assert!(cursor.len() < 4096);
    for change in 0..5 {
        let mut input = feed(FeedKind::Issues);
        input.cursor = Some(cursor.clone());
        match change {
            0 => input.account.id = "foreign".into(),
            1 => input.account.authorization_epoch = "2".into(),
            2 => input.kind = FeedKind::PullRequests,
            3 => input.repository = Some(selected_repository(PROJECT + 1, "else/project")),
            _ => input.cursor = Some("{\"version\":99}".into()),
        }
        assert_eq!(
            provider.fetch_page(&token(), input).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    let mut input = feed(FeedKind::Issues);
    input.cursor = Some(cursor);
    let second = provider.fetch_page(&token(), input).await.unwrap();
    assert_eq!(second.items[0].provider_id, (NATIVE + 1).to_string());
    assert!(second.next_cursor.is_none());
    let calls = task.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].contains("cursor=eyJpZCI6MX0%3D"));
    assert!(calls[0].contains("issue_type=issue"));
}

#[tokio::test]
async fn repeated_iids_across_projects_and_transferred_display_urls_preserve_identity() {
    let mut transferred = issue(NATIVE, PROJECT, 67);
    transferred["web_url"] = json!("https://gitlab.com/new/namespace/project/-/issues/67");
    let (provider, task) = server(|_| {
        vec![
            response(200, "", &values(&[transferred])),
            response(200, "", &values(&[issue(NATIVE + 1, PROJECT + 1, 67)])),
        ]
    });
    let first = provider
        .fetch_page(&token(), feed(FeedKind::Issues))
        .await
        .unwrap();
    let mut other = feed(FeedKind::Issues);
    other.repository = Some(selected_repository(PROJECT + 1, "other/project"));
    let second = provider.fetch_page(&token(), other).await.unwrap();
    assert_eq!(first.items[0].number, second.items[0].number);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert_ne!(first.items[0].repository_id, second.items[0].repository_id);
    assert!(
        first.items[0]
            .web_url
            .as_ref()
            .unwrap()
            .contains("new/namespace")
    );
    let calls = task.join().unwrap();
    assert!(calls[0].contains(&format!("projects/{PROJECT}/issues")));
    assert!(calls[1].contains(&format!("projects/{}/issues", PROJECT + 1)));
}

#[tokio::test]
async fn hostile_or_skipping_resource_links_retain_quota_and_never_claim_completion() {
    for change in 0..11 {
        let (provider, task) = server(|base| {
            let good =
                format!("{base}projects/{PROJECT}/merge_requests?{MERGE_REQUEST_QUERY}&page=2");
            let next = match change {
                0 => good.replace("page=2", "page=1"),
                1 => good.replace("page=2", "page=3"),
                2 => good.replace(
                    &format!("projects/{PROJECT}"),
                    &format!("projects/{}", PROJECT + 1),
                ),
                3 => good.replace("merge_requests", "issues"),
                4 => format!("{good}&sudo=actor"),
                5 => format!("{good}&private_token=secret"),
                6 => format!("{good}&page=2"),
                7 => good.replace("per_page=50", "per_page=100"),
                8 => good.replace("/projects/", "/%70rojects/"),
                9 => good.replace("/projects/", "/../projects/"),
                _ => good.replace(base, "https://foreign.invalid/api/v4/"),
            };
            vec![response(
                200,
                &format!("RateLimit-Remaining: 0\r\nLink: <{next}>; rel=\"next\"\r\n"),
                &values(&[merge_request(NATIVE, PROJECT, 67)]),
            )]
        });
        let error = provider
            .fetch_page(&token(), feed(FeedKind::PullRequests))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        assert_eq!(task.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn feed_rejects_duplicates_wrong_project_incidents_and_reordered_ids() {
    let base = issue(NATIVE, PROJECT, 67);
    let mut wrong_project = base.clone();
    wrong_project["project_id"] = json!(PROJECT + 1);
    let mut incident = base.clone();
    incident["issue_type"] = json!("incident");
    let mut duplicate_iid = base.clone();
    duplicate_iid["id"] = json!(NATIVE + 1);
    let mut string_id = base.clone();
    string_id["id"] = json!(NATIVE.to_string());
    for rows in [
        vec![base.clone(), base.clone()],
        vec![wrong_project],
        vec![incident],
        vec![base.clone(), duplicate_iid],
        vec![issue(NATIVE + 1, PROJECT, 68), base],
        vec![string_id],
    ] {
        let (provider, task) = server(|_| vec![response(200, "", &values(&rows))]);
        assert_eq!(
            provider
                .fetch_page(&token(), feed(FeedKind::Issues))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        task.join().unwrap();
    }
}

#[tokio::test]
async fn feed_boundaries_reject_repeated_page_empty_continuations_and_backward_clocks() {
    for issue_feed in [true, false] {
        let query = if issue_feed {
            ISSUE_QUERY
        } else {
            MERGE_REQUEST_QUERY
        };
        let segment = if issue_feed {
            "issues"
        } else {
            "merge_requests"
        };
        let continuation = if issue_feed {
            format!("&id_after={NATIVE}")
        } else {
            "&page=2".into()
        };
        let first = if issue_feed {
            issue(NATIVE, PROJECT, 67)
        } else {
            merge_request(NATIVE, PROJECT, 67)
        };
        let mut old = if issue_feed {
            issue(NATIVE, PROJECT, 68)
        } else {
            merge_request(NATIVE + 1, PROJECT, 68)
        };
        old["created_at"] = json!("2026-09-01T00:00:00Z");
        let (provider, task) = server(|base| {
            vec![
                response(
                    200,
                    &format!(
                        "Link: <{base}projects/{PROJECT}/{segment}?{query}{continuation}>; rel=\"next\"\r\n"
                    ),
                    &values(&[first]),
                ),
                response(200, "", &values(&[old])),
            ]
        });
        let kind = if issue_feed {
            FeedKind::Issues
        } else {
            FeedKind::PullRequests
        };
        let first = provider.fetch_page(&token(), feed(kind)).await.unwrap();
        let mut next = feed(kind);
        next.cursor = first.next_cursor;
        assert_eq!(
            provider.fetch_page(&token(), next).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
        task.join().unwrap();
    }
    let (provider, task) = server(|base| {
        vec![response(
            200,
            &format!(
                "Link: <{base}projects/{PROJECT}/issues?{ISSUE_QUERY}&cursor=opaque>; rel=\"next\"\r\n"
            ),
            "[]",
        )]
    });
    assert_eq!(
        provider
            .fetch_page(&token(), feed(FeedKind::Issues))
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    task.join().unwrap();
}

#[tokio::test]
async fn equal_creation_timestamp_across_offset_pages_cannot_replay_native_identity() {
    let rows = [
        merge_request(NATIVE, PROJECT, 67),
        merge_request(NATIVE + 1, PROJECT, 68),
    ];
    let (provider, task) = server(|base| {
        vec![
            response(
                200,
                &format!(
                    "Link: <{base}projects/{PROJECT}/merge_requests?{MERGE_REQUEST_QUERY}&page=2>; rel=\"next\"\r\n"
                ),
                &values(&rows),
            ),
            response(200, "", &values(&[rows[1].clone()])),
        ]
    });
    let first = provider
        .fetch_page(&token(), feed(FeedKind::PullRequests))
        .await
        .unwrap();
    let mut next = feed(FeedKind::PullRequests);
    next.cursor = first.next_cursor;
    assert_eq!(
        provider.fetch_page(&token(), next).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(task.join().unwrap().len(), 2);
}

#[tokio::test]
async fn singleton_body_and_metadata_have_independent_endpoint_authority() {
    let payload = merge_request(NATIVE, PROJECT, 67);
    let (provider, task) = server(|_| vec![response(200, "", &body(&payload))]);
    let page = provider
        .fetch_detail(&token(), detail(RemoteItemKind::PullRequest))
        .await
        .unwrap();
    assert_eq!(page.body.state, DetailValueState::Known);
    assert_eq!(page.body.text.as_deref(), Some("independent Body"));
    assert_eq!(page.source.source, resource_details::PULL_SOURCE);
    assert_eq!(
        page.source.provider_updated_at.as_deref(),
        Some("2026-10-04T00:00:00Z")
    );
    assert!(
        page.etag.is_none()
            && !page.not_modified
            && page.next_cursor.is_none()
            && page.entries.is_empty()
    );
    let metadata = page.metadata.as_ref().unwrap();
    assert_eq!(metadata.values.state.as_deref(), Some("open"));
    assert_eq!(metadata.values.title.as_deref(), Some("MR Δ 🚀"));
    assert_eq!(
        metadata.values.author.as_ref().unwrap().provider_id,
        "9007199254741111"
    );
    assert_eq!(metadata.values.labels[0].provider_id, None);
    assert_eq!(
        metadata.values.labels[1].provider_id.as_deref(),
        Some("9007199254742222")
    );
    assert_eq!(
        metadata.values.milestone.as_ref().unwrap().provider_id,
        "9007199254743333"
    );
    assert_eq!(metadata.values.head.as_ref().unwrap().oid, HEAD);
    assert_eq!(metadata.values.base.as_ref().unwrap().oid, BASE);
    assert_ne!(metadata.values.base.as_ref().unwrap().oid, MERGE_BASE);
    assert_eq!(metadata.values.merge_base_oid.as_deref(), Some(MERGE_BASE));
    assert_eq!(
        observed(&page, MetadataField::MergeBase),
        DetailValueState::Known
    );
    let head_repository = metadata
        .values
        .head
        .as_ref()
        .unwrap()
        .repository
        .as_ref()
        .unwrap();
    assert_eq!(head_repository.provider_id, SOURCE_PROJECT.to_string());
    assert_eq!(head_repository.full_name, SOURCE_PROJECT.to_string());
    assert!(head_repository.web_url.is_none());
    let base_repository = metadata
        .values
        .base
        .as_ref()
        .unwrap()
        .repository
        .as_ref()
        .unwrap();
    assert_eq!(base_repository.provider_id, PROJECT.to_string());
    assert_eq!(base_repository.full_name, "org/subgroup/project");
    assert!(base_repository.web_url.is_none());
    assert_eq!(
        observed(&page, MetadataField::MergedAt),
        DetailValueState::Known
    );
    let calls = task.join().unwrap();
    assert!(calls[0].starts_with(&format!(
        "GET /api/v4/projects/{PROJECT}/merge_requests/67 HTTP/1.1"
    )));
    assert!(!calls[0].to_ascii_lowercase().contains("if-none-match"));
}

#[tokio::test]
async fn singleton_repository_identity_uses_exact_ids_without_inventing_omitted_values() {
    for change in 0..4 {
        let mut payload = merge_request(NATIVE, PROJECT, 67);
        match change {
            0 => {
                payload.as_object_mut().unwrap().remove("target_project_id");
            }
            1 => {
                payload.as_object_mut().unwrap().remove("source_project_id");
            }
            2 => payload["source_project_id"] = Value::Null,
            _ => payload["source_project_id"] = json!(PROJECT),
        }
        let (provider, task) = server(|_| vec![response(200, "", &body(&payload))]);
        let page = provider
            .fetch_detail(&token(), detail(RemoteItemKind::PullRequest))
            .await
            .unwrap();
        let metadata = page.metadata.as_ref().unwrap();
        assert_eq!(
            observed(&page, MetadataField::Head),
            DetailValueState::Known
        );
        assert_eq!(
            observed(&page, MetadataField::Base),
            DetailValueState::Known
        );
        let head_repository = metadata.values.head.as_ref().unwrap().repository.as_ref();
        let base_repository = metadata.values.base.as_ref().unwrap().repository.as_ref();
        if change == 0 {
            assert!(base_repository.is_none());
        } else {
            assert_eq!(base_repository.unwrap().provider_id, PROJECT.to_string());
        }
        if matches!(change, 1 | 2) {
            assert!(head_repository.is_none());
        } else if change == 3 {
            let repository = head_repository.unwrap();
            assert_eq!(repository.provider_id, PROJECT.to_string());
            assert_eq!(repository.full_name, "org/subgroup/project");
        } else {
            assert_eq!(
                head_repository.unwrap().provider_id,
                SOURCE_PROJECT.to_string()
            );
        }
        task.join().unwrap();
    }
}

#[tokio::test]
async fn singleton_repository_identity_rejects_malformed_or_conflicting_ids() {
    for change in 0..4 {
        let mut payload = merge_request(NATIVE, PROJECT, 67);
        match change {
            0 => payload["target_project_id"] = json!(PROJECT + 1),
            1 => payload["target_project_id"] = json!(PROJECT.to_string()),
            2 => payload["source_project_id"] = json!(0),
            _ => payload["source_project_id"] = json!(SOURCE_PROJECT.to_string()),
        }
        let (provider, task) =
            server(|_| vec![response(200, "RateLimit-Remaining: 0\r\n", &body(&payload))]);
        let error = provider
            .fetch_detail(&token(), detail(RemoteItemKind::PullRequest))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        task.join().unwrap();
    }
}

#[tokio::test]
async fn descriptions_distinguish_omitted_null_empty_and_oversized_without_invented_refs() {
    for index in 0..4 {
        let mut payload = merge_request(NATIVE, PROJECT, 67);
        payload.as_object_mut().unwrap().remove("updated_at");
        payload.as_object_mut().unwrap().remove("labels");
        payload["sha"] = json!("");
        payload["diff_refs"] = json!({});
        match index {
            0 => {
                payload.as_object_mut().unwrap().remove("description");
            }
            1 => payload["description"] = Value::Null,
            2 => payload["description"] = json!(""),
            _ => payload["description"] = json!("x".repeat(1_048_577)),
        }
        let (provider, task) = server(|_| vec![response(200, "", &body(&payload))]);
        let page = provider
            .fetch_detail(&token(), detail(RemoteItemKind::PullRequest))
            .await
            .unwrap();
        assert_eq!(
            page.body.state,
            match index {
                0 => DetailValueState::Omitted,
                3 => DetailValueState::Oversized,
                _ => DetailValueState::Known,
            }
        );
        assert_eq!(
            page.body.text.as_deref(),
            if index == 2 { Some("") } else { None }
        );
        assert!(page.source.provider_updated_at.is_none());
        assert_eq!(
            observed(&page, MetadataField::Labels),
            DetailValueState::Omitted
        );
        assert_eq!(
            observed(&page, MetadataField::Head),
            DetailValueState::Omitted
        );
        assert_eq!(
            observed(&page, MetadataField::Base),
            DetailValueState::Omitted
        );
        assert!(page.metadata.as_ref().unwrap().values.head.is_none());
        task.join().unwrap();
    }
}

#[tokio::test]
async fn async_refs_require_known_matching_head_before_validating_base() {
    for change in 0..8 {
        let mut payload = merge_request(NATIVE, PROJECT, 67);
        match change {
            0 => {
                payload.as_object_mut().unwrap().remove("diff_refs");
            }
            1 => payload["diff_refs"] = Value::Null,
            2 => payload["diff_refs"] = json!({"head_sha":MERGE_BASE,"start_sha":BASE}),
            3 => {
                payload["sha"] = Value::Null;
                payload["diff_refs"] = json!({"head_sha":null,"start_sha":null});
            }
            4 => payload["diff_refs"] = json!({"start_sha":BASE}),
            5 => {
                payload["sha"] = Value::Null;
                payload["diff_refs"] = json!({"head_sha":null,"start_sha":BASE});
            }
            6 => {
                payload.as_object_mut().unwrap().remove("sha");
                payload["diff_refs"] = json!({"start_sha":BASE});
            }
            _ => {
                payload["sha"] = json!("");
                payload["diff_refs"] = json!({"head_sha":"","start_sha":BASE});
            }
        }
        let (provider, task) = server(|_| vec![response(200, "", &body(&payload))]);
        let page = provider
            .fetch_detail(&token(), detail(RemoteItemKind::PullRequest))
            .await
            .unwrap();
        assert_eq!(
            observed(&page, MetadataField::Base),
            DetailValueState::Omitted
        );
        assert_eq!(
            observed(&page, MetadataField::Head),
            if change == 3 || change >= 5 {
                DetailValueState::Omitted
            } else {
                DetailValueState::Known
            }
        );
        assert_eq!(page.body.state, DetailValueState::Known);
        assert_eq!(page.body.text.as_deref(), Some("independent Body"));
        let metadata = page.metadata.as_ref().unwrap();
        assert!(metadata.values.base.is_none());
        assert_eq!(metadata.values.title.as_deref(), Some("MR Δ 🚀"));
        assert_eq!(metadata.values.state.as_deref(), Some("open"));
        assert_eq!(
            observed(&page, MetadataField::MergeBase),
            DetailValueState::Omitted
        );
        assert!(
            page.metadata
                .as_ref()
                .unwrap()
                .values
                .merge_base_oid
                .is_none()
        );
        task.join().unwrap();
    }
}

#[tokio::test]
async fn bounded_metadata_marks_oversized_fields_without_discarding_body() {
    let mut payload = issue(NATIVE, PROJECT, 67);
    payload["title"] = json!("t".repeat(16385));
    payload["labels"] = json!(vec!["label"; 101]);
    payload["assignees"] = json!([]);
    payload["milestone"] = Value::Null;
    let (provider, task) = server(|_| vec![response(200, "", &body(&payload))]);
    let page = provider
        .fetch_detail(&token(), detail(RemoteItemKind::Issue))
        .await
        .unwrap();
    assert_eq!(page.body.text.as_deref(), Some("independent Body"));
    assert_eq!(
        observed(&page, MetadataField::Title),
        DetailValueState::Oversized
    );
    assert_eq!(
        observed(&page, MetadataField::Labels),
        DetailValueState::Oversized
    );
    assert_eq!(
        observed(&page, MetadataField::Assignees),
        DetailValueState::Known
    );
    assert_eq!(
        observed(&page, MetadataField::Milestone),
        DetailValueState::Known
    );
    assert!(page.metadata.as_ref().unwrap().values.title.is_none());
    assert!(page.metadata.as_ref().unwrap().values.labels.is_empty());
    task.join().unwrap();
}

#[tokio::test]
async fn singleton_identity_and_malformed_metadata_never_cross_subject_authority() {
    for change in 0..9 {
        let mut payload = issue(NATIVE, PROJECT, 67);
        match change {
            0 => payload["id"] = json!(NATIVE + 1),
            1 => payload["iid"] = json!(68),
            2 => payload["project_id"] = json!(PROJECT + 1),
            3 => payload["web_url"] = json!("https://gitlab.com/org/project/-/merge_requests/67"),
            4 => payload["description"] = json!(10),
            5 => payload["author"]["id"] = json!("9"),
            6 => payload["labels"] = Value::Null,
            7 => payload["updated_at"] = json!("not-a-clock"),
            _ => payload["milestone"]["web_url"] = json!("https://foreign.invalid/private"),
        }
        let (provider, task) =
            server(|_| vec![response(200, "RateLimit-Remaining: 0\r\n", &body(&payload))]);
        let error = provider
            .fetch_detail(&token(), detail(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        task.join().unwrap();
    }
}

#[tokio::test]
async fn resource_auth_permission_absence_and_transient_errors_are_safe_and_bounded() {
    for (status, kind) in [
        (401, ProviderErrorKind::Authentication),
        (403, ProviderErrorKind::Permission),
        (404, ProviderErrorKind::NotFound),
        (429, ProviderErrorKind::RateLimited),
        (503, ProviderErrorKind::Unavailable),
        (408, ProviderErrorKind::Unavailable),
        (409, ProviderErrorKind::Unavailable),
        (304, ProviderErrorKind::InvalidResponse),
    ] {
        let (provider, task) = server(|_| {
            vec![response(
                status,
                "Retry-After: 3\r\nRateLimit-Remaining: 0\r\n",
                "private provider error body",
            )]
        });
        let error = provider
            .fetch_detail(&token(), detail(RemoteItemKind::Issue))
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.account_cooldown_seconds, Some(60));
        if matches!(status, 503 | 408 | 409) {
            assert_eq!(error.retry_after_seconds, Some(3));
        }
        assert!(!format!("{error:?}").contains("private provider error body"));
        assert!(!format!("{error:?}").contains("synthetic_gitlab_token"));
        task.join().unwrap();
    }
}

#[tokio::test]
async fn malformed_local_resource_authority_and_unsupported_facets_never_send_requests() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let provider = GitlabProvider::fixture(
        reqwest::Url::parse(&format!(
            "http://{}/api/v4/",
            listener.local_addr().unwrap()
        ))
        .unwrap(),
    );
    for change in 0..9 {
        let mut input = detail(RemoteItemKind::Issue);
        match change {
            0 => input.subject.account_id = "foreign".into(),
            1 => input.repository.account_id = "foreign".into(),
            2 => input.repository.provider_id = "042".into(),
            3 => input.subject.number = Some("067".into()),
            4 => input.subject.repository_id = None,
            5 => input.cursor = Some("page=2".into()),
            6 => input.facet = DetailFacet::Activity,
            7 => input.account.host = "gitlab.enterprise.invalid".into(),
            _ => input.account.state = AccountState::Disconnected,
        }
        let error = provider.fetch_detail(&token(), input).await.unwrap_err();
        assert!(matches!(
            error.kind,
            ProviderErrorKind::InvalidResponse
                | ProviderErrorKind::Unsupported
                | ProviderErrorKind::Authentication
        ));
    }
    for kind in [FeedKind::PullRequests, FeedKind::Issues] {
        let mut input = feed(kind);
        input.repository.as_mut().unwrap().selected = false;
        assert_eq!(
            provider
                .fetch_page(&token(), input.clone())
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        input.repository = None;
        assert_eq!(
            provider.fetch_page(&token(), input).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
