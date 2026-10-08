use crate::commands::AppState;
use crate::models::ApiResponse;
use crate::services::protocol::accounts;
use tauri::{command, State};

#[command]
pub async fn list_accounts() -> Result<ApiResponse<Vec<accounts::AccountReport>>, String> {
    Ok(ApiResponse::ok(accounts::list().await))
}

#[command]
pub async fn register_account(
    uin: String,
    nickname: Option<String>,
) -> Result<ApiResponse<accounts::AccountInfoDto>, String> {
    let instance = crate::services::instances::get_or_register_instance(&uin, nickname.as_deref())?;
    Ok(ApiResponse::ok(accounts::AccountInfoDto::from(&instance)))
}

#[command]
pub async fn batch_accounts(
    operation: String,
    uins: Vec<String>,
) -> Result<ApiResponse<Vec<accounts::BatchOutcome>>, String> {
    let source = crate::services::napcat_boot::locate_napcat_dir();
    Ok(ApiResponse::ok(
        accounts::batch(&source, &operation, &uins).await?,
    ))
}

#[command]
pub async fn account_qrcode(
    uin: String,
    refresh: Option<bool>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    crate::services::protocol::session::validate_uin(&uin)?;
    let source = crate::services::napcat_boot::locate_napcat_dir();
    let _guard = accounts::operation_lock()
        .try_lock()
        .map_err(|_| "An account operation is already in progress")?;
    let instance = crate::services::protocol::layout::resolve_instance(&uin)?;
    let (napcat, onebot) = accounts::clients(&instance);
    if !napcat.is_alive().await {
        let account = uin.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::services::protocol::boot::start_instance(&source, &account)
        })
        .await
        .map_err(|e| e.to_string())?;
        if !result.ok {
            return Err(result.detail);
        }
    }
    for _ in 0..15 {
        if crate::services::protocol::session::probe(&napcat, &onebot)
            .await
            .logged_in
        {
            return Ok(ApiResponse::ok(
                serde_json::json!({"loggedIn":true,"uin":uin}),
            ));
        }
        let qr = if refresh.unwrap_or(false) {
            napcat.refresh_qrcode().await
        } else {
            napcat.get_qrcode().await
        };
        if let Ok(qr) = qr {
            return Ok(ApiResponse::ok(
                serde_json::json!({"qrcodeBase64":qr,"uin":uin}),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Err("The account's QR code is not ready; inspect its protocol boot log".into())
}

#[command]
pub async fn configure_account(uin: String, auto_start: bool) -> Result<ApiResponse<()>, String> {
    crate::services::instances::configure_instance(&uin, auto_start)?;
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn forget_account(
    uin: String,
    state: State<'_, AppState>,
) -> Result<ApiResponse<()>, String> {
    let _ = state;
    accounts::forget(&uin)?;
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn get_account_status(
    uin: String,
) -> Result<ApiResponse<accounts::AccountReport>, String> {
    Ok(ApiResponse::ok(accounts::status(&uin).await?))
}
