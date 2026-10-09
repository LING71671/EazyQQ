use crate::commands::AppState;
use crate::models::ApiResponse;
use tauri::{command, State};

pub use crate::services::infra::updates::AppUpdateInfo;

pub use crate::services::infra::updates::core::NapCatUpdateInfo;

#[command]
pub async fn check_app_update() -> Result<ApiResponse<AppUpdateInfo>, String> {
    Ok(ApiResponse::ok(crate::services::infra::updates::check().await?))
}

pub async fn upgrade_app_for(download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    let prepared = crate::services::infra::updates::prepare(download_url, &|_| {}).await?;
    crate::services::infra::updates::launch(&prepared, false)?;
    Ok(ApiResponse::ok("安装包 SHA256 已校验，安装助手将在本次 CLI 退出后启动；若桌面仍在运行，安装向导会提示关闭。QQ 会话保持运行。".into()))
}

#[command]
pub async fn upgrade_app(app: tauri::AppHandle, download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    use tauri::Emitter;
    let reporter = |progress| { let _ = app.emit("app-update-progress", progress); };
    let prepared = crate::services::infra::updates::prepare(download_url, &reporter).await?;
    crate::services::infra::updates::launch(&prepared, true)?;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        app.exit(0);
    });
    Ok(ApiResponse::ok("安装包已校验，EazyQQ 将退出并启动安装，完成后自动打开；QQ 会话保持运行。".into()))
}

pub use crate::services::infra::updates::core::{detect_local_napcat_version, current_napcat_version};

#[command]
pub async fn get_napcat_version(state: State<'_, AppState>) -> Result<ApiResponse<String>, String> {
    Ok(ApiResponse::ok(current_napcat_version(&state.onebot).await))
}

#[command]
pub async fn check_napcat_update(state: State<'_, AppState>) -> Result<ApiResponse<NapCatUpdateInfo>, String> {
    check_napcat_update_for(&state.onebot).await
}

pub async fn check_napcat_update_for(onebot: &crate::services::onebot::OneBotClient) -> Result<ApiResponse<NapCatUpdateInfo>, String> {
    crate::services::infra::updates::core::check(onebot).await
}

pub async fn upgrade_napcat_for(download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    crate::services::infra::updates::core::upgrade(download_url, &|_| {}).await
}

#[command]
pub async fn upgrade_napcat(app: tauri::AppHandle, download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    use tauri::Emitter;
    crate::services::infra::updates::core::upgrade(download_url, &|progress| { let _ = app.emit("napcat-update-progress",progress); }).await
}
