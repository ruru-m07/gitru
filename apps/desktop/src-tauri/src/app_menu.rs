use tauri::{
    menu::{
        AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
        WINDOW_SUBMENU_ID,
    },
    AppHandle, Emitter, EventTarget, Manager,
};

const MAIN_WEBVIEW_LABEL: &str = "main";
const TAB_SHORTCUT_EVENT: &str = "gitru:tab-switch-shortcut";

const NEW_TAB_MENU_ID: &str = "gitru.new-tab";
const CLOSE_TAB_MENU_ID: &str = "gitru.close-tab";
const CLOSE_WINDOW_MENU_ID: &str = "gitru.close-window";

pub fn build(app_handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let package_info = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = AboutMetadata {
        name: Some(package_info.name.clone()),
        version: Some(package_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let app_menu = Submenu::with_items(
        app_handle,
        package_info.name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::services(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::hide(app_handle, None)?,
            &PredefinedMenuItem::hide_others(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::quit(app_handle, None)?,
        ],
    )?;

    let file_menu = Submenu::with_items(
        app_handle,
        "File",
        true,
        &[
            &MenuItem::with_id(
                app_handle,
                NEW_TAB_MENU_ID,
                "New Tab",
                true,
                Some("CmdOrCtrl+T"),
            )?,
            &MenuItem::with_id(
                app_handle,
                CLOSE_TAB_MENU_ID,
                "Close Tab",
                true,
                Some("CmdOrCtrl+W"),
            )?,
            &PredefinedMenuItem::separator(app_handle)?,
            &MenuItem::with_id(
                app_handle,
                CLOSE_WINDOW_MENU_ID,
                "Close Window",
                true,
                Some("CmdOrCtrl+Shift+W"),
            )?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app_handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app_handle, None)?,
            &PredefinedMenuItem::redo(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::cut(app_handle, None)?,
            &PredefinedMenuItem::copy(app_handle, None)?,
            &PredefinedMenuItem::paste(app_handle, None)?,
            &PredefinedMenuItem::select_all(app_handle, None)?,
        ],
    )?;

    let view_menu = Submenu::with_items(
        app_handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
    )?;

    // Keep the standard window controls, but omit Tauri's second native
    // Close Window item so Cmd+W can belong exclusively to Close Tab.
    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
        ],
    )?;

    let help_menu = Submenu::with_id_and_items(app_handle, HELP_SUBMENU_ID, "Help", true, &[])?;

    Menu::with_items(
        app_handle,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
}

pub fn handle_event(app_handle: &AppHandle, event: MenuEvent) {
    if event.id() == NEW_TAB_MENU_ID {
        emit_tab_shortcut(app_handle, "create");
    } else if event.id() == CLOSE_TAB_MENU_ID {
        emit_tab_shortcut(app_handle, "close");
    } else if event.id() == CLOSE_WINDOW_MENU_ID {
        if let Some(window) = app_handle.get_webview_window(MAIN_WEBVIEW_LABEL) {
            if let Err(error) = window.close() {
                log::error!("failed to close main window: {error}");
            }
        }
    }
}

fn emit_tab_shortcut(app_handle: &AppHandle, phase: &str) {
    if let Err(error) = app_handle.emit_to(
        EventTarget::webview(MAIN_WEBVIEW_LABEL),
        TAB_SHORTCUT_EVENT,
        serde_json::json!({ "phase": phase }),
    ) {
        log::error!("failed to emit {phase} tab shortcut: {error}");
    }
}
