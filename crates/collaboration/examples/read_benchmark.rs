//! Repeatable offline read benchmark. Synthetic content never enters app data.
//! cargo run -p collaboration --example read_benchmark --release
use collaboration::*;
use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store = Store::open(dir.path().join("benchmark.sqlite3")).await?;
    let account = store
        .upsert_account(RemoteAccount {
            id: "benchmark-account".into(),
            provider: ProviderKind::Github,
            host: "github.com".into(),
            actor_id: "1".into(),
            login: "benchmark".into(),
            display_name: None,
            authorization_epoch: "1".into(),
            state: AccountState::Active,
            notifications_supported: false,
        })
        .await?;
    let repository = RemoteRepository {
        id: "benchmark-repository".into(),
        account_id: account.id.clone(),
        provider_id: "1".into(),
        full_name: "benchmark/repository".into(),
        name: "repository".into(),
        web_url: "https://github.com/benchmark/repository".into(),
        description: None,
        default_branch: Some("main".into()),
        selected: true,
    };
    let run_id = store.begin_sync(&account.id, "1", "repositories").await?;
    store
        .apply_page(PageCommit {
            account_id: account.id.clone(),
            authorization_epoch: "1".into(),
            scope: "repositories".into(),
            run_id,
            repositories: vec![repository.clone()],
            items: vec![],
            endpoint_aliases: Vec::new(),
            next_cursor: None,
            etag: None,
            last_modified: None,
            not_modified: false,
            complete: true,
            observed_at: "2026-10-02T00:00:00Z".into(),
        })
        .await?;
    let scope = format!("repo:{}:issue", repository.id);
    let run_id = store.begin_sync(&account.id, "1", &scope).await?;
    for batch in 0..100 {
        let items = (0..100)
            .map(|offset| {
                let number = batch * 100 + offset;
                RemoteItem {
                    id: format!("benchmark-item-{number:05}"),
                    account_id: account.id.clone(),
                    repository_id: Some(repository.id.clone()),
                    provider_id: number.to_string(),
                    kind: RemoteItemKind::Issue,
                    number: Some(number.to_string()),
                    title: format!("Searchable benchmark issue {number}"),
                    body: Some("Synthetic description. ".repeat(12)),
                    body_omitted: false,
                    author: Some("benchmark".into()),
                    web_url: None,
                    state: "open".into(),
                    updated_at: "2026-10-02T00:00:00Z".into(),
                    head_oid: None,
                    is_draft: None,
                    reason: None,
                    unread: None,
                }
            })
            .collect();
        store
            .apply_page(PageCommit {
                account_id: account.id.clone(),
                authorization_epoch: "1".into(),
                scope: scope.clone(),
                run_id: run_id.clone(),
                repositories: vec![],
                items,
                endpoint_aliases: Vec::new(),
                next_cursor: (batch < 99).then(|| format!("page-{}", batch + 1)),
                etag: None,
                last_modified: None,
                not_modified: false,
                complete: batch == 99,
                observed_at: "2026-10-02T00:00:00Z".into(),
            })
            .await?;
    }
    for (label, search) in [
        ("indexed list", None),
        ("FTS phrase", Some("benchmark".to_string())),
    ] {
        let mut samples = Vec::with_capacity(200);
        for _ in 0..200 {
            let start = Instant::now();
            let page = store
                .query_items(ItemQuery {
                    account_id: account.id.clone(),
                    kind: RemoteItemKind::Issue,
                    repository_id: Some(repository.id.clone()),
                    state: Some("open".into()),
                    search: search.clone(),
                    cursor: None,
                    limit: 50,
                })
                .await?;
            assert_eq!(page.items.len(), 50);
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        println!(
            "{label}: 10000 cached rows, 50-row page, 200 reads, p50={}us p95={}us",
            samples[100], samples[190]
        );
    }
    let instance = store.provider_instance(&account.id).await?;
    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let start = Instant::now();
        let resolved = store
            .resolve_resource(
                &account.id,
                ResourceLocator {
                    instance_id: instance.id.clone(),
                    kind: ResourceKind::Issue,
                    locator_kind: LocatorKind::Native,
                    value: "issue:5000".into(),
                    repository_path: None,
                },
            )
            .await?;
        assert_eq!(resolved.state, ResolutionState::Resolved);
        samples.push(start.elapsed().as_micros());
    }
    samples.sort_unstable();
    println!(
        "local identity: 10000 cached rows, 200 reads, p50={}us p95={}us",
        samples[100], samples[190]
    );
    store.close().await;
    Ok(())
}
