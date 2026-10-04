use tauri::{command, Manager, State};
use crate::commands::AppState;
use crate::models::ApiResponse;
use crate::services::db::Database;

/// Window behavior preferences, persisted in the SQLite `app_settings.app_config` blob.
/// Project default: BOTH minimize and close collapse into the system tray, so the
/// background OneBot listener and scheduled summarizer keep running.
pub fn read_window_behavior(db: &Database) -> (bool, bool) {
    if let Ok(Some(raw)) = db.get_setting("app_config") {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(win) = parsed.get("window") {
                let minimize_to_tray = win
                    .get("minimizeToTray")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let close_to_tray = win
                    .get("closeToTray")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                return (minimize_to_tray, close_to_tray);
            }
        }
    }
    (true, true)
}

/// Restore the main window from tray / minimized state and pull it to the foreground.
pub fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[command]
pub async fn app_minimize_window(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let (minimize_to_tray, _) = read_window_behavior(&state.db);
    if minimize_to_tray {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        window.minimize().map_err(|e| e.to_string())?;
    }
    Ok(minimize_to_tray)
}

#[command]
pub async fn app_toggle_maximize_window(window: tauri::Window) -> Result<bool, String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unminimize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        let _ = window.unminimize();
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[command]
pub async fn app_close_window(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let (_, close_to_tray) = read_window_behavior(&state.db);
    if close_to_tray {
        window.hide().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        window.app_handle().exit(0);
        Ok(false)
    }
}

#[command]
pub async fn app_start_drag_window(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[command]
pub async fn app_show_window(app: tauri::AppHandle) -> Result<(), String> {
    show_main_window(&app);
    Ok(())
}

#[command]
pub async fn app_get_window_behavior(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let (minimize_to_tray, close_to_tray) = read_window_behavior(&state.db);
    Ok(ApiResponse::ok(serde_json::json!({
        "minimizeToTray": minimize_to_tray,
        "closeToTray": close_to_tray
    })))
}
