//! Explicit local command review. Provider dispatch remains native-owned.
use super::{authorize, draft_export, CollaborationState, Operation};
use collaboration::{
    CollaborationError, CommandRecoveryActionRequest, CommandRecoveryContext,
    CommandRecoveryDetail, CommandRecoveryQuery, CommandRecoveryReceipt,
    CommandRecoveryReplaceRequest, CommandRecoverySnapshot, ErrorCode,
};
use tauri::{State, Webview};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn collaboration_command_recovery_list(
    query: CommandRecoveryQuery,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommandRecoverySnapshot, CollaborationError> {
    authorize(&view, Operation::CommandRecovery)?;
    state.get().await?.command_recovery_list(query).await
}

#[tauri::command]
pub async fn collaboration_command_recovery_detail(
    account_id: String,
    command_id: String,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommandRecoveryDetail, CollaborationError> {
    authorize(&view, Operation::CommandRecovery)?;
    state
        .get()
        .await?
        .command_recovery_detail(&account_id, &command_id)
        .await
}

#[tauri::command]
pub async fn collaboration_command_recovery_action(
    request: CommandRecoveryActionRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommandRecoveryReceipt, CollaborationError> {
    authorize(&view, Operation::CommandRecovery)?;
    state.get().await?.command_recovery_action(request).await
}

#[tauri::command]
pub async fn collaboration_command_recovery_replace(
    request: CommandRecoveryReplaceRequest,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<CommandRecoveryReceipt, CollaborationError> {
    authorize(&view, Operation::CommandRecovery)?;
    state.get().await?.command_recovery_replace(request).await
}

/// The OS picker is the only destination authority. Capture the exact reviewed
/// native bundle first, so a concurrent change cannot silently replace its text.
#[tauri::command]
pub async fn collaboration_command_recovery_export(
    context: CommandRecoveryContext,
    view: Webview,
    state: State<'_, CollaborationState>,
) -> Result<bool, CollaborationError> {
    authorize(&view, Operation::CommandRecovery)?;
    let _lease = state.draft_export.try_lock().map_err(|_| {
        CollaborationError::new(ErrorCode::Busy, "An export dialog is already open")
    })?;
    let bundle = state.get().await?.command_recovery_export(context).await?;
    let (send, receive) = tokio::sync::oneshot::channel();
    view.dialog()
        .file()
        .set_parent(&view.window())
        .set_title("Export saved change")
        .set_file_name("gitru-saved-change.json")
        .add_filter("JSON", &["json"])
        .save_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive.await.map_err(|_| CollaborationError::storage())? else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|_| CollaborationError::storage())?;
    tokio::task::spawn_blocking(move || draft_export::write(&path, &bundle.text))
        .await
        .map_err(|_| CollaborationError::storage())??;
    Ok(true)
}
