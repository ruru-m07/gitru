use super::*;
use crate::NotificationSubjectObservation;
use serde_json::json;

fn repository() -> RemoteRepository {
    RemoteRepository {
        id: "local-repository".into(),
        account_id: "synthetic-account".into(),
        provider_id: "9007199254740993".into(),
        full_name: "qa-fixture/project".into(),
        name: "project".into(),
        web_url: "https://github.com/qa-fixture/project".into(),
        description: None,
        default_branch: None,
        selected: false,
    }
}

fn api() -> Url {
    Url::parse("https://api.github.com/").unwrap()
}

fn subject(kind: &str, url: &str) -> Value {
    json!({"type": kind, "url": url})
}

fn selector(mapping: Mapping) -> NotificationSubjectSelector {
    match mapping {
        Mapping::Selector(selector) => selector,
        Mapping::Fallback(_) => panic!("Synthetic subject must have validated coordinates"),
    }
}

#[test]
fn fixture_preserves_parent_id_and_number_without_turning_thread_into_subject_id() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/github_notification_subject.json"
    ))
    .unwrap();
    let mapping = normalize(&api(), &repository(), &fixture["subject"]);
    let target = selector(mapping.clone());
    assert_eq!(target.repository_provider_id, "9007199254740993");
    assert_eq!(target.number, "67");
    assert_eq!(target.kind, Kind::Issue);
    assert_eq!(target.representation, Representation::GithubIssue);
    assert_eq!(target.repository_path, "qa-fixture/project");
    let observation = NotificationSubjectObservation {
        notification_id: fixture["notification_id"].as_str().unwrap().into(),
        mapping,
    };
    assert_eq!(observation.notification_id, "777");
    let serialized = serde_json::to_value(&target).unwrap();
    assert_eq!(serialized["repository_provider_id"], "9007199254740993");
    assert_eq!(serialized["number"], "67");
    assert!(serialized.get("url").is_none());
    assert!(serialized.get("provider_id").is_none());
    assert!(serialized.get("notification_id").is_none());
}

#[test]
fn named_and_native_routes_retain_exact_ids_and_closed_representation_kinds() {
    for (kind, resource, expected, representation) in [
        ("Issue", "issues", Kind::Issue, Representation::GithubIssue),
        (
            "PullRequest",
            "pulls",
            Kind::PullRequest,
            Representation::GithubPullRequest,
        ),
    ] {
        for parent in ["repos/qa-fixture/project", "repositories/9007199254740993"] {
            for number in ["1", "9007199254740995", "18446744073709551615"] {
                let url = format!("https://api.github.com/{parent}/{resource}/{number}");
                let target = selector(normalize(&api(), &repository(), &subject(kind, &url)));
                assert_eq!(target.number, number);
                assert_eq!(target.kind, expected);
                assert_eq!(target.representation, representation);
                assert_eq!(target.kind.resource_kind(), expected.resource_kind());
                assert_eq!(target.kind.item_kind(), expected.item_kind());
            }
        }
    }
    assert!(serde_json::from_value::<Representation>(json!("github_arbitrary_http_url")).is_err());
}

#[test]
fn configured_api_origin_port_and_segment_prefix_are_exact() {
    // Configuration/parser qualification only; no enterprise adapter or HTTP test.
    let base = Url::parse("https://git.example.test:8443/api/v3/").unwrap();
    let accepted = subject(
        "PullRequest",
        "https://git.example.test:8443/api/v3/repos/qa-fixture/project/pulls/67",
    );
    assert_eq!(
        selector(normalize(&base, &repository(), &accepted)).number,
        "67"
    );
    for raw in [
        "https://git.example.test/api/v3/repos/qa-fixture/project/pulls/67",
        "https://git.example.test:8444/api/v3/repos/qa-fixture/project/pulls/67",
        "https://other.example.test:8443/api/v3/repos/qa-fixture/project/pulls/67",
        "https://git.example.test:8443/api/v30/repos/qa-fixture/project/pulls/67",
        "https://git.example.test:8443/repos/qa-fixture/project/pulls/67",
        "http://git.example.test:8443/api/v3/repos/qa-fixture/project/pulls/67",
    ] {
        assert_eq!(
            normalize(&base, &repository(), &subject("PullRequest", raw)),
            fallback(Reason::InvalidSubjectUrl)
        );
    }
    let explicit_default = subject(
        "Issue",
        "https://api.github.com:443/repos/qa-fixture/project/issues/67",
    );
    assert!(matches!(
        normalize(&api(), &repository(), &explicit_default),
        Mapping::Selector(_)
    ));
}

#[test]
fn unsafe_spelling_is_rejected_before_url_normalization_can_hide_it() {
    for raw in [
        "https://api.github.com/repos/qa-fixture/temp/../project/issues/67",
        "https://api.github.com/repos/qa-fixture/./project/issues/67",
        "https://api.github.com/repos//qa-fixture/project/issues/67",
        "https://api.github.com/repos/qa-fixture/project/issues/67/",
        "https://api.github.com/repos/qa-fixture/project/issues/%36%37",
        "https://api.github.com/repos/qa-fixture/%70roject/issues/67",
        "https://api.github.com/repos/qa-fixture/%2e%2e/project/issues/67",
        "https://api.github.com/repos/qa-fixture/project%2fissues/67",
        "https://api.github.com\\repos\\qa-fixture\\project\\issues\\67",
        "https://api.github.com/repos/qa-fixture/project/issues/6\n7",
        "https://api.github.com/repos/qa-fixture/project/issues/6\t7",
        "https://api.github.com/repos/qa-fixture/project/issues/67\u{7f}",
        " https://api.github.com/repos/qa-fixture/project/issues/67",
        "https://api.github.com/repos/qa-fixture/project/issues/67?token=fixture-secret",
        "https://api.github.com/repos/qa-fixture/project/issues/67#fragment",
        "https://fixture-secret@api.github.com/repos/qa-fixture/project/issues/67",
        "https://@api.github.com/repos/qa-fixture/project/issues/67",
        "//api.github.com/repos/qa-fixture/project/issues/67",
        "/repos/qa-fixture/project/issues/67",
        "https://api.github.com.example.test/repos/qa-fixture/project/issues/67",
    ] {
        assert_eq!(
            normalize(&api(), &repository(), &subject("Issue", raw)),
            fallback(Reason::InvalidSubjectUrl)
        );
    }
}

#[test]
fn unsupported_missing_null_and_malformed_subjects_are_per_item_fallbacks() {
    for (input, reason) in [
        (Value::Null, Reason::MissingSubjectType),
        (json!({}), Reason::MissingSubjectType),
        (json!({"type":null}), Reason::MissingSubjectType),
        (json!({"type":4}), Reason::InvalidSubjectType),
        (
            json!({"type": {"url":"ignored"}}),
            Reason::InvalidSubjectType,
        ),
        (
            json!({"type":"Release", "url":"ignored"}),
            Reason::UnsupportedSubjectType,
        ),
        (
            json!({"type":"UnknownFutureType"}),
            Reason::UnsupportedSubjectType,
        ),
        (
            json!({"type":"pullrequest"}),
            Reason::UnsupportedSubjectType,
        ),
        (json!({"type":"Issue"}), Reason::MissingSubjectUrl),
        (
            json!({"type":"Issue", "url":null}),
            Reason::MissingSubjectUrl,
        ),
        (json!({"type":"Issue", "url":4}), Reason::InvalidSubjectUrl),
        (json!({"type":"Issue", "url":""}), Reason::InvalidSubjectUrl),
        (
            json!({"type":"Issue", "latest_comment_url":"https://api.github.com/repos/qa-fixture/project/issues/comments/67"}),
            Reason::MissingSubjectUrl,
        ),
    ] {
        assert_eq!(normalize(&api(), &repository(), &input), fallback(reason));
    }
}

#[test]
fn subject_kind_cannot_silently_relabel_another_endpoint_representation() {
    for (kind, resource) in [("Issue", "pulls"), ("PullRequest", "issues")] {
        let raw = format!("https://api.github.com/repos/qa-fixture/project/{resource}/67");
        assert_eq!(
            normalize(&api(), &repository(), &subject(kind, &raw)),
            fallback(Reason::RepresentationMismatch)
        );
    }
    for tail in [
        "issues/comments/67",
        "pulls/67/comments",
        "releases/67",
        "security-advisories/67",
        "check-runs/67",
        "issues/67.json",
    ] {
        let raw = format!("https://api.github.com/repos/qa-fixture/project/{tail}");
        assert_eq!(
            normalize(&api(), &repository(), &subject("Issue", &raw)),
            fallback(Reason::InvalidSubjectUrl)
        );
    }
}

#[test]
fn captured_parent_identity_prevents_path_alias_guessing_during_rename_or_reuse() {
    let named = subject(
        "Issue",
        "https://api.github.com/repos/qa-fixture/project/issues/67",
    );
    let native = subject(
        "Issue",
        "https://api.github.com/repositories/9007199254740993/issues/67",
    );
    let mut renamed = repository();
    renamed.full_name = "qa-fixture/renamed".into();
    assert_eq!(
        normalize(&api(), &renamed, &named),
        fallback(Reason::RepositoryMismatch)
    );
    let target = selector(normalize(&api(), &renamed, &native));
    assert_eq!(target.repository_path, "qa-fixture/renamed");
    assert_eq!(target.repository_provider_id, "9007199254740993");
    let mut reused = renamed;
    reused.provider_id = "9007199254740994".into();
    assert_eq!(
        normalize(&api(), &reused, &native),
        fallback(Reason::RepositoryMismatch)
    );
    for raw in [
        "https://api.github.com/repos/another-account/project/issues/67",
        "https://api.github.com/repos/qa-fixture/other-project/issues/67",
        "https://api.github.com/repositories/9007199254740994/issues/67",
        "https://api.github.com/repositories/09007199254740993/issues/67",
    ] {
        assert_eq!(
            normalize(&api(), &repository(), &subject("Issue", raw)),
            fallback(Reason::RepositoryMismatch)
        );
    }
}

#[test]
fn inconsistent_official_notification_example_is_a_negative_fixture() {
    let mut repo = repository();
    repo.provider_id = "1296269".into();
    repo.full_name = "octocat/Hello-World".into();
    let input = subject(
        "Issue",
        "https://api.github.com/repos/octokit/octokit.rb/issues/123",
    );
    assert_eq!(
        normalize(&api(), &repo, &input),
        fallback(Reason::RepositoryMismatch)
    );
}

#[test]
fn numbers_are_exact_positive_decimal_u64_strings_with_no_coercion() {
    for number in ["0", "-1", "+1", "01", "1.0", "1e2", "18446744073709551616"] {
        let raw = format!("https://api.github.com/repos/qa-fixture/project/issues/{number}");
        assert_eq!(
            normalize(&api(), &repository(), &subject("Issue", &raw)),
            fallback(Reason::InvalidSubjectUrl)
        );
    }
}

#[test]
fn invalid_native_configuration_or_parent_is_rejected_without_url_authority() {
    let input = subject(
        "Issue",
        "https://api.github.com/repos/qa-fixture/project/issues/67",
    );
    for raw in [
        "http://api.github.com/",
        "https://fixture-secret@api.github.com/",
        "https://api.github.com/?token=fixture-secret",
        "https://api.github.com/#fragment",
        "https://api.github.com/api/v3",
        "https://api.github.com/api//v3/",
        "https://api.github.com/%61pi/v3/",
    ] {
        assert_eq!(
            normalize(&Url::parse(raw).unwrap(), &repository(), &input),
            fallback(Reason::InvalidApiConfiguration)
        );
    }
    for native in ["", "0", "01", "1.0", "18446744073709551616"] {
        let mut repo = repository();
        repo.provider_id = native.into();
        assert_eq!(
            normalize(&api(), &repo, &input),
            fallback(Reason::InvalidRepository)
        );
    }
    for path in [
        "qa-fixture",
        "qa-fixture/project/extra",
        "qa-fixture/..",
        "qa-fixture/pro%6aect",
    ] {
        let mut repo = repository();
        repo.full_name = path.into();
        assert_eq!(
            normalize(&api(), &repo, &input),
            fallback(Reason::InvalidRepository)
        );
    }
}

#[test]
fn oversized_or_credentialed_input_does_not_survive_in_serialized_fallbacks() {
    let input = subject(
        "Issue",
        &format!("https://api.github.com/{}", "fixture-secret".repeat(200)),
    );
    let mapping = normalize(&api(), &repository(), &input);
    assert_eq!(mapping, fallback(Reason::InvalidSubjectUrl));
    let serialized = serde_json::to_string(&mapping).unwrap();
    assert!(!serialized.contains("fixture-secret"));
    assert!(!serialized.contains("https://"));
    let raw = "https://fixture-secret@api.github.com/repos/qa-fixture/project/issues/67?token=fixture-secret";
    let serialized =
        serde_json::to_string(&normalize(&api(), &repository(), &subject("Issue", raw))).unwrap();
    assert!(!serialized.contains("fixture-secret"));
    assert!(!serialized.contains("api.github.com"));
}
