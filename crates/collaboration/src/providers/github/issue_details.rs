//! The Issues endpoint also serves PR representations; issue authority is explicit.
use super::{resource_details::*, *};

pub(super) const ISSUE_SOURCE: &str = "github/issue-detail/2026-03-10";

pub(super) fn normalize(
    request: &DetailRequest,
    bytes: &[u8],
    source: &str,
) -> Result<Normalized, ProviderError> {
    if request.subject.kind != RemoteItemKind::Issue {
        return Err(invalid());
    }
    let json = object(bytes)?;
    if json.contains_key("pull_request") {
        return Err(invalid());
    }
    validate_identity(request, &json)?;
    let body = body(&json)?;
    let mut metadata = normalize_common(&json, RemoteItemKind::Issue, source)?;
    bound_metadata(&mut metadata)?;
    Ok((body, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DetailSubjectBinding, ErrorCode, PageCommit, Store,
        resource_metadata::{MetadataField, ResourceMetadataObservation, ResourceMetadataSnapshot},
    };
    use serde_json::{Value, json};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    const FIXTURE: &str = include_str!("../../../tests/fixtures/github_issue_detail.json");
    const PULL_FIXTURE: &str =
        include_str!("../../../tests/fixtures/github_issue_detail_pull.json");

    fn request() -> DetailRequest {
        DetailRequest {
            account: RemoteAccount {
                id: "fixture-account".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "9007199254741023".into(),
                login: "fixture-login".into(),
                display_name: None,
                authorization_epoch: "7".into(),
                state: AccountState::Active,
                notifications_supported: false,
            },
            repository: RemoteRepository {
                id: "github:repo:9007199254741021".into(),
                account_id: "fixture-account".into(),
                provider_id: "9007199254741021".into(),
                full_name: "fixture/project".into(),
                name: "project".into(),
                web_url: "https://github.com/fixture/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            subject: RemoteItem {
                id: "github:issue:9007199254740997".into(),
                account_id: "fixture-account".into(),
                repository_id: Some("github:repo:9007199254741021".into()),
                provider_id: "9007199254740997".into(),
                kind: RemoteItemKind::Issue,
                number: Some("68".into()),
                title: "An older list title".into(),
                body: Some("An independently cached summary".into()),
                body_omitted: false,
                author: Some("summary-author".into()),
                web_url: Some("https://github.com/fixture/project/issues/68".into()),
                state: "open".into(),
                updated_at: "2026-10-01T00:00:00Z".into(),
                head_oid: None,
                is_draft: None,
                reason: None,
                unread: None,
            },
            facet: DetailFacet::Body,
            cursor: None,
            etag: None,
            source: None,
        }
    }

    fn fixture() -> Value {
        serde_json::from_str(FIXTURE).expect("valid synthetic issue fixture")
    }

    fn mapped(value: &Value) -> Normalized {
        normalize(
            &request(),
            &serde_json::to_vec(value).expect("synthetic JSON"),
            ISSUE_SOURCE,
        )
        .expect("valid synthetic issue")
    }

    fn state(metadata: &ResourceMetadataObservation, field: MetadataField) -> DetailValueState {
        metadata
            .fields
            .iter()
            .find(|evidence| evidence.field == field)
            .expect("expected issue metadata field")
            .state
    }

    fn invalid_response(request: &DetailRequest, value: &Value) {
        let error = normalize(
            request,
            &serde_json::to_vec(value).expect("synthetic JSON"),
            ISSUE_SOURCE,
        )
        .expect_err("wrong issue identity or shape must fail closed");
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    }

    #[test]
    fn exact_large_ids_and_typed_metadata_do_not_promote_summary_truth() {
        let original = request();
        let (body, metadata) = mapped(&fixture());
        assert_eq!(body.state, DetailValueState::Known);
        assert!(
            body.text
                .as_deref()
                .is_some_and(|text| text.starts_with("# Cached issue"))
        );
        assert_eq!(metadata.kind, RemoteItemKind::Issue);
        assert_eq!(metadata.values.state.as_deref(), Some("closed"));
        assert_eq!(metadata.values.state_reason.as_deref(), Some("completed"));
        assert_eq!(
            metadata
                .values
                .author
                .as_ref()
                .expect("known author")
                .provider_id,
            "9007199254740999"
        );
        assert_eq!(
            metadata.values.labels[0].provider_id.as_deref(),
            Some("9007199254741001")
        );
        assert_eq!(metadata.values.labels[1].provider_id, None);
        assert_eq!(metadata.values.assignees[0].provider_id, "9007199254741003");
        assert_eq!(
            metadata
                .values
                .milestone
                .as_ref()
                .expect("known milestone")
                .provider_id,
            "9007199254741005"
        );
        assert_eq!(
            metadata
                .values
                .milestone
                .as_ref()
                .expect("known milestone")
                .number
                .as_deref(),
            Some("2")
        );
        assert_eq!(metadata.source.source, ISSUE_SOURCE);
        assert_eq!(
            metadata.source.provider_updated_at.as_deref(),
            Some("2026-10-03T12:00:00Z")
        );
        assert_eq!(metadata.fields.len(), MetadataField::COMMON.len());
        assert!(
            metadata
                .fields
                .iter()
                .all(|field| field.state == DetailValueState::Known)
        );
        assert!(
            metadata
                .fields
                .iter()
                .all(|field| !MetadataField::PULL.contains(&field.field))
        );
        assert_eq!(original.subject.state, "open");
        assert_eq!(
            original.subject.body.as_deref(),
            Some("An independently cached summary")
        );
    }

    #[test]
    fn null_empty_and_missing_preserve_distinct_field_authority() {
        for (key, field) in [
            ("user", MetadataField::Author),
            ("milestone", MetadataField::Milestone),
            ("state_reason", MetadataField::StateReason),
        ] {
            let mut missing = fixture();
            missing.as_object_mut().expect("object fixture").remove(key);
            let (_, metadata) = mapped(&missing);
            assert_eq!(state(&metadata, field), DetailValueState::Omitted);
            missing[key] = Value::Null;
            let (_, metadata) = mapped(&missing);
            assert_eq!(state(&metadata, field), DetailValueState::Known);
        }
        for (key, field) in [
            ("labels", MetadataField::Labels),
            ("assignees", MetadataField::Assignees),
        ] {
            let mut value = fixture();
            value.as_object_mut().expect("object fixture").remove(key);
            assert_eq!(state(&mapped(&value).1, field), DetailValueState::Omitted);
            value[key] = json!([]);
            let (_, metadata) = mapped(&value);
            assert_eq!(state(&metadata, field), DetailValueState::Known);
            assert!(metadata.values.labels.is_empty() == (key == "labels"));
            assert!(metadata.values.assignees.is_empty() == (key == "assignees"));
            value[key] = Value::Null;
            invalid_response(&request(), &value);
        }
        let mut value = fixture();
        value
            .as_object_mut()
            .expect("object fixture")
            .remove("body");
        assert_eq!(mapped(&value).0.state, DetailValueState::Omitted);
        value["body"] = Value::Null;
        assert_eq!(
            mapped(&value).0,
            DetailValue {
                state: DetailValueState::Known,
                text: None
            }
        );
        value["body"] = json!("");
        assert_eq!(
            mapped(&value).0,
            DetailValue {
                state: DetailValueState::Known,
                text: Some(String::new())
            }
        );
    }

    #[test]
    fn additive_states_reasons_and_oversized_fields_do_not_erase_valid_body() {
        let mut value = fixture();
        value["state"] = json!("future-state");
        value["state_reason"] = json!("future-provider-reason");
        let (_, metadata) = mapped(&value);
        assert_eq!(metadata.values.state.as_deref(), Some("future-state"));
        assert_eq!(
            metadata.values.state_reason.as_deref(),
            Some("future-provider-reason")
        );
        for (key, field, oversized) in [
            ("title", MetadataField::Title, json!("x".repeat(16_385))),
            (
                "state_reason",
                MetadataField::StateReason,
                json!("x".repeat(129)),
            ),
            (
                "user",
                MetadataField::Author,
                json!({"id": 1, "login": "x".repeat(256)}),
            ),
            ("labels", MetadataField::Labels, json!(vec!["label"; 101])),
            (
                "assignees",
                MetadataField::Assignees,
                json!(vec![json!({"id":1,"login":"fixture"}); 101]),
            ),
            (
                "milestone",
                MetadataField::Milestone,
                json!({"id":1,"title":"x".repeat(16_385)}),
            ),
        ] {
            let mut value = fixture();
            value[key] = oversized;
            let (body, metadata) = mapped(&value);
            assert_eq!(body.state, DetailValueState::Known);
            assert_eq!(state(&metadata, field), DetailValueState::Oversized);
            assert_eq!(
                state(&metadata, MetadataField::UpdatedAt),
                DetailValueState::Known
            );
        }
        let mut value = fixture();
        value["body"] = json!("x".repeat(1_048_577));
        let (body, metadata) = mapped(&value);
        assert_eq!(body.state, DetailValueState::Oversized);
        assert!(body.text.is_none());
        assert_eq!(
            metadata.values.title.as_deref(),
            Some("Keep issue metadata available after restart")
        );
    }

    #[test]
    fn aggregate_bound_discards_an_entire_collection_without_claiming_empty() {
        let mut value = fixture();
        value["labels"] = json!(vec!["l".repeat(1024); 100]);
        let url = format!("https://github.com/{}", "a".repeat(2020));
        value["assignees"] = json!(vec![
            json!({"id":1,"login":"a".repeat(255),"html_url":url});
            100
        ]);
        let (body, metadata) = mapped(&value);
        assert_eq!(body.state, DetailValueState::Known);
        assert!(
            serde_json::to_vec(&metadata.values)
                .expect("typed values")
                .len()
                <= 262_144
        );
        assert_eq!(
            state(&metadata, MetadataField::Assignees),
            DetailValueState::Oversized
        );
        assert!(metadata.values.assignees.is_empty());
        assert_eq!(
            state(&metadata, MetadataField::Labels),
            DetailValueState::Known
        );
        assert_eq!(metadata.values.labels.len(), 100);
        assert!(
            metadata
                .values
                .labels
                .iter()
                .all(|label| label.name.len() == 1024)
        );
    }

    #[test]
    fn issue_side_pull_representations_and_wrong_parent_binding_are_rejected() {
        invalid_response(
            &request(),
            &serde_json::from_str(PULL_FIXTURE).expect("synthetic pull representation"),
        );
        for flag in [Value::Null, json!({}), json!(false)] {
            let mut value = fixture();
            value["pull_request"] = flag;
            invalid_response(&request(), &value);
        }
        for (key, wrong) in [
            ("id", json!(9007199254740996u64)),
            ("number", json!(69)),
            (
                "repository_url",
                json!("https://api.github.com/repos/wrong/project"),
            ),
            (
                "repository_url",
                json!("https://api.github.com/repositories/9007199254741022"),
            ),
            (
                "repository_url",
                json!("https://api.github.com/repos/fixture/project?token=synthetic"),
            ),
            (
                "url",
                json!("https://api.github.com/repos/fixture/project/pulls/68"),
            ),
            (
                "url",
                json!("https://api.github.com/repos/fixture/project/issues/69"),
            ),
            (
                "html_url",
                json!("https://github.com/fixture/project/pull/68"),
            ),
            (
                "html_url",
                json!("https://github.com/other/project/issues/68"),
            ),
        ] {
            let mut value = fixture();
            value[key] = wrong;
            invalid_response(&request(), &value);
        }
        let mut native = fixture();
        native["repository_url"] = json!("https://api.github.com/repositories/9007199254741021");
        native["url"] = json!("https://api.github.com/repositories/9007199254741021/issues/68");
        assert_eq!(mapped(&native).0.state, DetailValueState::Known);
        for wrong in 0..6 {
            let mut request = request();
            match wrong {
                0 => request.account.host = "enterprise.fixture".into(),
                1 => request.subject.account_id = "other-account".into(),
                2 => request.repository.account_id = "other-account".into(),
                3 => request.subject.repository_id = Some("other-repository".into()),
                4 => request.subject.kind = RemoteItemKind::PullRequest,
                _ => request.cursor = Some("unexpected-continuation".into()),
            }
            invalid_response(&request, &fixture());
        }
    }

    #[test]
    fn malformed_json_ids_nested_fields_and_untrusted_web_destinations_fail_closed() {
        for (key, malformed) in [
            ("id", json!("9007199254740997")),
            ("id", json!(-1)),
            ("id", json!(0)),
            ("body", json!({"unexpected":"shape"})),
            ("user", json!({"id":"1","login":"fixture"})),
            ("user", json!({"id":1})),
            ("labels", json!([{"id":"1","name":"bad-id"}])),
            ("labels", json!([{"id":1}])),
            ("assignees", json!([null])),
            ("milestone", json!({"id":1,"title":false})),
            ("updated_at", json!("not-a-time")),
            (
                "html_url",
                json!("https://github.com.evil.invalid/fixture/project/issues/68"),
            ),
            (
                "html_url",
                json!("https://fixture@github.com/fixture/project/issues/68"),
            ),
            ("html_url", json!("javascript:synthetic")),
        ] {
            let mut value = fixture();
            value[key] = malformed;
            invalid_response(&request(), &value);
        }
        for malformed in [b"[1]".as_slice(), b"null", b"{"] {
            let error = normalize(&request(), malformed, ISSUE_SOURCE)
                .expect_err("object response required");
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        }
    }

    fn response(status: &str, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn server(
        responses: impl FnOnce(&str) -> Vec<String>,
    ) -> (GithubProvider, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener");
        listener
            .set_nonblocking(true)
            .expect("fixture nonblocking listener");
        let base = format!(
            "http://{}/",
            listener.local_addr().expect("fixture listener address")
        );
        let responses = responses(&base);
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(3);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "fixture request was not received"
                            );
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => panic!("fixture accept failed"),
                    }
                };
                stream
                    .set_nonblocking(false)
                    .expect("fixture blocking accepted stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("fixture read timeout");
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let length = stream.read(&mut chunk).expect("fixture HTTP request");
                    assert!(
                        length > 0 && bytes.len() + length <= 16_384,
                        "bounded HTTP request required"
                    );
                    bytes.extend_from_slice(&chunk[..length]);
                }
                requests.push(String::from_utf8(bytes).expect("fixture HTTP text"));
                stream
                    .write_all(response.as_bytes())
                    .expect("fixture HTTP response");
            }
            requests
        });
        (
            GithubProvider::for_test_base(reqwest::Url::parse(&base).expect("fixture base")),
            worker,
        )
    }

    fn source() -> DetailSource {
        DetailSource {
            source: ISSUE_SOURCE.into(),
            adapter_version: ADAPTER_VERSION,
            field_mask: vec![DetailField::Body],
            provider_updated_at: Some("2026-10-03T12:00:00Z".into()),
            observed_at: "2026-10-03T12:00:01Z".into(),
        }
    }

    #[tokio::test]
    async fn exact_issue_endpoint_and_matched_conditional_read_share_one_resource() {
        let (provider, worker) = server(|_| {
            vec![
                response("200 OK", "ETag: \"issue-v1\"\r\n", FIXTURE),
                response("304 Not Modified", "ETag: \"issue-v1\"\r\n", ""),
            ]
        });
        let token = SecretToken::new("fixture-token".into()).expect("synthetic token");
        assert_eq!(
            provider
                .profile(&request().account)
                .facet(ResourceFacet::IssueDetails)
                .state,
            CapabilityState::Supported
        );
        let page = provider
            .fetch_detail(&token, request())
            .await
            .expect("issue endpoint");
        assert!(page.metadata.is_some());
        assert!(page.entries.is_empty());
        assert!(page.next_cursor.is_none());
        let mut conditional = request();
        conditional.etag = page.etag;
        conditional.source = Some(page.source);
        let unchanged = provider
            .fetch_detail(&token, conditional)
            .await
            .expect("matched conditional endpoint");
        assert!(unchanged.not_modified);
        assert!(unchanged.metadata.is_none());
        let requests = worker.join().expect("fixture worker");
        assert_eq!(requests.len(), 2);
        for request in &requests {
            let lower = request.to_ascii_lowercase();
            assert!(request.starts_with("GET /repos/fixture/project/issues/68 HTTP/1.1\r\n"));
            assert!(lower.contains("x-github-api-version: 2026-03-10"));
            assert!(lower.contains("authorization: bearer fixture-token"));
            assert!(lower.contains("accept: application/vnd.github+json"));
        }
        assert!(!requests[0].to_ascii_lowercase().contains("if-none-match:"));
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("if-none-match: \"issue-v1\"")
        );
    }

    #[tokio::test]
    async fn stale_source_cannot_send_validator_or_accept_unmatched_304() {
        for stale in 0..3 {
            let (provider, worker) = server(|_| vec![response("304 Not Modified", "", "")]);
            let token = SecretToken::new("fixture-token".into()).expect("synthetic token");
            let mut request = request();
            request.etag = Some("\"old\"".into());
            let mut saved = source();
            match stale {
                0 => saved.source = "github/pull-detail/2026-03-10".into(),
                1 => saved.adapter_version += 1,
                _ => saved.field_mask.push(DetailField::Title),
            }
            request.source = Some(saved);
            let error = provider
                .request_resource_details(&token, request, ISSUE_SOURCE, normalize)
                .await
                .expect_err("unmatched304");
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            let requests = worker.join().expect("fixture worker");
            assert_eq!(requests.len(), 1);
            assert!(!requests[0].to_ascii_lowercase().contains("if-none-match:"));
        }
    }

    #[tokio::test]
    async fn native_repository_redirect_is_exact_and_never_adopts_another_scope() {
        let (provider, worker) = server(|base| {
            vec![
                response(
                    "301 Moved Permanently",
                    &format!("Location: {base}repositories/9007199254741021/issues/68\r\n"),
                    "",
                ),
                response("200 OK", "", FIXTURE),
            ]
        });
        let token = SecretToken::new("fixture-token".into()).expect("synthetic token");
        let page = provider
            .request_resource_details(&token, request(), ISSUE_SOURCE, normalize)
            .await
            .expect("same immutable repository");
        assert_eq!(page.body.state, DetailValueState::Known);
        let requests = worker.join().expect("fixture worker");
        assert_eq!(requests.len(), 2);
        assert!(requests[1].starts_with("GET /repositories/9007199254741021/issues/68 "));
        for path in [
            "repositories/9007199254741022/issues/68",
            "repositories/9007199254741021/pulls/68",
            "repositories/9007199254741021/issues/69",
            "user",
        ] {
            let (provider, worker) = server(|base| {
                vec![response(
                    "301 Moved Permanently",
                    &format!("Location: {base}{path}\r\n"),
                    "",
                )]
            });
            let error = provider
                .request_resource_details(&token, request(), ISSUE_SOURCE, normalize)
                .await
                .expect_err("untrusted redirect scope");
            assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
            assert_eq!(worker.join().expect("fixture worker").len(), 1);
        }
    }

    #[tokio::test]
    async fn safe_permission_rate_and_provider_failures_do_not_become_empty_details() {
        for (status, headers, expected) in [
            ("401 Unauthorized", "", ProviderErrorKind::Authentication),
            ("403 Forbidden", "", ProviderErrorKind::Permission),
            ("404 Not Found", "", ProviderErrorKind::NotFound),
            ("410 Gone", "", ProviderErrorKind::NotFound),
            (
                "429 Too Many Requests",
                "Retry-After: 7\r\n",
                ProviderErrorKind::RateLimited,
            ),
            (
                "503 Service Unavailable",
                "",
                ProviderErrorKind::Unavailable,
            ),
        ] {
            let (provider, worker) = server(|_| {
                vec![response(
                    status,
                    headers,
                    "{\"message\":\"synthetic private detail\"}",
                )]
            });
            let token = SecretToken::new("fixture-token".into()).expect("synthetic token");
            let error = provider
                .request_resource_details(&token, request(), ISSUE_SOURCE, normalize)
                .await
                .expect_err("provider failure");
            assert_eq!(error.kind, expected);
            if expected == ProviderErrorKind::RateLimited {
                assert_eq!(error.retry_after_seconds, Some(7));
            }
            assert!(!error.to_string().contains("synthetic private detail"));
            assert!(!error.to_string().contains("fixture-token"));
            assert_eq!(worker.join().expect("fixture worker").len(), 1);
        }
        let (provider, worker) =
            server(|_| vec![response("200 OK", "X-RateLimit-Remaining: 0\r\n", FIXTURE)]);
        let token = SecretToken::new("fixture-token".into()).expect("synthetic token");
        let page = provider
            .request_resource_details(&token, request(), ISSUE_SOURCE, normalize)
            .await
            .expect("successful rate-exhausted page");
        assert_eq!(page.body.state, DetailValueState::Known);
        assert!(page.cooldown_seconds.is_some_and(|seconds| seconds >= 1));
        assert_eq!(worker.join().expect("fixture worker").len(), 1);
    }

    async fn seed(store: &Store) -> DetailRequest {
        let mut request = request();
        request.account = store
            .upsert_account(request.account)
            .await
            .expect("synthetic account");
        for (scope, repositories, items) in [
            (
                "repositories".to_owned(),
                vec![request.repository.clone()],
                vec![],
            ),
            (
                format!("repo:{}:issue", request.repository.id),
                vec![],
                vec![request.subject.clone()],
            ),
        ] {
            let run_id = store
                .begin_sync(
                    &request.account.id,
                    &request.account.authorization_epoch,
                    &scope,
                )
                .await
                .expect("synthetic feed lease");
            store
                .apply_page(PageCommit {
                    account_id: request.account.id.clone(),
                    authorization_epoch: request.account.authorization_epoch.clone(),
                    scope,
                    run_id,
                    repositories,
                    items,
                    endpoint_aliases: vec![],
                    next_cursor: None,
                    etag: None,
                    last_modified: None,
                    not_modified: false,
                    complete: true,
                    observed_at: "2026-10-01T00:00:00Z".into(),
                })
                .await
                .expect("synthetic feed publication");
        }
        request
    }

    async fn fetched_commit(
        store: &Store,
        provider: &GithubProvider,
        token: &SecretToken,
        request: &DetailRequest,
        observed_at: &str,
    ) -> Result<DetailCommit, CollaborationError> {
        let lease = store
            .begin_detail(
                &request.account.id,
                &request.account.authorization_epoch,
                &request.subject.id,
                DetailFacet::Body,
            )
            .await?;
        let mut conditional = request.clone();
        conditional.etag = lease.etag;
        conditional.source = lease.source;
        let mut page = provider
            .request_resource_details(token, conditional, ISSUE_SOURCE, normalize)
            .await?;
        page.source.observed_at = observed_at.into();
        if let Some(metadata) = &mut page.metadata {
            metadata.source.observed_at = observed_at.into();
        }
        Ok(DetailCommit {
            reconciliation: Default::default(),
            account_id: request.account.id.clone(),
            authorization_epoch: request.account.authorization_epoch.clone(),
            authorization_view: lease.authorization_view,
            instance_id: lease.instance_id,
            subject_id: request.subject.id.clone(),
            facet: DetailFacet::Body,
            run_id: lease.run_id,
            request_cursor: lease.next_cursor,
            subject_binding: Some(DetailSubjectBinding {
                repository_id: request.repository.id.clone(),
                repository_provider_id: request.repository.provider_id.clone(),
                provider_id: request.subject.provider_id.clone(),
                number: request.subject.number.clone(),
                kind: RemoteItemKind::Issue,
                head_oid: None,
            }),
            body: page.body,
            metadata: page.metadata,
            entries: page.entries,
            source: page.source,
            next_cursor: page.next_cursor,
            etag: page.etag,
            not_modified: page.not_modified,
            whole_scope: true,
            complete: true,
            freshness_seconds: page.freshness_seconds,
        })
    }

    async fn fetch_and_commit(
        store: &Store,
        provider: &GithubProvider,
        token: &SecretToken,
        request: &DetailRequest,
        observed_at: &str,
    ) -> Result<String, CollaborationError> {
        store
            .apply_detail(fetched_commit(store, provider, token, request, observed_at).await?)
            .await
    }

    async fn read(store: &Store, request: &DetailRequest) -> DetailSnapshot {
        store
            .detail(DetailQuery {
                account_id: request.account.id.clone(),
                subject_id: request.subject.id.clone(),
                facet: DetailFacet::Body,
                cursor: None,
                limit: 100,
            })
            .await
            .expect("local issue snapshot")
    }

    fn evidence(
        metadata: &ResourceMetadataSnapshot,
        field: MetadataField,
    ) -> &crate::MetadataFieldEvidence {
        metadata
            .fields
            .iter()
            .find(|evidence| evidence.field == field)
            .expect("persisted issue field evidence")
    }

    #[tokio::test]
    async fn omitted_metadata_304_and_cold_restart_retain_known_field_clocks() {
        const T2: &str = "2026-10-03T12:00:01Z";
        const T3: &str = "2026-10-04T12:00:01Z";
        const T4: &str = "2026-10-05T12:00:01Z";
        let dir = tempfile::tempdir().expect("isolated issue cache");
        let path = dir.path().join("issue.sqlite");
        let store = Store::open(&path).await.expect("isolated native store");
        let request = seed(&store).await;
        let mut omitted = fixture();
        omitted
            .as_object_mut()
            .expect("object fixture")
            .remove("labels");
        omitted["title"] = json!("Newer authoritative issue title");
        omitted["body"] = json!("Newer authoritative Markdown");
        omitted["updated_at"] = json!("2026-10-04T12:00:00Z");
        let mut older = fixture();
        older["body"] = json!("An older response must not win");
        older["labels"] = json!(["older-label"]);
        older["updated_at"] = json!("2026-10-02T12:00:00Z");
        let omitted = serde_json::to_string(&omitted).expect("synthetic omitted response");
        let older = serde_json::to_string(&older).expect("synthetic older response");
        let (provider, worker) = server(|_| {
            vec![
                response("200 OK", "ETag: \"issue-v1\"\r\n", FIXTURE),
                response("200 OK", "ETag: \"issue-v2\"\r\n", &omitted),
                response("304 Not Modified", "ETag: \"issue-v2\"\r\n", ""),
                response("200 OK", "ETag: \"older\"\r\n", &older),
            ]
        });
        let token = SecretToken::new("fixture-token".into()).expect("synthetic token");
        fetch_and_commit(&store, &provider, &token, &request, T2)
            .await
            .expect("initial issue publication");
        fetch_and_commit(&store, &provider, &token, &request, T3)
            .await
            .expect("omitted labels publication");
        let omitted = read(&store, &request).await;
        let metadata = omitted.metadata.as_ref().expect("saved issue metadata");
        let labels = evidence(metadata, MetadataField::Labels);
        assert_eq!(labels.saved_state, DetailValueState::Known);
        assert_eq!(labels.observed_state, DetailValueState::Omitted);
        assert_eq!(labels.validated_at.as_deref(), Some(T2));
        assert_eq!(
            labels
                .source
                .as_ref()
                .expect("retained label authority")
                .provider_updated_at
                .as_deref(),
            Some("2026-10-03T12:00:00Z")
        );
        fetch_and_commit(&store, &provider, &token, &request, T4)
            .await
            .expect("conditional issue publication");
        let validated = read(&store, &request).await;
        let metadata = validated.metadata.as_ref().expect("saved issue metadata");
        assert_eq!(metadata.values.labels.len(), 2);
        assert_eq!(
            evidence(metadata, MetadataField::Labels)
                .validated_at
                .as_deref(),
            Some(T2)
        );
        assert_eq!(
            evidence(metadata, MetadataField::Title)
                .validated_at
                .as_deref(),
            Some(T4)
        );
        assert_eq!(
            validated.body.text.as_deref(),
            Some("Newer authoritative Markdown")
        );
        store.close().await;
        drop(store);
        let store = Store::open(&path).await.expect("cold native reopen");
        let reopened = read(&store, &request).await;
        assert_eq!(reopened.body, validated.body);
        assert_eq!(reopened.metadata, validated.metadata);
        let older = fetched_commit(&store, &provider, &token, &request, "2026-10-06T12:00:01Z")
            .await
            .expect("older response reaches fenced publication");
        let revision = store.revision().await.expect("dispatch revision");
        let error = store
            .apply_detail(older)
            .await
            .expect_err("older comparable response");
        assert_eq!(error.code, ErrorCode::StaleView);
        assert_eq!(
            store.revision().await.expect("unchanged revision"),
            revision
        );
        let retained = read(&store, &request).await;
        assert_eq!(retained.body, reopened.body);
        assert_eq!(retained.metadata, reopened.metadata);
        let requests = worker.join().expect("fixture worker");
        assert_eq!(requests.len(), 4);
        assert!(
            requests[2]
                .to_ascii_lowercase()
                .contains("if-none-match: \"issue-v2\"")
        );
        assert!(
            requests[3]
                .to_ascii_lowercase()
                .contains("if-none-match: \"issue-v2\"")
        );
    }
}
