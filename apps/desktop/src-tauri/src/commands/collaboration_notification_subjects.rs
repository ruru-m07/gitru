//! Cached subject reads and explicit finite discovery have separate commands.
use super::{
    collaboration::{authorize, CollaborationState, Operation},
    collaboration_local_links::CallerProof,
};
use collaboration::{
    CollaborationError, DiscoverNotificationSubjectRequest, NotificationSubjectQuery,
    NotificationSubjectSnapshot, RefreshReceipt,
};
use tauri::{AppHandle, State, Webview};

#[tauri::command]
pub async fn collaboration_notification_subject(
    query: NotificationSubjectQuery,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<NotificationSubjectSnapshot, CollaborationError> {
    authorize(&view, Operation::NotificationSubject)?;
    // This uses the same trusted local-domain origin class as local links,
    // retaining its native incarnation and owner allocation across startup/read.
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    caller.validate(&view, &app)?;
    let snapshot = runtime.notification_subject(query).await?;
    caller.validate(&view, &app)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn collaboration_discover_notification_subject(
    request: DiscoverNotificationSubjectRequest,
    view: Webview,
    app: AppHandle,
    state: State<'_, CollaborationState>,
) -> Result<RefreshReceipt, CollaborationError> {
    authorize(&view, Operation::DiscoverNotificationSubject)?;
    let caller = CallerProof::capture(&view, &app)?;
    let runtime = state.get().await?;
    caller.validate(&view, &app)?;
    // No URL lookup or UI-thread dispatch occurs while SQLite owns the writer.
    // Once accepted, explicit finite read intent survives closing the view.
    runtime
        .discover_notification_subject_checked(request, || caller.validate_under_writer(&app))
        .await
}
