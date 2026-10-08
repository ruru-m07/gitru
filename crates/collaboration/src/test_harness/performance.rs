use super::*;
use chrono::{TimeZone, Utc};

pub const PERFORMANCE_DATASET_VERSION: u32 = 1;
pub const PERFORMANCE_REPOSITORIES_PER_ACCOUNT: u32 = 5;
pub const PERFORMANCE_ITEMS_PER_ACCOUNT: usize = 5_000;
pub const PERFORMANCE_TOTAL_ITEMS: u32 = 10_000;
pub const PERFORMANCE_SEARCH_SAMPLE_COUNT: u32 = 30;

fn slot_name(slot: HarnessActorSlot) -> &'static str {
    match slot {
        HarnessActorSlot::Primary => "primary",
        HarnessActorSlot::Alternate => "alternate",
    }
}

pub fn performance_repository(
    slot: HarnessActorSlot,
    account: &RemoteAccount,
    index: u32,
) -> RemoteRepository {
    let (id, provider_id) = if index == 0 {
        (REPOSITORY_ID.to_owned(), "9007199254741993".to_owned())
    } else {
        (
            format!("github:repository:ruru125:{}:{index}", slot_name(slot)),
            format!(
                "125{}{:02}",
                if slot == HarnessActorSlot::Primary {
                    1
                } else {
                    2
                },
                index
            ),
        )
    };
    RemoteRepository {
        id,
        account_id: account.id.clone(),
        provider_id,
        full_name: format!("x-ruru125-{}/project-{index}", slot_name(slot)),
        name: format!("project-{index}"),
        web_url: format!(
            "https://github.com/x-ruru125-{}/project-{index}",
            slot_name(slot)
        ),
        description: Some("Deterministic native cached-navigation fixture".into()),
        default_branch: Some("main".into()),
        selected: true,
    }
}

pub fn performance_item(
    slot: HarnessActorSlot,
    account: &RemoteAccount,
    repository: &RemoteRepository,
    repository_index: usize,
    item_index: usize,
) -> RemoteItem {
    let global_index = repository_index * 1_000 + item_index;
    let search =
        if (4_900..4_900 + PERFORMANCE_SEARCH_SAMPLE_COUNT as usize).contains(&global_index) {
            format!(" needle{:02}", global_index - 4_900)
        } else {
            String::new()
        };
    let account_offset = if slot == HarnessActorSlot::Primary {
        0
    } else {
        10_000
    };
    let updated_at = Utc
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("valid fixture time")
        + chrono::Duration::seconds((account_offset + global_index) as i64);
    let identity = format!("ruru125:{}:{global_index:04}", slot_name(slot));
    RemoteItem {
        id: format!("github:pull:{identity}"),
        account_id: account.id.clone(),
        repository_id: Some(repository.id.clone()),
        provider_id: format!(
            "125{}{:04}",
            if slot == HarnessActorSlot::Primary {
                1
            } else {
                2
            },
            global_index
        ),
        kind: RemoteItemKind::PullRequest,
        number: Some((global_index + 1).to_string()),
        title: format!(
            "RURU-125 cached pull {global_index:04} {} repository {repository_index}{search}",
            slot_name(slot)
        ),
        body: Some(format!(
            "Deterministic cached body for {identity}; no provider access is required."
        )),
        body_omitted: false,
        author: Some(format!("ruru125-{}", slot_name(slot))),
        web_url: Some(format!("{}/pull/{}", repository.web_url, global_index + 1)),
        state: "open".into(),
        updated_at: updated_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        head_oid: Some(format!("{:040x}", account_offset + global_index + 1)),
        is_draft: Some(global_index.is_multiple_of(17)),
        reason: None,
        unread: None,
        native_inbox: None,
    }
}
