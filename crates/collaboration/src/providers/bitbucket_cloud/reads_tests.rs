//! Actual loopback HTTP qualification of divergent PR identities and authority.
use super::{tests::*, *};
use crate::resource_metadata::MetadataField;
use serde_json::{Value, json};

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const QUERY: &str = "state=OPEN&state=MERGED&state=DECLINED&state=SUPERSEDED&pagelen=50&sort=id";
fn repository_ref(uuid: &str) -> RemoteRepository {
    RemoteRepository {
        id: format!("bitbucket_cloud:repository:{uuid}"),
        account_id: "fixture-account".into(),
        provider_id: uuid.into(),
        full_name: "old/project".into(),
        name: "project".into(),
        web_url: "https://bitbucket.org/old/project".into(),
        description: None,
        default_branch: None,
        selected: true,
    }
}
fn feed(uuid: &str) -> FeedRequest {
    let mut req = request();
    req.kind = FeedKind::PullRequests;
    req.repository = Some(repository_ref(uuid));
    req
}
fn pull(uuid: &str, id: u64, state: &str) -> Value {
    json!({"type":"pullrequest","id":id,"title":"Δ pull","state":state,"updated_on":"2026-10-04T12:00:00Z",
        "rendered":{"description":{"raw":"line one\nline two"}},"summary":{"raw":"not Body authority"},
        "author":{"type":"user","uuid":format!("{{{ACTOR}}}"),"nickname":"same-nickname","links":{"html":{"href":"https://bitbucket.org/same-nickname/"}}},
        "links":{"html":{"href":format!("https://bitbucket.org/renamed/project/pull-requests/{id}")}},
        "source":{"branch":{"name":"feature"},"commit":{"hash":HEAD},"repository":{"type":"repository","uuid":format!("{{{W1}}}")}},
        "destination":{"branch":{"name":"main"},"commit":{"hash":BASE},"repository":{"type":"repository","uuid":format!("{{{uuid}}}")}},
        "draft":true,"merged_at":"2026-10-04T12:00:00Z","participants":[{"approved":true}],"task_count":9})
}
fn detail(uuid: &str) -> DetailRequest {
    let req = feed(uuid);
    DetailRequest {
        account: req.account,
        repository: req.repository.unwrap(),
        subject: RemoteItem {
            id: format!("bitbucket_cloud:pull:{uuid}:67"),
            account_id: "fixture-account".into(),
            repository_id: Some(format!("bitbucket_cloud:repository:{uuid}")),
            provider_id: format!("{uuid}:67"),
            kind: RemoteItemKind::PullRequest,
            number: Some("67".into()),
            title: "old".into(),
            body: Some("preview".into()),
            body_omitted: false,
            author: None,
            web_url: None,
            state: "open".into(),
            updated_at: "2026-10-03T12:00:00Z".into(),
            head_oid: Some(HEAD.into()),
            is_draft: None,
            reason: None,
            unread: None,
        },
        facet: DetailFacet::Body,
        cursor: None,
        etag: Some("not-authority".into()),
        source: None,
    }
}
fn url(base: &str, uuid: &str, opaque: &str) -> String {
    format!("{base}repositories/%7B%7D/%7B{uuid}%7D/pullrequests?{QUERY}&page={opaque}")
}
fn field(page: &DetailPage, field: MetadataField) -> DetailValueState {
    page.metadata
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .find(|v| v.field == field)
        .unwrap()
        .state
}
#[tokio::test]
async fn actual_all_state_uuid_list_maps_closed_reasons_without_child_authority() {
    let (provider, calls) = server(|_| {
        vec![ok(collection(
            vec![
                pull(REPO, 1, "OPEN"),
                pull(REPO, 2, "MERGED"),
                pull(REPO, 3, "DECLINED"),
                pull(REPO, 4, "SUPERSEDED"),
            ],
            None,
        ))]
    });
    let page = provider.fetch_page(&token(), feed(REPO)).await.unwrap();
    assert_eq!(
        page.items
            .iter()
            .map(|v| (v.state.as_str(), v.reason.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("open", None),
            ("merged", None),
            ("closed", Some("declined")),
            ("closed", Some("superseded"))
        ]
    );
    assert!(page.items.iter().all(|v| v.is_draft.is_none()
        && v.body.as_deref() == Some("line one\nline two")
        && v.head_oid.as_deref() == Some(HEAD)));
    assert!(
        page.next_cursor.is_none()
            && page.etag.is_none()
            && page.last_modified.is_none()
            && !page.not_modified
    );
    let calls = calls.join().unwrap();
    assert!(calls[0].starts_with(&format!(
        "GET /2.0/repositories/%7B%7D/%7B{REPO}%7D/pullrequests?{QUERY} HTTP/1.1"
    )));
    assert!(!calls[0].to_ascii_lowercase().contains("if-none-match"));
}
#[tokio::test]
async fn actual_same_number_has_repository_compound_identity_and_account_partition() {
    let (provider, calls) = server(|_| {
        vec![
            ok(collection(vec![pull(REPO, 67, "OPEN")], None)),
            ok(collection(vec![pull(W2, 67, "OPEN")], None)),
            ok(collection(vec![pull(REPO, 67, "OPEN")], None)),
        ]
    });
    let a = provider
        .fetch_page(&token(), feed(REPO))
        .await
        .unwrap()
        .items
        .remove(0);
    let b = provider
        .fetch_page(&token(), feed(W2))
        .await
        .unwrap()
        .items
        .remove(0);
    let mut other = feed(REPO);
    other.account.id = "other-account".into();
    other.repository.as_mut().unwrap().account_id = "other-account".into();
    let c = provider
        .fetch_page(&token(), other)
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(a.provider_id, format!("{REPO}:67"));
    assert_eq!(a.id, format!("bitbucket_cloud:pull:{REPO}:67"));
    assert_ne!(a.id, b.id);
    assert_ne!(a.provider_id, b.provider_id);
    assert_eq!(a.number, b.number);
    assert_eq!(a.id, c.id);
    assert_ne!(a.account_id, c.account_id);
    assert_eq!(calls.join().unwrap().len(), 3);
}
#[tokio::test]
async fn actual_opaque_state_multiset_and_brace_aliases_retain_one_history() {
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![pull(REPO, 1, "OPEN")],
                Some(format!(
                    "{base}repositories/{{}}/{{{REPO}}}/pullrequests?sort=id&state=SUPERSEDED&state=OPEN&state=DECLINED&state=MERGED&pagelen=50&cursor=opaque%2B%2F%3D"
                )),
            )),
            ok(collection(vec![pull(REPO, 2, "MERGED")], None)),
        ]
    });
    let mut req = feed(REPO);
    req.cursor = provider
        .fetch_page(&token(), req.clone())
        .await
        .unwrap()
        .next_cursor;
    assert!(req.cursor.as_ref().unwrap().len() <= 4096);
    let page = provider.fetch_page(&token(), req).await.unwrap();
    assert!(page.next_cursor.is_none());
    let calls = calls.join().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].contains("cursor=opaque%2B%2F%3D"));
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(vec![], Some(url(base, REPO, "A")))),
            ok(collection(vec![], Some(url(base, REPO, "B")))),
            ok(collection(
                vec![],
                Some(format!(
                    "{base}repositories/%7b%7d/%7b{REPO}%7d/pullrequests?page=A&sort=id&pagelen=50&state=SUPERSEDED&state=DECLINED&state=MERGED&state=OPEN"
                )),
            )),
        ]
    });
    let mut req = feed(REPO);
    for _ in 0..2 {
        req.cursor = provider
            .fetch_page(&token(), req.clone())
            .await
            .unwrap()
            .next_cursor;
    }
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 3);
}
#[tokio::test]
async fn actual_pr_continuations_cannot_change_scope_filters_or_secret_authority() {
    for fault in [
        "origin",
        "user",
        "password",
        "repo",
        "path",
        "double-slash",
        "missing-state",
        "duplicate-state",
        "unknown-state",
        "size",
        "sort",
        "duplicate",
        "secret",
        "dot",
        "fragment",
    ] {
        let (provider, calls) = server(|base| {
            let valid = url(base, REPO, "opaque");
            let next = match fault {
                "origin" => valid.replace(base, "https://evil.example/2.0/"),
                "user" => valid.replacen("http://", "http://actor@", 1),
                "password" => valid.replacen("http://", "http://actor:secret@", 1),
                "repo" => valid.replace(REPO, W2),
                "path" => valid.replace("pullrequests", "issues"),
                "double-slash" => valid.replace("%7B%7D/", "/"),
                "missing-state" => valid.replace("state=MERGED&", ""),
                "duplicate-state" => format!("{valid}&state=OPEN"),
                "unknown-state" => valid.replace("state=MERGED", "state=FUTURE"),
                "size" => valid.replace("pagelen=50", "pagelen=100"),
                "sort" => valid.replace("sort=id", "sort=-id"),
                "duplicate" => format!("{valid}&sort=id"),
                "secret" => format!("{valid}&access_token=secret"),
                "dot" => valid.replace("/pullrequests", "/../pullrequests"),
                _ => format!("{valid}#fragment"),
            };
            vec![ok(collection(vec![pull(REPO, 1, "OPEN")], Some(next)))]
        });
        assert_eq!(
            provider
                .fetch_page(&token(), feed(REPO))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse,
            "{fault}"
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn actual_feed_replay_order_identity_and_value_bounds_fail_the_entire_page() {
    for fault in [
        "duplicate",
        "reverse",
        "destination",
        "type",
        "id",
        "cap",
        "state",
        "url",
    ] {
        let mut row = pull(REPO, 2, "OPEN");
        match fault {
            "destination" => row["destination"]["repository"]["uuid"] = format!("{{{W2}}}").into(),
            "type" => row["type"] = "issue".into(),
            "id" => row["id"] = 0.into(),
            "state" => row["state"] = "FUTURE".into(),
            "url" => row["links"]["html"]["href"] = "https://evil.example/pr/2".into(),
            _ => {}
        }
        let values = match fault {
            "duplicate" => vec![row.clone(), row],
            "reverse" => vec![row, pull(REPO, 1, "OPEN")],
            "cap" => vec![row; 51],
            _ => vec![row],
        };
        let (provider, calls) = server(|_| {
            vec![response(
                200,
                "Retry-After: 120\r\n",
                &collection(values, None),
            )]
        });
        let error = provider.fetch_page(&token(), feed(REPO)).await.unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse, "{fault}");
        assert_eq!(error.account_cooldown_seconds, Some(120));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    let (provider, calls) = server(|base| {
        vec![
            ok(collection(
                vec![pull(REPO, 67, "OPEN")],
                Some(url(base, REPO, "2")),
            )),
            ok(collection(vec![pull(REPO, 67, "OPEN")], None)),
        ]
    });
    let mut req = feed(REPO);
    req.cursor = provider
        .fetch_page(&token(), req.clone())
        .await
        .unwrap()
        .next_cursor;
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
    assert_eq!(calls.join().unwrap().len(), 2);
}
#[tokio::test]
async fn actual_cursor_is_bound_before_http_and_cannot_reset_twenty_page_budget() {
    let (provider, calls) = server(|base| {
        (0..20)
            .map(|index| {
                ok(collection(
                    vec![],
                    Some(url(base, REPO, &format!("next-{index}"))),
                ))
            })
            .collect()
    });
    let mut req = feed(REPO);
    for _ in 0..20 {
        req.cursor = provider
            .fetch_page(&token(), req.clone())
            .await
            .unwrap()
            .next_cursor;
        assert!(req.cursor.is_some());
    }
    assert_eq!(calls.join().unwrap().len(), 20); // HTTP listener closed: InvalidResponse proves pre-HTTP cap.
    let raw = req.cursor.clone().unwrap();
    assert!(raw.len() <= 4096);
    assert_eq!(
        provider
            .fetch_page(&token(), req.clone())
            .await
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    for key in [
        "account",
        "epoch",
        "repository",
        "kind",
        "version",
        "pages",
        "last_id",
        "seen_pages",
        "url",
        "unknown",
    ] {
        let mut cursor: Value = serde_json::from_str(&raw).unwrap();
        cursor[key] = match key {
            "version" => 2.into(),
            "pages" => 0.into(),
            "last_id" => 0.into(),
            "seen_pages" => json!(vec!["a".repeat(64); 2]),
            _ => "foreign".into(),
        };
        let mut changed = req.clone();
        changed.cursor = Some(cursor.to_string());
        assert_eq!(
            provider
                .fetch_page(&token(), changed)
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse,
            "{key}"
        );
    }
    req.cursor = Some("x".repeat(4097));
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
}
#[tokio::test]
async fn actual_terminal_twentieth_page_completes_but_empty_intermediate_pages_do_not() {
    let (provider, calls) = server(|base| {
        (0..20)
            .map(|index| {
                ok(collection(
                    vec![],
                    (index < 19).then(|| url(base, REPO, &index.to_string())),
                ))
            })
            .collect()
    });
    let mut req = feed(REPO);
    for index in 0..20 {
        let page = provider.fetch_page(&token(), req.clone()).await.unwrap();
        assert_eq!(page.next_cursor.is_none(), index == 19);
        req.cursor = page.next_cursor;
    }
    assert_eq!(calls.join().unwrap().len(), 20);
}
#[tokio::test]
async fn actual_singleton_body_metadata_refs_have_independent_named_authority() {
    let mut value = pull(REPO, 67, "SUPERSEDED");
    value["description"] = "line one\nline two".into();
    value["destination"]["repository"]["full_name"] = "transferred/project".into();
    value["destination"]["repository"]["links"] =
        json!({"html":{"href":"https://bitbucket.org/transferred/project"}});
    let (provider, calls) = server(|_| vec![ok(value)]);
    let page = provider.fetch_detail(&token(), detail(REPO)).await.unwrap();
    assert_eq!(page.body.text.as_deref(), Some("line one\nline two"));
    assert_eq!(page.body.state, DetailValueState::Known);
    assert_eq!(page.source.source, resource_details::SOURCE);
    assert_eq!(page.source.field_mask, vec![DetailField::Body]);
    assert!(
        page.entries.is_empty()
            && page.next_cursor.is_none()
            && page.etag.is_none()
            && !page.not_modified
    );
    let metadata = page.metadata.as_ref().unwrap();
    assert_eq!(metadata.values.state.as_deref(), Some("closed"));
    assert_eq!(metadata.values.state_reason.as_deref(), Some("superseded"));
    assert_eq!(metadata.values.head.as_ref().unwrap().oid, HEAD);
    assert_eq!(metadata.values.head.as_ref().unwrap().repository, None);
    assert_eq!(
        metadata
            .values
            .base
            .as_ref()
            .unwrap()
            .repository
            .as_ref()
            .unwrap()
            .full_name,
        "transferred/project"
    );
    assert_eq!(metadata.values.author.as_ref().unwrap().provider_id, ACTOR);
    for omitted in [
        MetadataField::Labels,
        MetadataField::Assignees,
        MetadataField::Milestone,
        MetadataField::IsDraft,
        MetadataField::MergedAt,
    ] {
        assert_eq!(field(&page, omitted), DetailValueState::Omitted);
    }
    let calls = calls.join().unwrap();
    assert!(calls[0].starts_with(&format!(
        "GET /2.0/repositories/%7B%7D/%7B{REPO}%7D/pullrequests/67 HTTP/1.1"
    )));
    assert!(!calls[0].to_ascii_lowercase().contains("if-none-match"));
}
#[tokio::test]
async fn actual_body_omission_empty_null_and_oversize_never_use_html_summary_or_fallback() {
    for mode in [
        "missing-rendered",
        "missing-description",
        "missing-raw",
        "null",
        "empty",
        "oversized",
    ] {
        let mut value = pull(REPO, 67, "OPEN");
        let (expected, text) = match mode {
            "missing-rendered" => {
                value.as_object_mut().unwrap().remove("rendered");
                value["description"] = "fallback forbidden".into();
                (DetailValueState::Omitted, None)
            }
            "missing-description" => {
                value["rendered"] = json!({"title":{"raw":"title"}});
                (DetailValueState::Omitted, None)
            }
            "missing-raw" => {
                value["rendered"]["description"] = json!({"html":"<p>forbidden</p>"});
                (DetailValueState::Omitted, None)
            }
            "null" => {
                value["rendered"]["description"]["raw"] = Value::Null;
                value["description"] = Value::Null;
                (DetailValueState::Known, None)
            }
            "empty" => {
                value["rendered"]["description"]["raw"] = "".into();
                (DetailValueState::Known, Some(""))
            }
            _ => {
                value["rendered"]["description"]["raw"] = "x".repeat(1_048_577).into();
                (DetailValueState::Oversized, None)
            }
        };
        let (provider, calls) = server(|_| vec![ok(value)]);
        let page = provider.fetch_detail(&token(), detail(REPO)).await.unwrap();
        assert_eq!(page.body.state, expected, "{mode}");
        assert_eq!(page.body.text.as_deref(), text);
        assert_eq!(field(&page, MetadataField::Title), DetailValueState::Known);
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn actual_conflicting_or_malformed_canonical_body_rejects_with_quota() {
    for mode in [
        "conflict",
        "null-conflict",
        "number",
        "nul",
        "malformed-rendered",
    ] {
        let mut value = pull(REPO, 67, "OPEN");
        match mode {
            "conflict" => value["description"] = "different".into(),
            "null-conflict" => value["description"] = Value::Null,
            "number" => value["rendered"]["description"]["raw"] = 42.into(),
            "nul" => value["rendered"]["description"]["raw"] = "hello\0world".into(),
            _ => value["rendered"] = json!([]),
        }
        let (provider, calls) = server(|_| vec![response(200, "Retry-After: 172800\r\n", &value)]);
        let error = provider
            .fetch_detail(&token(), detail(REPO))
            .await
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(error.account_cooldown_seconds, Some(172800));
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn actual_singleton_response_and_captured_subject_must_match_exact_identity() {
    for fault in ["id", "uuid", "type"] {
        let mut value = pull(REPO, 67, "OPEN");
        match fault {
            "id" => value["id"] = 68.into(),
            "uuid" => value["destination"]["repository"]["uuid"] = format!("{{{W2}}}").into(),
            _ => value["type"] = "issue".into(),
        };
        let (provider, calls) = server(|_| vec![ok(value)]);
        assert_eq!(
            provider
                .fetch_detail(&token(), detail(REPO))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    let (provider, calls) = server(|_| vec![]);
    calls.join().unwrap();
    for fault in [
        "account",
        "repository",
        "provider",
        "id",
        "number",
        "selected",
        "cursor",
        "epoch",
    ] {
        let mut req = detail(REPO);
        match fault {
            "account" => req.subject.account_id = "foreign".into(),
            "repository" => req.subject.repository_id = Some("foreign".into()),
            "provider" => req.subject.provider_id = "67".into(),
            "id" => req.subject.id = "foreign".into(),
            "number" => req.subject.number = Some("067".into()),
            "selected" => req.repository.selected = false,
            "cursor" => req.cursor = Some("arbitrary".into()),
            _ => req.account.authorization_epoch = "01".into(),
        };
        assert_eq!(
            provider.fetch_detail(&token(), req).await.unwrap_err().kind,
            ProviderErrorKind::InvalidResponse,
            "{fault}"
        );
    }
    for facet in [
        DetailFacet::Comments,
        DetailFacet::Reviews,
        DetailFacet::Checks,
    ] {
        let mut req = detail(REPO);
        req.facet = facet;
        assert_eq!(
            provider.fetch_detail(&token(), req).await.unwrap_err().kind,
            ProviderErrorKind::Unsupported
        );
    }
}
#[tokio::test]
async fn actual_abbreviated_or_missing_source_head_cannot_promote_base() {
    for mode in [
        "abbreviated",
        "missing-source",
        "missing-hash",
        "null",
        "deleted-fork",
        "complete64",
    ] {
        let mut value = pull(REPO, 67, "OPEN");
        let known = matches!(mode, "deleted-fork" | "complete64");
        match mode {
            "abbreviated" => value["source"]["commit"]["hash"] = "aaaaaaa".into(),
            "missing-source" => {
                value.as_object_mut().unwrap().remove("source");
            }
            "missing-hash" => value["source"]["commit"] = json!({}),
            "null" => value["source"]["commit"]["hash"] = Value::Null,
            "deleted-fork" => value["source"]["repository"] = Value::Null,
            _ => value["source"]["commit"]["hash"] = "A".repeat(64).into(),
        };
        let (provider, calls) = server(|_| vec![ok(value)]);
        let page = provider.fetch_detail(&token(), detail(REPO)).await.unwrap();
        assert_eq!(
            field(&page, MetadataField::Head) == DetailValueState::Known,
            known,
            "{mode}"
        );
        assert_eq!(
            field(&page, MetadataField::Base) == DetailValueState::Known,
            known,
            "{mode}"
        );
        if known {
            assert_eq!(
                page.metadata
                    .as_ref()
                    .unwrap()
                    .values
                    .head
                    .as_ref()
                    .unwrap()
                    .repository,
                None
            )
        }
        assert_eq!(calls.join().unwrap().len(), 1);
    }
    for fault in ["bad-hash", "short-hash", "branch", "fork-uuid", "fork-link"] {
        let mut value = pull(REPO, 67, "OPEN");
        match fault {
            "bad-hash" => value["source"]["commit"]["hash"] = "x".repeat(40).into(),
            "short-hash" => value["source"]["commit"]["hash"] = "aaaaaa".into(),
            "branch" => value["source"]["branch"]["name"] = "".into(),
            "fork-uuid" => value["source"]["repository"]["uuid"] = "bad".into(),
            _ => {
                value["source"]["repository"]["links"] =
                    json!({"html":{"href":"https://evil.example/team/project"}})
            }
        };
        let (provider, calls) = server(|_| vec![ok(value)]);
        assert_eq!(
            provider
                .fetch_detail(&token(), detail(REPO))
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse,
            "{fault}"
        );
        assert_eq!(calls.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn actual_pr_list_and_detail_access_rate_signals_keep_scope_and_minimum_wait() {
    for detail_call in [false, true] {
        for (status, kind) in [
            (401, ProviderErrorKind::Authentication),
            (403, ProviderErrorKind::Permission),
            (404, ProviderErrorKind::NotFound),
            (429, ProviderErrorKind::RateLimited),
            (503, ProviderErrorKind::Unavailable),
        ] {
            let (provider, calls) = server(|_| {
                vec![response(
                    status,
                    "Retry-After: 120\r\n",
                    &json!({"error":"private"}),
                )]
            });
            let error = if detail_call {
                provider
                    .fetch_detail(&token(), detail(REPO))
                    .await
                    .unwrap_err()
            } else {
                provider.fetch_page(&token(), feed(REPO)).await.unwrap_err()
            };
            assert_eq!(error.kind, kind);
            assert_eq!(error.account_cooldown_seconds, Some(120));
            assert!(!error.to_string().contains("private"));
            assert_eq!(calls.join().unwrap().len(), 1);
        }
    }
}

#[tokio::test]
async fn actual_old_discovery_fingerprint_bytes_and_persisted_loop_history_are_compatible() {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    let (provider, calls) = server(|base| {
        vec![ok(collection(
            vec![],
            Some(format!("{base}user/workspaces?pagelen=10&page=A")),
        ))]
    });
    let mut req = request();
    req.cursor = provider
        .fetch_page(&token(), req.clone())
        .await
        .unwrap()
        .next_cursor;
    assert_eq!(calls.join().unwrap().len(), 1);
    let mut cursor: Value = serde_json::from_str(req.cursor.as_ref().unwrap()).unwrap();
    let raw = cursor["stage"]["url"].as_str().unwrap();
    let parsed = reqwest::Url::parse(raw).unwrap();
    let pairs: BTreeMap<_, _> = parsed.query_pairs().collect();
    let old_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(parsed.path(), pairs)).unwrap())
    );
    assert_eq!(
        provider.http.fingerprint(raw, &Route::Workspaces).unwrap(),
        old_hash
    );
    cursor["seen_pages"] = json!([old_hash]);
    req.cursor = Some(cursor.to_string());
    assert_eq!(
        provider.fetch_page(&token(), req).await.unwrap_err().kind,
        ProviderErrorKind::InvalidResponse
    );
}

#[tokio::test]
async fn actual_singleton_oversized_title_preserves_independent_body_observation() {
    let mut value = pull(REPO, 67, "DECLINED");
    value["title"] = "x".repeat(16385).into();
    let (provider, calls) = server(|_| vec![ok(value)]);
    let page = provider.fetch_detail(&token(), detail(REPO)).await.unwrap();
    assert_eq!(page.body.text.as_deref(), Some("line one\nline two"));
    assert_eq!(
        field(&page, MetadataField::Title),
        DetailValueState::Oversized
    );
    assert_eq!(page.metadata.as_ref().unwrap().values.title, None);
    assert_eq!(
        page.metadata
            .as_ref()
            .unwrap()
            .values
            .state_reason
            .as_deref(),
        Some("declined")
    );
    assert_eq!(calls.join().unwrap().len(), 1);
}
