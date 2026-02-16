use git::models::commit::CommitInfo;
use git::models::graph::HistoryQuery;
use git::models::history::HistoryGraphResponse;
use git::service::core::get_services;
use git::AppState;

#[tauri::command]
pub async fn history(
    skip: usize,
    limit: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CommitInfo>, String> {
    let services = get_services(state).await?;
    services.history().history(skip, limit).await
}

#[tauri::command]
pub async fn history_graph(
    query: HistoryQuery,
    state: tauri::State<'_, AppState>,
) -> Result<HistoryGraphResponse, String> {
    let services = get_services(state).await?;
    services.history().history_graph(query).await
}
