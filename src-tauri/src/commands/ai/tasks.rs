use crate::commands::AppState;
use crate::models::ApiResponse;
use serde_json::Value;
use tauri::{command, State};

#[command]
#[tracing::instrument(name = "eazyqq.ipc.manage_tasks", skip_all, err)]
pub async fn manage_tasks(
    state: State<'_, AppState>,
    request: Value,
) -> Result<ApiResponse<Value>, String> {
    crate::services::workflows::tasks::manage(
        state.db.clone(),
        state.onebot.clone(),
        state.ai.clone(),
        request,
    )
    .await
    .map(ApiResponse::ok)
}
