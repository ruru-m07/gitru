use super::*;
use serde_json::json;
use std::io::{Read, Write};
fn p(provider: ProviderKind) -> Payload {
    Payload {
        instance: ProviderInstance::public(provider).id,
        actor: "1".into(),
        native_id: "101".into(),
        project: "42".into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
        activity: "a".repeat(64),
        view: "1".into(),
        action: if provider == ProviderKind::Github {
            ProviderInboxAction::MarkRead
        } else {
            ProviderInboxAction::MarkDone
        },
        subject_type: if provider == ProviderKind::Github {
            "PullRequest"
        } else {
            "MergeRequest"
        }
        .into(),
        native_action: if provider == ProviderKind::Github {
            ""
        } else {
            "mentioned"
        }
        .into(),
    }
}
fn gl(done: bool) -> Value {
    json!({"id":101,"project":{"id":42},"action_name":"mentioned","target_type":"MergeRequest","state":if done{"done"}else{"pending"},"updated_at":"2026-10-07T00:00:00Z"})
}
fn server(
    provider: ProviderKind,
    status: &str,
    headers: &str,
    body: String,
) -> (InboxPolicy, std::thread::JoinHandle<String>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = Url::parse(&format!("http://{}/", l.local_addr().unwrap())).unwrap();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    );
    let task = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        s.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut b = [0u8; 8192];
        let n = s.read(&mut b).unwrap();
        let _ = s.write_all(response.as_bytes());
        String::from_utf8(b[..n].to_vec()).unwrap()
    });
    let http = if provider == ProviderKind::Github {
        InboxHttp::Github(GithubHttp::for_test_base(base).unwrap())
    } else {
        InboxHttp::Gitlab {
            client: mutation_client(&base).unwrap(),
            base,
        }
    };
    (
        InboxPolicy {
            instance: ProviderInstance::public(provider),
            http,
        },
        task,
    )
}
fn token() -> SecretToken {
    SecretToken::new("synthetic_test_token".into()).unwrap()
}
#[tokio::test]
async fn github_point_read_validates_identity_activity_and_quota() {
    let bytes=json!({"id":"101","repository":{"id":42},"subject":{"type":"PullRequest"},"unread":true,"updated_at":"2026-10-07T00:00:00Z"}).to_string();
    let (policy, task) = server(
        ProviderKind::Github,
        "200 OK",
        "X-RateLimit-Remaining: 0\r\n",
        bytes,
    );
    let (o, quota) = policy
        .observe(&token(), &p(ProviderKind::Github), false)
        .await
        .unwrap();
    assert!(!o.applied);
    assert_eq!(quota, Some(60));
    assert!(
        task.join()
            .unwrap()
            .starts_with("GET /notifications/threads/101 ")
    );
}
#[tokio::test]
async fn gitlab_pending_and_done_searches_are_explicit_separate_finite_rounds() {
    for done in [false, true] {
        let (policy, task) = server(
            ProviderKind::Gitlab,
            "200 OK",
            "",
            json!([gl(done)]).to_string(),
        );
        let (o, _) = policy
            .observe(&token(), &p(ProviderKind::Gitlab), done)
            .await
            .unwrap();
        assert_eq!(o.applied, done);
        let request = task.join().unwrap();
        assert!(request.starts_with(&format!(
            "GET /todos?project_id=42&state={}&per_page=100&page=1 ",
            if done { "done" } else { "pending" }
        )));
    }
}
#[tokio::test]
async fn gitlab_absence_duplicate_and_overflow_cannot_prove_done() {
    for values in [vec![], vec![gl(true), gl(true)], vec![gl(true); 101]] {
        let (policy, task) = server(
            ProviderKind::Gitlab,
            "200 OK",
            "Retry-After: 51\r\n",
            serde_json::to_string(&values).unwrap(),
        );
        let error = policy
            .observe(&token(), &p(ProviderKind::Gitlab), true)
            .await
            .unwrap_err();
        assert_eq!(error.account_cooldown_seconds, Some(51));
        assert!(matches!(
            error.kind,
            ProviderErrorKind::NotFound | ProviderErrorKind::InvalidResponse
        ));
        task.join().unwrap();
    }
}
#[tokio::test]
async fn gitlab_rate_denial_without_headers_retains_default_cooldown() {
    let (policy, task) = server(
        ProviderKind::Gitlab,
        "429 Too Many Requests",
        "",
        "{}".into(),
    );
    let error = policy
        .observe(&token(), &p(ProviderKind::Gitlab), false)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(error.account_cooldown_seconds, Some(60));
    assert_eq!(error.retry_after_seconds, Some(60));
    task.join().unwrap();
}
#[tokio::test]
async fn gitlab_done_mutation_uses_one_fixed_id_without_bulk_route() {
    let (policy, task) = server(ProviderKind::Gitlab, "200 OK", "", gl(true).to_string());
    let response = policy
        .mutate(&token(), &p(ProviderKind::Gitlab))
        .await
        .unwrap();
    assert_eq!(response.status, StatusCode::OK);
    assert!(
        parse_gitlab(
            &serde_json::from_slice::<Value>(&response.body).unwrap(),
            &p(ProviderKind::Gitlab)
        )
        .unwrap()
        .applied
    );
    assert!(
        task.join()
            .unwrap()
            .starts_with("POST /todos/101/mark_as_done ")
    );
}
#[test]
fn wrong_native_identity_and_source_semantics_are_rejected() {
    for key in [
        "id",
        "project",
        "target_type",
        "action_name",
        "state",
        "updated_at",
    ] {
        let mut v = gl(false);
        v[key] = Value::Null;
        assert!(parse_gitlab(&v, &p(ProviderKind::Gitlab)).is_err());
    }
    let mut gh = json!({"id":"101","repository":{"id":42},"subject":{"type":"PullRequest"},"unread":false,"updated_at":"2026-10-07T00:00:00Z"});
    assert!(
        parse_github(&serde_json::to_vec(&gh).unwrap(), &p(ProviderKind::Github))
            .unwrap()
            .applied
    );
    gh["unread"] = Value::Null;
    assert!(parse_github(&serde_json::to_vec(&gh).unwrap(), &p(ProviderKind::Github)).is_err());
}
