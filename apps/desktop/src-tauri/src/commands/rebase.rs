use git::{
    core::get_services,
    models::{
        operation::RepoOperation,
        rebase::{
            ConflictResolveRequest, RebaseAbortPreview, RebasePlan, RebaseStartRequest,
            RebaseUpdateTodoRequest,
        },
    },
    AppState,
};

#[tauri::command]
pub async fn get_repo_operation(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<RepoOperation, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().get_repo_operation()
}

#[tauri::command]
pub async fn rebase_plan(
    context_id: String,
    onto: String,
    upstream: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<RebasePlan, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().plan_rebase(&onto, upstream.as_deref())
}

#[tauri::command]
pub async fn rebase_start(
    context_id: String,
    request: RebaseStartRequest,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<RepoOperation, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().start(request, Some(app)).await
}

#[tauri::command]
pub async fn rebase_continue(
    context_id: String,
    message: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<RepoOperation, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().continue_rebase(message, Some(app)).await
}

#[tauri::command]
pub async fn rebase_skip(
    context_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<RepoOperation, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().skip(Some(app)).await
}

#[tauri::command]
pub async fn rebase_abort(
    context_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<RepoOperation, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().abort(Some(app)).await
}

#[tauri::command]
pub async fn rebase_abort_preview(
    context_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<RebaseAbortPreview, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().abort_preview()
}

#[tauri::command]
pub async fn rebase_update_todo(
    context_id: String,
    request: RebaseUpdateTodoRequest,
    state: tauri::State<'_, AppState>,
) -> Result<RepoOperation, String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().update_todo(request.entries)
}

#[tauri::command]
pub async fn rebase_set_commit_message(
    context_id: String,
    message: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().set_commit_message(&message)
}

#[tauri::command]
pub async fn rebase_resolve_conflict(
    context_id: String,
    request: ConflictResolveRequest,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let services = get_services(state, &context_id).await?;
    services.rebase().resolve_conflict(request)
}
