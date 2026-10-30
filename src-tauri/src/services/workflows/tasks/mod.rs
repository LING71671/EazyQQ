//! Fixed-scope, bounded assistant tasks. Tools never send QQ messages.

mod runtime;
mod store;
mod tools;

use crate::services::{
    ai::{AiRuntimeConfig, AiService},
    db::Database,
    onebot::OneBotClient,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) const MAX_MODEL_CALLS: usize = 6;
pub(crate) const MAX_DURATION_SECONDS: u64 = 90;
pub(crate) const MAX_TASKS: usize = 100;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSession {
    pub id: String,
    pub generation: u64,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub sequence: u64,
    pub state: String,
    pub input_revision: u64,
    #[serde(default)]
    pub persona_revision: u64,
    #[serde(default)]
    pub context_snapshot: Vec<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub model_calls: usize,
    pub max_model_calls: usize,
    pub max_duration_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStep {
    pub id: String,
    pub run_id: String,
    pub kind: String,
    pub state: String,
    pub input: Value,
    pub output: Value,
    pub sources: Vec<Value>,
    pub started_at: i64,
    pub finished_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskTodo {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub account_uin: String,
    pub target_id: String,
    pub target_type: String,
    pub title: String,
    pub instruction: String,
    pub persona: String,
    pub allow_todos: bool,
    #[serde(default)]
    pub selected_file_ids: Vec<String>,
    pub revision: u64,
    pub state: String,
    pub session: TaskSession,
    pub messages: Vec<TaskMessage>,
    pub steps: Vec<TaskStep>,
    pub todos: Vec<TaskTodo>,
    pub result: Option<String>,
    pub candidate_reply: Option<Value>,
    pub error: Option<String>,
    pub active_run: Option<TaskRun>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub(crate) fn new_id(prefix: &str) -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "无法生成任务编号".to_string())?;
    Ok(format!(
        "{prefix}_{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

pub(crate) fn fingerprint(config: &AiRuntimeConfig) -> String {
    let mut digest = Sha256::new();
    for value in [
        &config.provider,
        &config.base_url,
        &config.model,
        &config.api_key,
    ] {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn field(request: &Value, name: &str, max: usize) -> Result<String, String> {
    let value = request
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if value.chars().count() > max || value.chars().any(|c| c == '\0') {
        return Err(format!("{name} 超过长度限制或含无效字符"));
    }
    Ok(value.into())
}

pub async fn manage(
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
    request: Value,
) -> Result<Value, String> {
    let account = onebot
        .expected_account()
        .ok_or("任务需要固定的已绑定账号")?
        .to_string();
    store::initialize(&db)?;
    store::recover_expired(&db, &account, now_ms())?;
    let operation = request["operation"].as_str().ok_or("缺少任务操作")?;
    let id = field(&request, "taskId", 64)?;
    let task = match operation {
        "list" => return Ok(json!({"tasks":store::list(&db, &account)?,"task":Value::Null})),
        "get" => store::get(&db, &account, &id)?,
        "create" => {
            let target = field(&request, "targetId", 32)?;
            if !(5..=20).contains(&target.len())
                || !target.bytes().all(|b| b.is_ascii_digit())
                || target.starts_with('0')
            {
                return Err("任务目标必须为有效的 QQ 或群编号".into());
            }
            let kind = request["targetType"].as_str().unwrap_or("private");
            if !matches!(kind, "private" | "group") {
                return Err("不支持的任务会话类型".into());
            }
            let selected_file_ids = match request.get("selectedFileIds") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(values)) if values.len() <= 5 => values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .filter(|id| {
                                !id.is_empty()
                                    && id.len() <= 256
                                    && !id.chars().any(char::is_control)
                            })
                            .map(str::to_string)
                            .ok_or_else(|| "所选文件编号无效".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => return Err("每个任务最多明确选择 5 个群文件".into()),
            };
            if !selected_file_ids.is_empty() && kind != "group" {
                return Err("只有群任务可以选择群文件".into());
            }
            for file_id in &selected_file_ids {
                if db
                    .find_group_file(&target, file_id)
                    .map_err(|error| error.to_string())?
                    .is_none()
                {
                    return Err("所选文件不属于当前群缓存，请先刷新群文件".into());
                }
            }
            let instruction = field(&request, "instruction", 8_000)?;
            if instruction.is_empty() {
                return Err("请填写任务要求".into());
            }
            let title = field(&request, "title", 120)?;
            let config = ai.current();
            let task = Task {
                id: new_id("task")?,
                account_uin: account.clone(),
                target_id: target,
                target_type: kind.into(),
                title: if title.is_empty() {
                    instruction.chars().take(40).collect()
                } else {
                    title
                },
                instruction,
                persona: field(&request, "persona", 4_000)?,
                allow_todos: request["allowTodos"].as_bool().unwrap_or(false),
                selected_file_ids,
                revision: 1,
                state: "ready".into(),
                session: TaskSession {
                    id: new_id("session")?,
                    generation: 1,
                    provider: config.provider.clone(),
                    model: config.model.clone(),
                },
                messages: Vec::new(),
                steps: Vec::new(),
                todos: Vec::new(),
                result: None,
                candidate_reply: None,
                error: None,
                active_run: None,
                created_at: now_ms(),
                updated_at: now_ms(),
            };
            store::insert(&db, &task, &fingerprint(&config))?;
            task
        }
        "run" => {
            return runtime::run(db, onebot, ai, &account, &id, &request)
                .await
                .map(|task| json!({"tasks":[],"task":task}))
        }
        "cancel" => store::cancel(&db, &account, &id)?,
        "draft" => {
            let task = store::get(&db, &account, &id)?;
            if request["expectedRevision"]
                .as_u64()
                .is_some_and(|revision| revision != task.revision)
            {
                return Err("任务版本已变化，请刷新后保存草稿".into());
            }
            let draft = crate::services::protocol::reply_workflow::create_task_draft(
                &db, &onebot, &ai, &task,
            )?;
            return Ok(json!({"tasks":[],"task":task,"draftId":draft.id}));
        }
        "delete" => {
            store::delete(&db, &account, &id)?;
            let cleanup_error = runtime::cleanup_deleted_task(&ai, &id).err();
            return Ok(
                json!({"tasks":[],"task":Value::Null,"deletedId":id,"cleanupError":cleanup_error}),
            );
        }
        _ => return Err("未知任务操作".into()),
    };
    Ok(json!({"tasks":[],"task":task}))
}

#[cfg(test)]
mod tests;
