#![allow(dead_code)]
use collaboration::*;

pub fn account(id: &str) -> RemoteAccount {
    RemoteAccount {
        id: id.into(),
        provider: ProviderKind::Github,
        host: "github.com".into(),
        actor_id: id.into(),
        login: id.into(),
        display_name: None,
        authorization_epoch: "1".into(),
        state: AccountState::Active,
        notifications_supported: false,
    }
}
pub async fn seed(store: &Store, id: &str) -> RemoteAccount {
    let account = store.upsert_account(account(id)).await.unwrap();
    project(store, &account).await;
    account
}
pub async fn project(store: &Store, account: &RemoteAccount) {
    let id = account.id.as_str();
    let repository = RemoteRepository {
        id: "repo".into(),
        account_id: id.into(),
        provider_id: "1".into(),
        full_name: "owner/project".into(),
        name: "project".into(),
        web_url: "https://github.com/owner/project".into(),
        description: None,
        default_branch: None,
        selected: true,
    };
    let item = RemoteItem {
        id: "pull".into(),
        account_id: id.into(),
        repository_id: Some(repository.id.clone()),
        provider_id: "9007199254740997".into(),
        kind: RemoteItemKind::PullRequest,
        number: Some("67".into()),
        title: "Summary".into(),
        body: Some("summary body has no detail authority".into()),
        body_omitted: false,
        author: None,
        web_url: None,
        state: "open".into(),
        updated_at: "2026-10-03T00:00:00Z".into(),
        head_oid: None,
        is_draft: None,
        reason: None,
        unread: None,
    };
    for (scope, repositories, items) in [
        ("repositories", vec![repository], vec![]),
        ("repo:repo:pull_request", vec![], vec![item]),
    ] {
        let run_id = store
            .begin_sync(id, &account.authorization_epoch, scope)
            .await
            .unwrap();
        store
            .apply_page(PageCommit {
                account_id: id.into(),
                authorization_epoch: account.authorization_epoch.clone(),
                scope: scope.into(),
                run_id,
                repositories,
                items,
                endpoint_aliases: vec![],
                next_cursor: None,
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: true,
                observed_at: "2026-10-03T00:00:00Z".into(),
            })
            .await
            .unwrap();
    }
}
pub fn source(facet: DetailFacet) -> DetailSource {
    DetailSource {
        source: format!("fixture/{}/v1", facet.name()),
        adapter_version: 1,
        field_mask: if facet == DetailFacet::Body {
            vec![DetailField::Body]
        } else {
            vec![]
        },
        provider_updated_at: None,
        observed_at: "2026-10-03T00:00:00Z".into(),
    }
}
pub fn known(text: Option<&str>) -> DetailValue {
    DetailValue {
        state: DetailValueState::Known,
        text: text.map(String::from),
    }
}
pub fn entry(id: &str) -> DetailEntry {
    DetailEntry {
        id: id.into(),
        provider_id: id.into(),
        author: Some("actor".into()),
        title: None,
        state: None,
        body: known(Some("saved comment")),
        observed_body_state: DetailValueState::NotLoaded,
        updated_at: None,
        head_oid: None,
        field_mask: vec![DetailField::Author, DetailField::Body],
        field_validations: vec![],
        native: None,
    }
}
pub async fn commit(store: &Store, account: &RemoteAccount, facet: DetailFacet) -> DetailCommit {
    let lease = store
        .begin_detail(&account.id, &account.authorization_epoch, "pull", facet)
        .await
        .unwrap();
    from_lease(account, facet, lease)
}
pub fn from_lease(account: &RemoteAccount, facet: DetailFacet, lease: DetailLease) -> DetailCommit {
    DetailCommit {
        reconciliation: DetailReconciliation::full_history(),
        metadata: None,
        subject_binding: None,
        check_context: None,
        review_context: None,
        account_id: account.id.clone(),
        authorization_epoch: account.authorization_epoch.clone(),
        authorization_view: lease.authorization_view,
        instance_id: lease.instance_id,
        subject_id: "pull".into(),
        facet,
        run_id: lease.run_id,
        request_cursor: lease.next_cursor,
        body: if facet == DetailFacet::Body {
            known(Some("saved authoritative body"))
        } else {
            DetailValue::default()
        },
        entries: vec![],
        source: source(facet),
        next_cursor: None,
        etag: Some("whole-facet-v1".into()),
        not_modified: false,
        whole_scope: true,
        complete: true,
        freshness_seconds: 60,
    }
}
pub fn query(account: &str, facet: DetailFacet) -> DetailQuery {
    DetailQuery {
        account_id: account.into(),
        subject_id: "pull".into(),
        facet,
        cursor: None,
        limit: 100,
    }
}
