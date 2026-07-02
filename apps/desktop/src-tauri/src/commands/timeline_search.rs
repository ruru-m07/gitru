use git::core::get_services;
use git::models::timeline_search::TimelineSearchQuery;
use git::service::timeline_search::TimelineSearchService;
use git::AppState;

#[tauri::command]
pub async fn start_timeline_search(
    context_id: String,
    query: TimelineSearchQuery,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let operation_id = query.operation_id.clone();
    let services = get_services(state, &context_id).await?;

    tauri::async_runtime::spawn(async move {
        let _ = services.timeline_search().start_search(query, app).await;
    });

    Ok(operation_id)
}

#[tauri::command]
pub async fn cancel_timeline_search(operation_id: String) -> Result<bool, String> {
    TimelineSearchService::cancel_search(&operation_id)
}