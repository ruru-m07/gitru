use git::core::get_services;
use git::models::commit::CommitInfo;
use git::models::graph::HistoryQuery;
use git::models::history::{CommitOverview, HistoryGraphResponse};
use git::AppState;

#[tauri::command]
pub async fn history(
    context_id: String,
    skip: usize,
    limit: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CommitInfo>, String> {
    let services = get_services(state, &context_id).await?;
    services.history().history(skip, limit).await
}

#[tauri::command]
pub async fn history_graph(
    context_id: String,
    query: HistoryQuery,
    state: tauri::State<'_, AppState>,
) -> Result<HistoryGraphResponse, String> {
    let services = get_services(state, &context_id).await?;
    services.history().history_graph(query).await
}

#[tauri::command]
pub async fn history_overview(
    context_id: String,
    query: HistoryQuery,
    state: tauri::State<'_, AppState>,
) -> Result<CommitOverview, String> {
    let services = get_services(state, &context_id).await?;
    services.history().history_overview(query).await
}
