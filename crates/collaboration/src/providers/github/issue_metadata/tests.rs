use super::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread::JoinHandle,
    time::{Duration, Instant},
};
struct Reply {
    status: u16,
    body: Value,
    headers: String,
}
fn reply(body: Value) -> Reply {
    Reply {
        status: 200,
        body,
        headers: String::new(),
    }
}
fn server(replies: Vec<Reply>) -> (GithubProvider, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let provider =
        GithubProvider::for_test_base(reqwest::Url::parse(&format!("http://{addr}/")).unwrap());
    let handle = std::thread::spawn(move || {
        let mut requests = vec![];
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(s) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("fixture accept failed: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut b = [0; 1024];
                let n = stream.read(&mut b).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&b[..n]);
                if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
                assert!(bytes.len() < 16384);
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.contains("x-github-api-version: 2026-03-10"));
            requests.push(request.lines().next().unwrap().into());
            let body = if reply.status == 204 {
                String::new()
            } else {
                reply.body.to_string()
            };
            let headers = reply.headers.replace("{origin}", &format!("http://{addr}"));
            write!(stream,"HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",reply.status,body.len(),headers,body).unwrap();
        }
        requests
    });
    (provider, handle)
}
fn context() -> IssueMetadataReadContext {
    IssueMetadataReadContext {
        account: RemoteAccount {
            id: "a".into(),
            provider: ProviderKind::Github,
            host: "github.com".into(),
            actor_id: "7".into(),
            login: "actor".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: false,
        },
        repository: RemoteRepository {
            id: "r".into(),
            account_id: "a".into(),
            provider_id: "42".into(),
            full_name: "owner/repo".into(),
            name: "repo".into(),
            web_url: "https://github.com/owner/repo".into(),
            description: None,
            default_branch: Some("main".into()),
            selected: true,
        },
        authorization_view: "2".into(),
    }
}
fn request(kind: Kind) -> IssueMetadataCatalogRequest {
    IssueMetadataCatalogRequest {
        context: context(),
        kind,
        catalog_generation: "1".into(),
        cursor: None,
    }
}
fn token() -> SecretToken {
    SecretToken::new("synthetic-fixture".into()).unwrap()
}
fn label() -> IssueMetadataLabel {
    IssueMetadataLabel {
        provider_id: "9".into(),
        name: "bug".into(),
        color: None,
    }
}
fn point(point: IssueMetadataPoint) -> IssueMetadataPointRequest {
    IssueMetadataPointRequest {
        context: context(),
        point,
    }
}
#[tokio::test]
async fn three_catalog_families_preserve_native_ids_and_explicit_availability() {
    let (p, h) = server(vec![
        reply(
            json!([{"id":9,"name":"bug","color":"aabB00","archived_at":null},{"id":10,"name":"old","archived_at":"2026-01-01T00:00:00Z"},{"id":11,"name":"legacy"}]),
        ),
        reply(json!([{"id":18446744073709551615u64,"login":"bot[bot]"}])),
        reply(json!([{"id":15,"number":3,"title":"Ship","state":"closed"}])),
    ]);
    let page = p
        .fetch_issue_metadata_catalog(&token(), request(Kind::Labels))
        .await
        .unwrap();
    assert_eq!(
        page.options
            .iter()
            .map(|v| v.availability)
            .collect::<Vec<_>>(),
        [
            Availability::Available,
            Availability::Unavailable,
            Availability::Unknown
        ]
    );
    assert!(page.next_cursor.is_none());
    assert!(!page.truncated);
    let page = p
        .fetch_issue_metadata_catalog(&token(), request(Kind::Assignees))
        .await
        .unwrap();
    assert_eq!(option_id(&page.options[0]), u64::MAX.to_string());
    let page = p
        .fetch_issue_metadata_catalog(&token(), request(Kind::Milestones))
        .await
        .unwrap();
    assert_eq!(page.options[0].reason, Some(Reason::Closed));
    assert_eq!(
        h.join().unwrap(),
        [
            "GET /repositories/42/labels?per_page=100&page=1 HTTP/1.1",
            "GET /repositories/42/assignees?per_page=100&page=1 HTTP/1.1",
            "GET /repositories/42/milestones?per_page=100&page=1&state=all HTTP/1.1"
        ]
    );
}
#[tokio::test]
async fn cursor_is_bound_to_account_actor_epoch_view_repository_kind_and_counter() {
    let (p, h) = server(vec![Reply {
        status: 200,
        body: json!([]),
        headers: "Link: <{origin}/repositories/42/labels?per_page=100&page=2>; rel=\"next\"\r\n"
            .into(),
    }]);
    let cursor = p
        .fetch_issue_metadata_catalog(&token(), request(Kind::Labels))
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    h.join().unwrap();
    for field in [
        "account",
        "actor",
        "epoch",
        "view",
        "repository",
        "native_repository",
        "path",
        "kind",
        "page",
        "catalog_generation",
        "version",
    ] {
        let mut c: Value = serde_json::from_str(&cursor).unwrap();
        c[field] = if field == "page" {
            json!(21)
        } else if field == "version" {
            json!(2)
        } else {
            json!("other")
        };
        let mut r = request(Kind::Labels);
        r.cursor = Some(c.to_string());
        assert!(
            p.fetch_issue_metadata_catalog(&token(), r).await.is_err(),
            "{field}"
        );
    }
}
#[tokio::test]
async fn exact_native_continuation_advances_and_cap_is_terminal_refreshable() {
    let (p, h) = server(vec![
        Reply {
            status: 200,
            body: json!([]),
            headers:
                "Link: <{origin}/repositories/42/labels?per_page=100&page=3>; rel=\"next\"\r\n"
                    .into(),
        },
        Reply {
            status: 200,
            body: json!([]),
            headers:
                "Link: <{origin}/repositories/42/labels?per_page=100&page=21>; rel=\"next\"\r\n"
                    .into(),
        },
        reply(json!([])),
    ]);
    let mut r = request(Kind::Labels);
    r.cursor = Some(
        serde_json::to_string(&Cursor::new(&r.context, r.kind, &r.catalog_generation, 2)).unwrap(),
    );
    let page = p
        .fetch_issue_metadata_catalog(&token(), r.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Cursor>(page.next_cursor.as_ref().unwrap())
            .unwrap()
            .page,
        3
    );
    r.cursor = Some(
        serde_json::to_string(&Cursor::new(&r.context, r.kind, &r.catalog_generation, 20)).unwrap(),
    );
    let page = p.fetch_issue_metadata_catalog(&token(), r).await.unwrap();
    assert!(page.truncated);
    assert!(page.next_cursor.is_none());
    p.fetch_issue_metadata_catalog(&token(), request(Kind::Labels))
        .await
        .unwrap();
    assert!(h.join().unwrap()[2].contains("page=1 "));
}
#[tokio::test]
async fn foreign_cyclic_and_query_injected_continuations_fail_with_quota() {
    for next in [
        "https://evil.example/repositories/42/labels?per_page=100&page=2",
        "{origin}/repositories/43/labels?per_page=100&page=2",
        "{origin}/repositories/42/labels?per_page=100&page=1",
        "{origin}/repositories/42/labels?per_page=100&page=2&evil=1",
    ] {
        let (p, h) = server(vec![Reply {
            status: 200,
            body: json!([]),
            headers: format!("Retry-After: 120\r\nLink: <{next}>; rel=\"next\"\r\n"),
        }]);
        let e = p
            .fetch_issue_metadata_catalog(&token(), request(Kind::Labels))
            .await
            .unwrap_err();
        assert_eq!(e.kind, ProviderErrorKind::InvalidResponse);
        assert_eq!(e.account_cooldown_seconds, Some(120));
        assert_eq!(h.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn malformed_duplicate_oversized_rows_never_become_empty_with_quota() {
    for body in [
        json!({}),
        json!([{"id":9,"name":"bug"},{"id":9,"name":"bug"}]),
        json!([{"id":9,"name":"bug","archived_at":false}]),
        json!(
            (1..=101)
                .map(|n| json!({"id":n,"name":"bug"}))
                .collect::<Vec<_>>()
        ),
    ] {
        let (p, h) = server(vec![Reply {
            status: 200,
            body,
            headers: "Retry-After: 99\r\n".into(),
        }]);
        let e = p
            .fetch_issue_metadata_catalog(&token(), request(Kind::Labels))
            .await
            .unwrap_err();
        assert_eq!(e.account_cooldown_seconds, Some(99));
        h.join().unwrap();
    }
}
#[tokio::test]
async fn literal_label_segments_do_not_become_query_fragment_or_route() {
    let name = "bug/雪 #?%";
    let (p, h) = server(vec![reply(json!({"id":9,"name":name}))]);
    let mut v = label();
    v.name = name.into();
    let got = p
        .fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Label(v)))
        .await
        .unwrap();
    assert!(matches!(
        got.value,
        IssueMetadataPointValue::Selection {
            identity_matches: true,
            availability: Availability::Unknown,
            ..
        }
    ));
    let requests = h.join().unwrap();
    assert_eq!(
        requests,
        ["GET /repositories/42/labels/bug%2F%E9%9B%AA%20%23%3F%25 HTTP/1.1"]
    );
    for name in [".", ".."] {
        let mut v = label();
        v.name = name.into();
        assert!(
            p.fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Label(v)))
                .await
                .is_err()
        );
    }
}
#[tokio::test]
async fn recycled_renamed_and_archived_selections_are_distinct() {
    for (body, reason, matches) in [
        (
            json!({"id":10,"name":"bug"}),
            Reason::ChangedIdentity,
            false,
        ),
        (json!({"id":9,"name":"renamed"}), Reason::ChangedName, false),
        (
            json!({"id":9,"name":"bug","archived_at":"2026-01-01T00:00:00Z"}),
            Reason::Archived,
            true,
        ),
    ] {
        let (p, h) = server(vec![reply(body)]);
        let got = p
            .fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Label(label())))
            .await
            .unwrap();
        assert!(
            matches!(got.value,IssueMetadataPointValue::Selection{identity_matches,availability:Availability::Unavailable,reason:Some(got),..} if identity_matches==matches&&got==reason)
        );
        h.join().unwrap();
    }
}
#[tokio::test]
async fn assignee_requires_numeric_identity_then_exact_assignability_status() {
    let user = IssueMetadataAssignee {
        provider_id: "8".into(),
        login: "app[bot]".into(),
    };
    let (p, h) = server(vec![
        reply(json!({"id":8,"login":"app[bot]"})),
        Reply {
            status: 204,
            body: Value::Null,
            headers: "Retry-After: 70\r\n".into(),
        },
    ]);
    let r = p
        .fetch_issue_metadata_point(
            &token(),
            point(IssueMetadataPoint::AssigneeIdentity(user.clone())),
        )
        .await
        .unwrap();
    assert!(matches!(
        r.value,
        IssueMetadataPointValue::Selection {
            identity_matches: true,
            ..
        }
    ));
    let r = p
        .fetch_issue_metadata_point(
            &token(),
            point(IssueMetadataPoint::AssigneeAssignable(user.clone())),
        )
        .await
        .unwrap();
    assert!(matches!(r.value, IssueMetadataPointValue::Assignable));
    assert_eq!(r.cooldown_seconds, Some(70));
    assert_eq!(
        h.join().unwrap(),
        [
            "GET /user/8 HTTP/1.1",
            "GET /repositories/42/assignees/app[bot] HTTP/1.1"
        ]
    );
    for status in [200, 202, 301, 304, 404, 401, 429] {
        let (p, h) = server(vec![Reply {
            status,
            body: json!({}),
            headers: "Retry-After: 80\r\n".into(),
        }]);
        let e = p
            .fetch_issue_metadata_point(
                &token(),
                point(IssueMetadataPoint::AssigneeAssignable(user.clone())),
            )
            .await
            .unwrap_err();
        assert_eq!(e.account_cooldown_seconds, Some(80));
        if status == 401 {
            assert_eq!(e.kind, ProviderErrorKind::Authentication);
        }
        if status == 429 {
            assert_eq!(e.kind, ProviderErrorKind::RateLimited);
        }
        h.join().unwrap();
    }
}
#[tokio::test]
async fn repository_permission_is_three_state_and_foreign_identity_is_refused() {
    for (permission, expected) in [
        (Some(true), Availability::Available),
        (Some(false), Availability::Unavailable),
        (None, Availability::Unknown),
    ] {
        let mut v = json!({"id":42,"full_name":"owner/repo","has_issues":true,"archived":false});
        if let Some(p) = permission {
            v["permissions"] = json!({"push":p});
        }
        let (p, h) = server(vec![reply(v)]);
        let got = p
            .fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Repository))
            .await
            .unwrap();
        assert!(
            matches!(got.value,IssueMetadataPointValue::Repository{metadata_access}if metadata_access==expected)
        );
        h.join().unwrap();
    }
    let (p, h) = server(vec![Reply {
        status: 200,
        body: json!({"id":43,"full_name":"owner/repo","has_issues":true,"archived":false}),
        headers: "Retry-After: 100\r\n".into(),
    }]);
    assert_eq!(
        p.fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Repository))
            .await
            .unwrap_err()
            .account_cooldown_seconds,
        Some(100)
    );
    h.join().unwrap();
}
#[tokio::test]
async fn point_redirect_or_any_link_cannot_acquire_authority_and_quota_is_retained() {
    for (status, headers) in [
        (301, "Location: /repositories/42/labels/bug\r\n"),
        (200, "Link: garbage\r\n"),
    ] {
        let (p, h) = server(vec![Reply {
            status,
            body: json!({"id":9,"name":"bug"}),
            headers: format!("Retry-After: 90\r\n{headers}"),
        }]);
        let e = p
            .fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Label(label())))
            .await
            .unwrap_err();
        assert_eq!(e.account_cooldown_seconds, Some(90));
        assert_eq!(h.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn milestone_requires_immutable_id_and_repository_number_and_keeps_closed_state() {
    let selected = IssueMetadataMilestone {
        provider_id: "9".into(),
        number: "2".into(),
        title: "Release".into(),
    };
    let (p, h) = server(vec![
        reply(json!({"id":9,"number":2,"title":"renamed title","state":"closed"})),
        reply(json!({"id":10,"number":2,"title":"Release","state":"open"})),
    ]);
    let r = p
        .fetch_issue_metadata_point(
            &token(),
            point(IssueMetadataPoint::Milestone(selected.clone())),
        )
        .await
        .unwrap();
    assert!(matches!(
        r.value,
        IssueMetadataPointValue::Selection {
            identity_matches: true,
            reason: Some(Reason::Closed),
            ..
        }
    ));
    let r = p
        .fetch_issue_metadata_point(&token(), point(IssueMetadataPoint::Milestone(selected)))
        .await
        .unwrap();
    assert!(matches!(
        r.value,
        IssueMetadataPointValue::Selection {
            identity_matches: false,
            reason: Some(Reason::ChangedIdentity),
            ..
        }
    ));
    h.join().unwrap();
}
#[test]
fn malformed_oversized_missing_and_extra_optional_metadata_are_independent_outcomes() {
    use crate::issue_metadata::native::{AssigneeV2, LabelV2, MilestoneV2, SelectionV2};
    let selected = SelectionV2 {
        labels: vec![LabelV2 {
            id: "9".into(),
            name: "bug".into(),
            color: None,
        }],
        assignees: vec![AssigneeV2 {
            id: "8".into(),
            login: "a".into(),
        }],
        milestone: Some(MilestoneV2 {
            id: "7".into(),
            number: "1".into(),
            title: "m".into(),
        }),
    };
    let command = "00000000-0000-4000-8000-000000000001";
    let v = json!({"labels":[{"id":9},{"id":10}],"assignees":[{"id":8},{"id":11}],"milestone":{"id":7}});
    let o = observations::observe(&v, &selected)
        .outcome(command, &selected)
        .unwrap();
    assert!(
        o.fields
            .iter()
            .all(|f| f.result == IssueMetadataResult::Applied)
    );
    assert!(!o.needs_attention);
    for v in [
        json!({}),
        json!({"labels":false,"assignees":"bad","milestone":false}),
        json!({"labels":[{"id":9,"description":"x".repeat(32768)}],"assignees":[],"milestone":null}),
    ] {
        let o = observations::observe(&v, &selected)
            .outcome(command, &selected)
            .unwrap();
        assert!(o.needs_attention);
        assert!(matches!(
            o.fields[0].result,
            IssueMetadataResult::Unobserved
        ));
        assert!(
            n::encode(&observations::observe(&v, &selected))
                .unwrap()
                .len()
                < 1024
        );
    }
}
#[test]
fn identical_saved_cursor_rejects_new_context_or_catalog_generation() {
    let base = request(Kind::Labels);
    let cursor = serde_json::to_string(&Cursor::new(
        &base.context,
        base.kind,
        &base.catalog_generation,
        2,
    ))
    .unwrap();
    for i in 0..9 {
        let mut r = base.clone();
        r.cursor = Some(cursor.clone());
        match i {
            0 => r.context.account.id.push('x'),
            1 => r.context.account.actor_id = "8".into(),
            2 => r.context.account.authorization_epoch = "2".into(),
            3 => r.context.authorization_view = "3".into(),
            4 => r.context.repository.id.push('x'),
            5 => r.context.repository.provider_id = "43".into(),
            6 => r.context.repository.full_name = "owner/other".into(),
            7 => r.kind = Kind::Milestones,
            _ => r.catalog_generation = "2".into(),
        }
        assert!(Cursor::parse(&r).is_err(), "changed bound field {i}");
    }
    let mut r = base;
    r.context.account.id = "\u{1}".repeat(1024);
    r.context.repository.account_id = r.context.account.id.clone();
    r.context.repository.id = "\u{2}".repeat(1024);
    r.cursor = Some(
        serde_json::to_string(&Cursor::new(&r.context, r.kind, &r.catalog_generation, 2)).unwrap(),
    );
    assert!(r.cursor.as_ref().unwrap().len() < 16384);
    assert!(Cursor::parse(&r).is_ok());
}
#[tokio::test]
async fn unsupported_inactive_foreign_account_and_malformed_ids_refuse_before_http() {
    let (p, h) = server(vec![]);
    h.join().unwrap();
    for i in 0..5 {
        let mut r = request(Kind::Labels);
        match i {
            0 => r.context.account.provider = ProviderKind::Gitlab,
            1 => r.context.account.state = AccountState::AuthRequired,
            2 => r.context.repository.account_id = "other".into(),
            3 => r.context.repository.provider_id = "../../42".into(),
            _ => r.context.account.authorization_epoch = "01".into(),
        };
        let error = p
            .fetch_issue_metadata_catalog(&token(), r)
            .await
            .unwrap_err();
        assert_ne!(error.kind, ProviderErrorKind::Offline);
    }
}
fn receipt_fixture() -> (n::PayloadV2, n::PreparationV2, Value) {
    let c = context();
    let p = n::PayloadV2 {
        request: SubmitIssueV2Request {
            context: IssueDraftContext {
                account_id: c.account.id.clone(),
                repository_id: c.repository.id.clone(),
                authorization_epoch: c.account.authorization_epoch.clone(),
                authorization_view: c.authorization_view.clone(),
                review_token: "a".repeat(64),
            },
            draft_id: "00000000-0000-4000-8000-000000000001".into(),
            draft_generation: "1".into(),
            command_id: "00000000-0000-4000-8000-000000000002".into(),
            accept_background_delivery: true,
            accept_metadata_best_effort: true,
        },
        title: "Created".into(),
        body: "Text".into(),
        repository_native: c.repository.provider_id.clone(),
        metadata: n::SelectionV2::from_public(IssueMetadataSelection {
            labels: vec![label()],
            assignees: vec![IssueMetadataAssignee {
                provider_id: "8".into(),
                login: "user".into(),
            }],
            milestone: Some(IssueMetadataMilestone {
                provider_id: "10".into(),
                number: "1".into(),
                title: "Ship".into(),
            }),
        })
        .unwrap(),
    };
    let prep = n::PreparationV2 {
        frame: n::FrameV2 {
            account_id: c.account.id,
            repository_id: c.repository.id,
            repository_native: c.repository.provider_id,
            repository_path: c.repository.full_name,
            authorization_view: c.authorization_view,
        },
        actor: c.account.actor_id,
        epoch: c.account.authorization_epoch,
        command_hash: "b".repeat(64),
        revalidated_metadata: p.metadata.clone(),
        metadata_push_access: Some(true),
    };
    let v = json!({"id":100,"number":4,"title":"Created","body":"Text","url":"https://api.github.com/repos/owner/repo/issues/4","repository_url":"https://api.github.com/repos/owner/repo","html_url":"https://github.com/owner/repo/issues/4","user":{"id":7,"login":"actor"},"state":"open","created_at":"2026-10-08T00:00:00Z","updated_at":"2026-10-08T00:00:00Z","labels":[{"id":9}],"assignees":[{"id":8}],"milestone":{"id":10}});
    (p, prep, v)
}
#[test]
fn strong201_core_survives_missing_malformed_oversized_or_different_optional_fields() {
    let (p, prep, v) = receipt_fixture();
    n::validate_receipt_budget(&p, &prep).unwrap();
    for optional in [
        json!({}),
        json!({"labels":false,"assignees":{"unexpected":1},"milestone":"wrong"}),
        json!({"labels":[{"id":9,"name":"x".repeat(32768)}],"assignees":[],"milestone":null}),
        json!({"labels":[],"assignees":[],"milestone":{"id":11}}),
    ] {
        let mut got = v.clone();
        for field in ["labels", "assignees", "milestone"] {
            got.as_object_mut().unwrap().remove(field);
            if let Some(value) = optional.get(field) {
                got[field] = value.clone();
            }
        }
        let evidence =
            receipt::parse_created(201, &p, &prep, &serde_json::to_vec(&got).unwrap()).unwrap();
        assert_eq!(evidence.core.provider_id, "100");
        assert!(
            evidence
                .metadata
                .outcome(&p.request.command_id, &p.metadata)
                .unwrap()
                .needs_attention
        );
        assert!(n::encode(&evidence).unwrap().len() < 4096);
        assert!(n::receipt_matches(&evidence, &p));
    }
}
#[test]
fn core_identity_status_and_clock_ambiguity_never_become_metadata_only_attention() {
    let (p, prep, v) = receipt_fixture();
    for (status, field, bad) in [
        (202, "state", json!("open")),
        (201, "id", json!(0)),
        (
            201,
            "repository_url",
            json!("https://api.github.com/repos/other/repo"),
        ),
        (201, "title", json!("other")),
        (201, "body", json!("other")),
        (201, "user", json!({"id":8,"login":"actor"})),
        (201, "pull_request", json!({"url":"x"})),
        (201, "updated_at", json!("2026-10-07T00:00:00Z")),
    ] {
        let mut got = v.clone();
        got[field] = bad;
        assert!(
            receipt::parse_created(status, &p, &prep, &serde_json::to_vec(&got).unwrap()).is_err(),
            "{field}"
        );
    }
}
#[test]
fn receipt_two_canonical_bytes_preserve_core_and_reject_new_fields_and_fabricated_outcome() {
    let (p, prep, v) = receipt_fixture();
    let evidence =
        receipt::parse_created(201, &p, &prep, &serde_json::to_vec(&v).unwrap()).unwrap();
    let encoded = n::encode(&evidence).unwrap();
    let decoded: n::ReceiptV2 = n::decode_json(&encoded).unwrap();
    assert_eq!(evidence, decoded);
    let extra = String::from_utf8(encoded).unwrap().replace(
        "\"provider_id\":\"100\"",
        "\"provider_id\":\"100\",\"native_inbox\":null",
    );
    assert!(n::decode_json::<n::ReceiptV2>(extra.as_bytes()).is_err());
    let mut tampered = evidence;
    tampered.metadata.labels = n::SetObservationV2::Known {
        present_ids: vec!["999".into()],
    };
    assert!(!n::receipt_matches(&tampered, &p));
}
#[test]
fn actual_escaped_core_and_metadata_preparation_reserve_prevents_postreceipt_overflow() {
    let (mut p, mut prep, mut v) = receipt_fixture();
    p.request.context.account_id = "\u{1}".repeat(1024);
    p.request.context.repository_id = "\u{2}".repeat(1024);
    prep.frame.account_id = p.request.context.account_id.clone();
    prep.frame.repository_id = p.request.context.repository_id.clone();
    p.title = "\"".repeat(256);
    v["title"] = json!(p.title);
    v["user"]["login"] = json!("\\".repeat(256));
    let (mut accepted, mut refused) = (0, 16_385);
    while refused - accepted > 1 {
        let length = (accepted + refused) / 2;
        p.body = "\u{1}".repeat(length);
        if n::validate_receipt_budget(&p, &prep).is_ok() {
            accepted = length;
        } else {
            refused = length;
        }
    }
    assert!(accepted > 1000 && accepted < 16384);
    p.body = "\u{1}".repeat(accepted);
    v["body"] = json!(p.body);
    n::validate_receipt_budget(&p, &prep).unwrap();
    let evidence =
        receipt::parse_created(201, &p, &prep, &serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(n::encode(&evidence).unwrap().len() <= 65536);
    p.body.push('\u{1}');
    assert!(n::validate_receipt_budget(&p, &prep).is_err());
}
