use crate::commands::AppState;
use crate::models::ApiResponse;
use serde_json::Value;
use tauri::{command, State};

#[command]
pub async fn manage_personas(
    state: State<'_, AppState>,
    request: Value,
) -> Result<ApiResponse<Value>, String> {
    if state.onebot.expected_account().is_none() {
        return Err("请先选择固定账号".into());
    }
    Ok(ApiResponse::ok(
        crate::services::ai::persona_settings::manage(&state.db, request)?,
    ))
}
