use super::{resource_details::*, *};
use crate::resource_metadata::*;
use serde_json::Value;

pub(super) fn normalize(
    request: &DetailRequest,
    bytes: &[u8],
    source: &str,
) -> Result<Normalized, ProviderError> {
    if request.subject.kind != RemoteItemKind::PullRequest {
        return Err(invalid());
    }
    let json = object(bytes)?;
    if json
        .get("merged")
        .is_some_and(|value| !value.is_boolean() && !value.is_null())
    {
        return Err(invalid());
    }
    validate_identity(request, &json)?;
    let body = body(&json)?;
    let mut metadata = normalize_common(&json, RemoteItemKind::PullRequest, source)?;
    let (field, value) = observe(&json, "draft", MetadataField::IsDraft, |v| {
        v.as_bool().ok_or(FieldError::Invalid)
    })?;
    metadata.fields.push(field);
    metadata.values.is_draft = value;
    for (key, field) in [("head", MetadataField::Head), ("base", MetadataField::Base)] {
        let (observed, value) = observe(&json, key, field, branch)?;
        metadata.fields.push(observed);
        if field == MetadataField::Head {
            metadata.values.head = value;
        } else {
            metadata.values.base = value;
        }
    }
    let (field, value) = observe(&json, "merged_at", MetadataField::MergedAt, |v| {
        let value = string(v, 128)?;
        if chrono::DateTime::parse_from_rfc3339(&value).is_err() {
            return Err(FieldError::Invalid);
        }
        Ok(value)
    })?;
    metadata.fields.push(field);
    metadata.values.merged_at = value;
    if json
        .get("merged")
        .is_some_and(|value| value == &Value::Bool(true))
        || metadata.values.merged_at.is_some()
    {
        metadata.values.state = Some("merged".into());
        if let Some(field) = metadata
            .fields
            .iter_mut()
            .find(|f| f.field == MetadataField::State)
        {
            field.state = DetailValueState::Known;
        }
    }
    bound_metadata(&mut metadata)?;
    Ok((body, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    fn request() -> DetailRequest {
        DetailRequest {
            account: RemoteAccount {
                id: "a".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "1".into(),
                login: "actor".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Active,
                notifications_supported: false,
            },
            repository: RemoteRepository {
                id: "repo".into(),
                account_id: "a".into(),
                provider_id: "1".into(),
                full_name: "owner/project".into(),
                name: "project".into(),
                web_url: "https://github.com/owner/project".into(),
                description: None,
                default_branch: None,
                selected: true,
            },
            subject: RemoteItem {
                id: "canonical-pull".into(),
                account_id: "a".into(),
                repository_id: Some("repo".into()),
                provider_id: "9007199254740997".into(),
                kind: RemoteItemKind::PullRequest,
                number: Some("67".into()),
                title: "summary".into(),
                body: None,
                body_omitted: true,
                author: None,
                web_url: None,
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
    fn json() -> Value {
        serde_json::json!({"id":9007199254740997_u64,"number":67,"url":"https://api.github.com/repos/owner/project/pulls/67","html_url":"https://github.com/owner/project/pull/67","title":"Full PR title","body":"# Cached Markdown\n\nDescription","state":"closed","merged":true,"merged_at":"2026-10-02T00:00:00Z","updated_at":"2026-10-02T00:00:00Z","draft":false,"user":{"id":9007199254740999_u64,"login":"author","html_url":"https://github.com/author"},"labels":[{"id":9007199254741001_u64,"name":"bug","color":"aabbcc"}],"assignees":[],"milestone":null,"head":{"ref":"feature","sha":"a".repeat(40),"repo":null},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"id":1,"full_name":"owner/project","html_url":"https://github.com/owner/project"}}})
    }
    fn map(value: Value) -> Result<Normalized, ProviderError> {
        normalize(
            &request(),
            &serde_json::to_vec(&value).unwrap(),
            PULL_SOURCE,
        )
    }

    #[test]
    fn single_response_maps_description_merged_state_and_nullable_fork_with_exact_ids() {
        let (body, metadata) = map(json()).unwrap();
        assert_eq!(body.state, DetailValueState::Known);
        assert_eq!(metadata.values.state.as_deref(), Some("merged"));
        assert_eq!(
            metadata.values.author.unwrap().provider_id,
            "9007199254740999"
        );
        assert_eq!(
            metadata.values.labels[0].provider_id.as_deref(),
            Some("9007199254741001")
        );
        let head = metadata.values.head.unwrap();
        assert_eq!(head.name, "feature");
        assert_eq!(head.oid, "a".repeat(40));
        assert!(head.repository.is_none());
        let mut value = json();
        value.as_object_mut().unwrap().remove("state");
        let (_, metadata) = map(value).unwrap();
        assert_eq!(metadata.values.state.as_deref(), Some("merged"));
        assert_eq!(
            metadata
                .fields
                .iter()
                .find(|f| f.field == MetadataField::State)
                .unwrap()
                .state,
            DetailValueState::Known
        );
        let mut value = json();
        value["merged"] = "malformed".into();
        assert_eq!(
            map(value).unwrap_err().kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    #[test]
    fn null_empty_omitted_oversized_and_additive_state_remain_distinct() {
        for value in [Value::Null, Value::String(String::new())] {
            let mut json = json();
            json["body"] = value;
            let (body, metadata) = map(json).unwrap();
            assert_eq!(body.state, DetailValueState::Known);
            assert!(metadata.values.title.is_some());
        }
        let mut value = json();
        value.as_object_mut().unwrap().remove("body");
        value.as_object_mut().unwrap().remove("user");
        value["merged"] = false.into();
        value["merged_at"] = Value::Null;
        value["state"] = "future-state".into();
        let (body, metadata) = map(value).unwrap();
        assert_eq!(body.state, DetailValueState::Omitted);
        assert_eq!(metadata.values.state.as_deref(), Some("future-state"));
        assert_eq!(
            metadata
                .fields
                .iter()
                .find(|f| f.field == MetadataField::Author)
                .unwrap()
                .state,
            DetailValueState::Omitted
        );
        let mut value = json();
        value["body"] = "x".repeat(1_048_577).into();
        value["labels"] = serde_json::json!(["x".repeat(1025)]);
        let (body, metadata) = map(value).unwrap();
        assert_eq!(body.state, DetailValueState::Oversized);
        assert_eq!(
            metadata
                .fields
                .iter()
                .find(|f| f.field == MetadataField::Labels)
                .unwrap()
                .state,
            DetailValueState::Oversized
        );
        assert!(metadata.values.title.is_some());
    }
    #[test]
    fn wrong_native_repository_number_kind_and_endpoint_paths_are_rejected() {
        for key in ["id", "number", "url", "html_url", "base"] {
            let mut value = json();
            value[key] = match key {
                "id" => 1.into(),
                "number" => 68.into(),
                "url" => "https://api.github.com/repos/owner/project/issues/67".into(),
                "html_url" => "https://github.com/owner/project/pull/68".into(),
                _ => serde_json::json!({"repo":{"id":2}}),
            };
            assert_eq!(
                map(value).unwrap_err().kind,
                ProviderErrorKind::InvalidResponse
            );
        }
        let mut request = request();
        request.subject.kind = RemoteItemKind::Issue;
        assert_eq!(
            normalize(&request, &serde_json::to_vec(&json()).unwrap(), PULL_SOURCE)
                .unwrap_err()
                .kind,
            ProviderErrorKind::InvalidResponse
        );
    }
    type RecordedRequests = std::thread::JoinHandle<Vec<(String, Option<String>)>>;

    fn server(responses: Vec<String>) -> (GithubProvider, RecordedRequests) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let provider = GithubProvider::for_test_base(
            reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap(),
        );
        let handle = std::thread::spawn(move || {
            let mut requests = vec![];
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buffer = [0; 4096];
                while !bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let text = String::from_utf8(bytes).unwrap();
                let path = text.lines().next().unwrap().to_string();
                let etag = text
                    .lines()
                    .find_map(|line| line.strip_prefix("if-none-match: ").map(String::from));
                requests.push((path, etag));
                stream.write_all(response.as_bytes()).unwrap();
            }
            requests
        });
        (provider, handle)
    }
    fn response(value: &Value) -> String {
        let body = serde_json::to_string(value).unwrap();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: W/\"literal\"\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }
    #[tokio::test]
    async fn endpoint_uses_literal_comparable_etag_and_returns_304_without_content() {
        let (provider, thread) = server(vec![
            response(&json()),
            "HTTP/1.1 304 Not Modified\r\nETag: W/\"literal\"\r\nConnection: close\r\n\r\n".into(),
        ]);
        let token = SecretToken::new("fixture".into()).unwrap();
        let first = provider.fetch_detail(&token, request()).await.unwrap();
        assert!(first.metadata.is_some());
        let mut next = request();
        next.etag = first.etag;
        next.source = Some(first.source);
        let second = provider.fetch_detail(&token, next).await.unwrap();
        assert!(second.not_modified);
        assert_eq!(second.body, DetailValue::default());
        assert!(second.metadata.is_none());
        let requests = thread.join().unwrap();
        assert!(
            requests
                .iter()
                .all(|r| r.0.starts_with("GET /repos/owner/project/pulls/67 "))
        );
        assert_eq!(requests[1].1.as_deref(), Some("W/\"literal\""));
    }
    #[tokio::test]
    async fn issue_detail_and_other_facets_never_dispatch_http() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let provider = GithubProvider::for_test_base(
            reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap(),
        );
        let token = SecretToken::new("fixture".into()).unwrap();
        let mut issue = request();
        issue.subject.kind = RemoteItemKind::Issue;
        assert_eq!(
            provider
                .profile(&issue.account)
                .facet(ResourceFacet::IssueDetails)
                .state,
            CapabilityState::Unsupported
        );
        assert_eq!(
            provider.fetch_detail(&token, issue).await.unwrap_err().kind,
            ProviderErrorKind::Unsupported
        );
        let mut comments = request();
        comments.facet = DetailFacet::Comments;
        assert_eq!(
            provider
                .fetch_detail(&token, comments)
                .await
                .unwrap_err()
                .kind,
            ProviderErrorKind::Unsupported
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[tokio::test]
    async fn detail_errors_preserve_shared_permission_and_quota_classification() {
        for (headers, kind) in [
            ("", ProviderErrorKind::Permission),
            (
                "x-ratelimit-remaining: 0\r\nretry-after: 61\r\n",
                ProviderErrorKind::RateLimited,
            ),
        ] {
            let (provider, thread) = server(vec![format!(
                "HTTP/1.1 403 Forbidden\r\n{headers}Content-Length: 0\r\nConnection: close\r\n\r\n"
            )]);
            assert_eq!(
                provider
                    .fetch_detail(&SecretToken::new("fixture".into()).unwrap(), request())
                    .await
                    .unwrap_err()
                    .kind,
                kind
            );
            thread.join().unwrap();
        }
    }
}
