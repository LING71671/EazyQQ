use tauri::{command, State};
use crate::commands::AppState;
use crate::models::{ApiResponse, GroupSummaryDto};

#[command]
pub async fn generate_summary(
    state: State<'_, AppState>,
    target_id: String,
    sliding_window_hours: Option<i32>,
    hours: Option<i32>,
) -> Result<ApiResponse<GroupSummaryDto>, String> {
    // Real summarization: reads the sliding-window message stream, filters noise and
    // asks the model for structured output. (Previously this returned a hardcoded
    // template string without reading a single message.)
    let settings = crate::services::scheduler::read_settings(&state.db);
    let window = sliding_window_hours
        .or(hours)
        .unwrap_or(settings.sliding_window_hours)
        .clamp(1, 720);

    tracing::info!("manual summary requested for {} ({}h window)", target_id, window);

    let req = crate::services::summarizer::SummaryRequest {
        target_id,
        sliding_window_hours: window,
        custom_prompt: Some(settings.custom_prompt),
        max_messages: 400,
        // A manual request should always attempt the model, even on a quiet window.
        min_messages: 1,
    };

    let outcome = crate::services::summarizer::generate(&state.db, &state.ai, &req).await?;
    Ok(ApiResponse::ok(outcome.summary))
}

#[command]
pub async fn get_summary_history(
    state: State<'_, AppState>,
    target_id: Option<String>,
) -> Result<ApiResponse<Vec<GroupSummaryDto>>, String> {
    let summaries = state.db.get_summaries(target_id.as_deref()).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(summaries))
}

#[command]
pub async fn delete_summary(
    state: State<'_, AppState>,
    id: String,
) -> Result<ApiResponse<()>, String> {
    state.db.delete_summary(&id).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(()))
}
