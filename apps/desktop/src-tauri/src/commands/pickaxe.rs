use git::core::get_services;
use git::models::pickaxe::PickaxeQuery;
use git::service::pickaxe::PickaxeService;
use git::AppState;

#[tauri::command]
pub async fn start_pickaxe(
    context_id: String,
    query: PickaxeQuery,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let operation_id = query.operation_id.clone();
    let services = get_services(state, &context_id).await?;

    tauri::async_runtime::spawn(async move {
        let _ = services.pickaxe().start_search(query, app).await;
    });

    Ok(operation_id)
}

#[tauri::command]
pub async fn cancel_pickaxe(operation_id: String) -> Result<bool, String> {
    PickaxeService::cancel_search(&operation_id)
}