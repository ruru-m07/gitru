use git::{
    core::get_services,
    models::commit::{Author, CommitInfo, CommitMessage, FullCommitInfo},
    AppState,
};

#[tauri::command]
pub async fn last_commit(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CommitInfo, String> {
    let services = get_services(state, &context_id).await?;
    services.commit().last_commit().await
}

#[tauri::command]
pub async fn commit_by_id(
    context_id: String,
    hash: &str,
    state: tauri::State<'_, AppState>,
) -> Result<FullCommitInfo, String> {
    let services = get_services(state, &context_id).await?;
    services.commit().commit_by_id(hash).await
}

#[tauri::command]
pub async fn create_commit(
    context_id: String,
    commit_meta: CommitMessage,
    allow_empty: bool,
    amend: bool,
    expected_head: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let services = get_services(state, &context_id).await?;
    services
        .commit()
        .create_commit(&commit_meta, allow_empty, amend, expected_head.as_deref())
        .await
}

#[tauri::command]
pub async fn commit_authors(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Author>, String> {
    let services = get_services(state, &context_id).await?;
    services.commit().commit_authors().await
}
