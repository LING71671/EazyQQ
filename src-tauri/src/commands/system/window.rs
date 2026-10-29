use crate::commands::AppState;
use crate::models::ApiResponse;
use crate::services::db::Database;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{command, Manager, State};

struct CloseGate(AtomicBool);
impl CloseGate {
    const fn new() -> Self { Self(AtomicBool::new(false)) }
    fn begin(&self) -> Option<ClosePermit<'_>> {
        self.0.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).ok()
            .map(|_| ClosePermit { gate: self, exiting: false })
    }
}
struct ClosePermit<'a> { gate: &'a CloseGate, exiting: bool }
impl Drop for ClosePermit<'_> {
    fn drop(&mut self) { if !self.exiting { self.gate.0.store(false, Ordering::Release); } }
}
static CLOSE_GATE: CloseGate = CloseGate::new();

async fn finish_window_close(
    window: tauri::Window,
    runtime_dir: std::path::PathBuf,
    close_to_tray: bool,
    mut permit: ClosePermit<'_>,
) -> Result<bool, String> {
    crate::services::ai::free_models::cancel_and_wait(&runtime_dir).await?;
    if close_to_tray {
        window.hide().map_err(|error| error.to_string())?;
        Ok(true)
    } else {
        // AppHandle::exit bypasses CloseRequested; do not recursively call Window::close.
        permit.exiting = true;
        window.app_handle().exit(0);
        Ok(false)
    }
}

/// The native event handler must only prevent close and enqueue this asynchronous work.
pub fn request_system_window_close(window: tauri::Window) {
    let Some(permit) = CLOSE_GATE.begin() else { return; };
    tauri::async_runtime::spawn(async move {
        let target = window.app_handle().try_state::<AppState>()
            .map(|state| (state.ai.runtime_dir().to_path_buf(), read_window_behavior(&state.db).1));
        let result = match target {
            Some((runtime_dir, close_to_tray)) => finish_window_close(window.clone(), runtime_dir, close_to_tray, permit).await.map(|_| ()),
            None => Err("窗口账号上下文尚未就绪，请稍后重试关闭".to_string()),
        };
        if let Err(error) = result {
            tracing::error!("Window close was kept open: {}", error);
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

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
                    .unwrap_or(false);
                let close_to_tray = win
                    .get("closeToTray")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                return (minimize_to_tray, close_to_tray);
            }
        }
    }
    (false, true)
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
#[tracing::instrument(name = "eazyqq.ipc.app_minimize_window", skip_all, err)]
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
#[tracing::instrument(name = "eazyqq.ipc.app_toggle_maximize_window", skip_all, err)]
pub async fn app_toggle_maximize_window(window: tauri::Window) -> Result<bool, String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        let _ = window.unminimize();
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.app_close_window", skip_all, err)]
pub async fn app_close_window(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let permit = CLOSE_GATE.begin().ok_or("窗口正在关闭，正在等待本次检测回收")?;
    let (_, close_to_tray) = read_window_behavior(&state.db);
    finish_window_close(window, state.ai.runtime_dir().to_path_buf(), close_to_tray, permit).await
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.app_start_drag_window", skip_all, err)]
pub async fn app_start_drag_window(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.app_show_window", skip_all, err)]
pub async fn app_show_window(app: tauri::AppHandle) -> Result<(), String> {
    show_main_window(&app);
    Ok(())
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.app_get_window_behavior", skip_all, err)]
pub async fn app_get_window_behavior(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let (minimize_to_tray, close_to_tray) = read_window_behavior(&state.db);
    Ok(ApiResponse::ok(serde_json::json!({
        "minimizeToTray": minimize_to_tray,
        "closeToTray": close_to_tray
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_api_and_native_close_requests_share_one_gate() {
        let gate = CloseGate::new();
        let first = gate.begin().unwrap();
        assert!(gate.begin().is_none(), "A second close must not launch another cancellation task");
        drop(first);
        assert!(gate.begin().is_some(), "Hide or a failed close must permit a later attempt");
    }

    #[test]
    fn an_exit_keeps_the_gate_until_the_process_finishes() {
        let gate = CloseGate::new();
        let mut permit = gate.begin().unwrap();
        permit.exiting = true;
        drop(permit);
        assert!(gate.begin().is_none(), "Scheduling app.exit must not admit another close request");
    }
}
