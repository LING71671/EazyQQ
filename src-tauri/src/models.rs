use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorPayload>,
    pub timestamp: i64,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono_now(),
        }
    }

    pub fn err(code: i32, message: impl Into<String>, suggested_action: Option<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiErrorPayload {
                code,
                message: message.into(),
                suggested_action,
            }),
            timestamp: chrono_now(),
        }
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorPayload {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolStatusDto {
    pub is_connected: bool,
    pub login_status: String, // "unlogged" | "waiting_scan" | "scanned" | "logged_in"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qrcode_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qq_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactItemDto {
    pub id: String,
    pub target_type: String, // "friend" | "group"
    pub target_id: String,
    pub name: String,
    pub avatar_url: String,
    pub rule: RoutingRuleDto,
    pub unread_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingRuleDto {
    pub id: String,
    pub target_id: String,
    pub mode: String, // "auto_reply" | "copilot" | "summary_only" | "ignore"
    pub trigger_condition: String, // "all" | "at_me" | "keyword"
    pub keywords: Vec<String>,
    pub cooldown_seconds: i32,
    pub enabled: bool,
    #[serde(default)]
    pub is_summary_whitelist: bool,
    #[serde(default = "default_summary_interval")]
    pub summary_interval_hours: i32,
}

fn default_summary_interval() -> i32 {
    6
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageItemDto {
    pub id: String,
    pub target_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub content: String,
    pub is_from_me: bool,
    #[serde(default)]
    pub ai_reply_status: String, // "none" | "auto_replied" | "draft_pending" | "summarized"
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDraftDto {
    pub id: String,
    pub target_id: String,
    pub target_name: String,
    pub target_type: String,
    pub reply_to_msg_id: String,
    pub incoming_message_snippet: String,
    pub generated_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_content: Option<String>,
    pub model_used: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupFileItemDto {
    pub file_id: String,
    pub file_name: String,
    pub file_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    pub uploader_id: String,
    pub uploader_name: String,
    pub upload_time: i64,
    pub download_status: String, // "remote" | "downloading" | "downloaded"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummaryResultDto {
    pub file_name: String,
    pub file_size: i64,
    pub total_chars: i32,
    pub summary_text: String,
    pub key_takeaways: Vec<String>,
    pub action_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSummaryDto {
    pub id: String,
    pub target_id: String,
    pub target_name: String,
    pub summary_text: String,
    pub key_points: Vec<String>,
    pub decisions: Vec<String>,
    pub shared_files: Vec<String>,
    pub start_time: i64,
    pub end_time: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyHealthReport {
    pub is_all_ready: bool,
    pub qq_nt: ReadyPath,
    pub open_code: ReadyPath,
    pub storage: StorageReady,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyPath {
    pub ready: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageReady {
    pub is_writable: bool,
    pub free_space_mb: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub active_provider: String,
    pub model: String,
    pub port: u16,
    pub workspace_dir: String,
}
