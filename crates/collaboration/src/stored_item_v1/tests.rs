use crate::{
    comment_send::native as comments, issue_creation::native as issues,
    workflow_state::native as workflow,
};

// Frozen bytes in RemoteItem's pre-inbox, v1 serialization order. Do not derive
// this fixture by serializing the current public projection.
const ITEM: &str = r#"{"id":"github:issue:9","account_id":"a","repository_id":"r","provider_id":"9","kind":"issue","number":"2","title":"saved","body":"authored native_inbox text","body_omitted":false,"author":"author","web_url":"https://github.com/a/r/issues/2","state":"open","updated_at":"2026-10-08T00:00:00Z","head_oid":null,"is_draft":null,"reason":null,"unread":null}"#;
const REPO: &str = r#"{"id":"r","account_id":"a","provider_id":"1","full_name":"a/r","name":"r","web_url":"https://github.com/a/r","description":null,"default_branch":null,"selected":true}"#;

fn comment_frame() -> String {
    format!(r#"{{"repository":{REPO},"subject":{ITEM},"authorization_view":"1"}}"#)
}

#[test]
fn comment_v1_frame_preparation_and_receipt_keep_exact_legacy_bytes() {
    let frame = comment_frame();
    let decoded: comments::Frame = comments::decode_json(frame.as_bytes()).unwrap();
    assert_eq!(comments::encode(&decoded).unwrap(), frame.as_bytes());
    let prep = format!(r#"{{"frame":{frame},"actor":"7","epoch":"1","command_hash":"hash"}}"#);
    let decoded: comments::Preparation = comments::decode_json(prep.as_bytes()).unwrap();
    assert_eq!(comments::encode(&decoded).unwrap(), prep.as_bytes());
    let receipt = format!(
        r#"{{"preparation":{prep},"receipt":{{"command_id":"command","draft_generation":"1","provider_id":"99","url":"https://github.com/a/r/issues/2#issuecomment-99","body":"saved","author":"author","created_at":"2026-10-08T00:00:00Z","observed_at":"2026-10-08T00:00:01Z"}}}}"#
    );
    let decoded: comments::ReceiptEvidence = comments::decode_json(receipt.as_bytes()).unwrap();
    assert_eq!(comments::encode(&decoded).unwrap(), receipt.as_bytes());
    for field in [
        r#", "extra":1"#,
        r#", "native_inbox":null"#,
        r#", "native_inbox":{"source":"notification","unread":true}"#,
        r#", "id":"duplicate""#,
    ] {
        let bad_item = format!("{}{field}}}", ITEM.trim_end_matches('}'));
        let bad = receipt.replace(ITEM, &bad_item);
        assert!(comments::decode_json::<comments::ReceiptEvidence>(bad.as_bytes()).is_err());
    }
    assert!(comments::decode_json::<comments::Frame>(format!(" {frame}").as_bytes()).is_err());
    let mut non_inbox_violation = decoded.preparation.frame.clone();
    non_inbox_violation.subject.native_inbox =
        Some(crate::NativeInboxState::Notification { unread: true });
    assert!(comments::encode(&non_inbox_violation).is_err());
    let public = serde_json::to_value(&decoded.preparation.frame.subject).unwrap();
    assert!(
        public.get("native_inbox").unwrap().is_null(),
        "public IPC still requires its nullable field"
    );
}

#[test]
fn issue_v1_creation_receipt_keeps_exact_legacy_item_and_rejects_inbox_fields() {
    let metadata = issues::ReceiptMetadata {
        kind: crate::RemoteItemKind::Issue,
        values: crate::ResourceMetadataValues::default(),
        source: crate::MetadataSource {
            source: "github/issue-detail/2026-03-10".into(),
            adapter_version: 1,
            provider_updated_at: None,
            observed_at: "2026-10-08T00:00:00Z".into(),
        },
        fields: vec![],
    };
    let metadata = serde_json::to_string(&metadata).unwrap();
    let created =
        format!(r#"{{"item":{ITEM},"metadata":{metadata},"created_at":"2026-10-08T00:00:00Z"}}"#);
    let decoded: issues::CreatedReceipt = issues::decode_json(created.as_bytes()).unwrap();
    assert_eq!(issues::encode(&decoded).unwrap(), created.as_bytes());
    let evidence = format!(
        r#"{{"preparation":{{"frame":{{"repository":{REPO},"authorization_view":"1"}},"actor":"7","epoch":"1","command_hash":"hash"}},"receipt":{created}}}"#
    );
    let decoded: issues::ReceiptEvidence = issues::decode_json(evidence.as_bytes()).unwrap();
    assert_eq!(issues::encode(&decoded).unwrap(), evidence.as_bytes());
    for field in [
        r#", "extra":1"#,
        r#", "native_inbox":null"#,
        r#", "native_inbox":{"source":"todo","completion":"done","action":"assigned","target_type":"Issue"}"#,
    ] {
        let bad_item = format!("{}{field}}}", ITEM.trim_end_matches('}'));
        assert!(
            issues::decode_json::<issues::ReceiptEvidence>(
                evidence.replace(ITEM, &bad_item).as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn workflow_v1_frame_and_evidence_keep_exact_legacy_item() {
    let frame = format!(
        r#"{{"repository":{REPO},"subject":{ITEM},"base":{{"state":"open","updated_at":"2026-10-08T00:00:00Z","head":null,"source":"github/issue-detail/2026-03-10","repository_native_id":"1","subject_native_id":"9","number":"2"}},"authorization_view":"1","body_revision":"1","run_id":"run"}}"#
    );
    let decoded: workflow::NativeFrame = workflow::decode_bounded(frame.as_bytes()).unwrap();
    assert_eq!(
        workflow::encode_bounded(&decoded).unwrap(),
        frame.as_bytes()
    );
    let evidence = format!(
        r#"{{"frame":{frame},"observation":{{"title":"saved","body":{{"state":"known","text":null}},"state":"closed","head":null,"provider_updated_at":"2026-10-08T00:00:00Z","observed_at":"2026-10-08T00:00:01Z"}},"account_id":"a","actor_id":"7","authorization_epoch":"1","command_hash":"hash","origin":"MutationResponse"}}"#
    );
    let decoded: workflow::Evidence = workflow::decode_bounded(evidence.as_bytes()).unwrap();
    assert_eq!(
        workflow::encode_bounded(&decoded).unwrap(),
        evidence.as_bytes()
    );
    let bad_item = format!(r#"{},"native_inbox":null}}"#, ITEM.trim_end_matches('}'));
    assert!(
        workflow::decode_bounded::<workflow::Evidence>(
            evidence.replace(ITEM, &bad_item).as_bytes()
        )
        .is_err()
    );
}
