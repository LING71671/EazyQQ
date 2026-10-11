use crate::commands::AppState;
use crate::models::ApiResponse;
use tauri::{command, State};

pub use crate::services::infra::updates::AppUpdateInfo;

pub use crate::services::infra::updates::core::NapCatUpdateInfo;

#[command]
pub async fn get_update_history() -> Result<ApiResponse<Vec<crate::services::infra::updates::receipts::UpdateReceipt>>, String> {
    let root = crate::services::infra::updates::staging_root();
    let values = tauri::async_runtime::spawn_blocking(move || crate::services::infra::updates::receipts::history(&root))
        .await.map_err(|error| error.to_string())??;
    Ok(ApiResponse::ok(values))
}

#[command]
pub async fn check_app_update() -> Result<ApiResponse<AppUpdateInfo>, String> {
    Ok(ApiResponse::ok(crate::services::infra::updates::check().await?))
}

pub async fn upgrade_app_for(download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    let prepared = crate::services::infra::updates::prepare(download_url, &|_| {}).await?;
    let receipt = crate::services::infra::updates::launch(&prepared, false)?;
    Ok(ApiResponse::ok(format!("更新 {} 已交接；安装助手将在本次 CLI 退出后启动，最终结果可通过 updates history 查询。若桌面仍在运行，安装向导会提示关闭。QQ 会话保持运行。", receipt.operation_id)))
}

#[command]
pub async fn upgrade_app(app: tauri::AppHandle, download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    use tauri::Emitter;
    let reporter = |progress| { let _ = app.emit("app-update-progress", progress); };
    let prepared = crate::services::infra::updates::prepare(download_url, &reporter).await?;
    let receipt = crate::services::infra::updates::launch(&prepared, true)?;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        app.exit(0);
    });
    Ok(ApiResponse::ok(format!("更新 {} 已交接，EazyQQ 将退出并启动安装；QQ 会话保持运行。交接成功不代表安装已经完成。", receipt.operation_id)))
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
