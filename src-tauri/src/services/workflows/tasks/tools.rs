use super::*;
use crate::services::protocol::context::{self, ConversationScope};

#[derive(Debug)]
pub(super) struct ToolResult {
    pub output: Value,
    pub sources: Vec<Value>,
    pub todos: Option<Vec<TaskTodo>>,
}

fn bounded(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

pub(super) fn execute(db: &Database, task: &Task, call: &Value) -> Result<ToolResult, String> {
    for field in [
        "targetId",
        "targetType",
        "accountUin",
        "taskId",
        "url",
        "path",
    ] {
        if call.get(field).is_some() {
            return Err("工具不能修改任务范围或接受外部路径与网址".into());
        }
    }
    let query = call["query"].as_str().unwrap_or("").trim().to_lowercase();
    if query.chars().count() > 200 {
        return Err("搜索关键词最多 200 字符".into());
    }
    match call["action"].as_str().unwrap_or("") {
        "search_history" => {
            let scope = ConversationScope {
                account_uin: task.account_uin.clone(),
                target_type: task.target_type.clone(),
                target_id: task.target_id.clone(),
            };
            let mut matches = Vec::new();
            let mut sources = Vec::new();
            for message in db
                .get_messages_by_target(&task.target_id, 100)
                .map_err(|error| error.to_string())?
            {
                let Some(source) = db
                    .get_message_source(&message.id)
                    .map_err(|error| error.to_string())?
                else {
                    continue;
                };
                if !scope.accepts(&source)
                    || (!query.is_empty() && !message.content.to_lowercase().contains(&query))
                {
                    continue;
                }
                let envelope = context::context_message(&scope, &message, Some(&source))?;
                let envelope: Value =
                    serde_json::from_str(&envelope.content).map_err(|_| "消息来源不能解析")?;
                let mut envelope = json!({"scope":scope,"messageId":message.id,"senderUin":envelope["senderUin"],"senderName":bounded(&message.sender_name,100),"memberKey":envelope["memberKey"],"relation":envelope["relation"],"timeMs":message.timestamp,"origin":envelope["origin"],"replyToMessageId":envelope["replyToMessageId"],"text":bounded(&message.content,900),"scopeVerified":true});
                // Resolved media belongs to a separate material workflow; filenames do not prove content.
                envelope["materialWarning"] =
                    json!("此工具只返回已缓存消息文字与作者来源；附件占位和文件名不是已读取的正文");
                envelope["resolutionWarning"] = Value::Null;
                sources.push(json!({"kind":"message","messageId":message.id,"accountUin":task.account_uin,"targetId":task.target_id,"targetType":task.target_type,"senderUin":message.sender_id,"timeMs":message.timestamp}));
                matches.push(envelope);
                if matches.len() >= 12 {
                    break;
                }
            }
            Ok(ToolResult {
                output: json!({"matches":matches,"searchedCachedMessages":100,"scopeVerifiedOnly":true}),
                sources,
                todos: None,
            })
        }
        "list_group_files" => {
            if task.target_type != "group" {
                return Err("群文件工具只适用于当前群任务".into());
            }
            let files = db
                .get_group_files(&task.target_id)
                .map_err(|error| error.to_string())?;
            let mut result = Vec::new();
            let mut sources = Vec::new();
            for file in files
                .into_iter()
                .filter(|file| query.is_empty() || file.file_name.to_lowercase().contains(&query))
                .take(20)
            {
                sources.push(json!({"kind":"group_file","fileId":file.file_id,"fileName":file.file_name,"groupId":task.target_id,"updatedAt":file.updated_at}));
                result.push(json!({"fileId":file.file_id,"fileName":bounded(&file.file_name,300),"fileSize":file.file_size,"uploaderName":bounded(&file.uploader_name,100),"uploadTime":file.upload_time,"updatedAt":file.updated_at}));
            }
            Ok(ToolResult {
                output: json!({"files":result,"origin":"account_group_file_cache","contentRead":false}),
                sources,
                todos: None,
            })
        }
        "set_todos" => {
            if !task.allow_todos {
                return Err("本任务未授权写入本地待办".into());
            }
            let items = call["todos"].as_array().ok_or("待办必须为数组")?;
            if items.len() > 20 {
                return Err("每个任务最多 20 项待办".into());
            }
            let mut todos = Vec::new();
            for item in items {
                let text = item["text"].as_str().unwrap_or("").trim();
                if text.is_empty() || text.chars().count() > 300 {
                    return Err("待办文本需为 1–300 字符".into());
                }
                todos.push(TaskTodo {
                    id: new_id("todo")?,
                    text: text.into(),
                    done: item["done"].as_bool().unwrap_or(false),
                });
            }
            Ok(ToolResult {
                output: json!({"todos":todos,"storage":"task_local_only","externalSync":false}),
                sources: Vec::new(),
                todos: Some(todos),
            })
        }
        _ => Err("工具不在允许列表；仅支持 search_history、list_group_files、set_todos".into()),
    }
}

fn reject_link(path: &std::path::Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "所选文件尚未下载到当前账号缓存，请先在群文件中下载")?;
    if metadata.file_type().is_symlink() {
        return Err("任务材料不能经过文件链接".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("任务材料不能经过重解析点".into());
        }
    }
    Ok(())
}

pub(super) fn read_selected_file(
    db: &Database,
    task: &Task,
    call: &Value,
    account_dir: &std::path::Path,
) -> Result<ToolResult, String> {
    for key in [
        "targetId",
        "targetType",
        "accountUin",
        "taskId",
        "url",
        "path",
    ] {
        if call.get(key).is_some() {
            return Err("文件工具不能接受其他目标、路径或网址".into());
        }
    }
    let file_id = call["fileId"].as_str().ok_or("请选择文件编号")?;
    if task.target_type != "group"
        || !task
            .selected_file_ids
            .iter()
            .any(|selected| selected == file_id)
    {
        return Err("文件未在创建此任务时明确选择，未读取正文".into());
    }
    let record = db
        .find_group_file(&task.target_id, file_id)
        .map_err(|error| error.to_string())?
        .ok_or("所选文件已不在当前群缓存")?;
    if record.download_status != "downloaded" {
        return Err("所选文件尚未下载；请先在群文件中下载后继续任务".into());
    }
    let path =
        std::path::PathBuf::from(record.local_path.as_deref().ok_or("所选文件缺少下载缓存")?);
    let group_root = account_dir.join("group_files").join(&task.target_id);
    for component in [
        account_dir.to_path_buf(),
        account_dir.join("group_files"),
        group_root.clone(),
        path.clone(),
    ] {
        reject_link(&component)?;
    }
    let owned = std::fs::canonicalize(account_dir).map_err(|_| "账号材料目录不可读取")?;
    let allowed = std::fs::canonicalize(&group_root).map_err(|_| "群材料目录不可读取")?;
    let resolved = std::fs::canonicalize(&path).map_err(|_| "群文件缓存不存在")?;
    if !allowed.starts_with(&owned) || resolved.parent() != Some(allowed.as_path()) {
        return Err("文件缓存不在当前账号与群的目录，未读取正文".into());
    }
    use std::io::Read;
    let file = std::fs::File::open(&resolved).map_err(|_| "群文件缓存不可读取")?;
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "群文件缓存读取中断")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("所选文件超过单次 8 MiB 材料上限".into());
    }
    let text = crate::services::ai::multimodal::extract(&record.file_name, &bytes)?;
    let total_chars = text.chars().count();
    let text = bounded(&text, 20_000);
    let source = json!({"kind":"group_file_content","fileId":record.file_id,"fileName":record.file_name,"groupId":task.target_id,"accountUin":task.account_uin,"updatedAt":record.updated_at});
    Ok(ToolResult {
        output: json!({"fileId":record.file_id,"fileName":bounded(&record.file_name,300),"text":text,"totalChars":total_chars,"truncated":total_chars>20_000,"bytesRead":bytes.len(),"origin":"explicitly_selected_owned_cache","contentRead":true}),
        sources: vec![source],
        todos: None,
    })
}
