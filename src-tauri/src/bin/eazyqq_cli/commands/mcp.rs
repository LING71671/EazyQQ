use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::context::Services;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

fn rpc_ok(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn rpc_err(id: Option<Value>, code: i64, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(json!({ "code": code, "message": message })),
    }
}

fn tool_text_response(id: Option<Value>, text: String, is_error: bool) -> JsonRpcResponse {
    rpc_ok(
        id,
        json!({
            "content": [
                {
                    "type": "text",
                    "text": text
                }
            ],
            "isError": is_error
        }),
    )
}

fn tool_json_response<T: Serialize>(id: Option<Value>, data: &T) -> JsonRpcResponse {
    match serde_json::to_string_pretty(data) {
        Ok(s) => tool_text_response(id, s, false),
        Err(e) => tool_text_response(id, format!("JSON serialization error: {}", e), true),
    }
}

fn mcp_tool_definitions() -> Value {
    json!([
        {
            "name": "send_message",
            "description": "Send a QQ message to a user or group via OneBot",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_type": { "type": "string", "enum": ["private", "group"] },
                    "target_id": { "type": "string", "description": "QQ number or Group ID" },
                    "message": { "type": "string", "description": "Message text to send" }
                },
                "required": ["target_type", "target_id", "message"]
            }
        },
        {
            "name": "get_chat_history",
            "description": "Retrieve recent chat messages from local SQLite store for a target",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_id": { "type": "string", "description": "QQ number or Group ID" },
                    "limit": { "type": "integer", "description": "Maximum messages to return (default: 20)" }
                },
                "required": ["target_id"]
            }
        },
        {
            "name": "list_contacts",
            "description": "List contacts (friends and groups) with unread badge count and summary whitelist flags",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_type": { "type": "string", "enum": ["all", "friend", "group"] }
                }
            }
        },
        {
            "name": "trigger_group_summary",
            "description": "Generate an AI summary for a group using sliding window and noise filters",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_id": { "type": "string", "description": "Group ID" },
                    "sliding_window_hours": { "type": "integer", "description": "Hours to look back (default: 6)" },
                    "custom_prompt": { "type": "string", "description": "Custom prompt override" }
                },
                "required": ["target_id"]
            }
        },
        {
            "name": "extract_document",
            "description": "Extract text content from an Office document (.docx/.pptx/.xlsx) or text file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to local file on disk" }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "simulate_rule_match",
            "description": "Simulate rule matching and AI pipeline action for an incoming message",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sender_id": { "type": "string", "description": "Sender QQ number" },
                    "target_id": { "type": "string", "description": "Friend QQ or Group ID" },
                    "content": { "type": "string", "description": "Message content" },
                    "is_group": { "type": "boolean", "description": "True if group message, false if private" }
                },
                "required": ["sender_id", "target_id", "content"]
            }
        },
        {
            "name": "update_target_rule",
            "description": "Update rule policy for a specific contact or group",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_id": { "type": "string", "description": "Friend QQ or Group ID" },
                    "ai_mode": { "type": "string", "enum": ["auto_reply", "copilot", "ignore"] },
                    "is_summary_whitelist": { "type": "boolean" }
                },
                "required": ["target_id"]
            }
        },
        {
            "name": "get_system_health",
            "description": "Query connectivity status of NapCat, OneBot, SQLite DB, and AI endpoint",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "deep": { "type": "boolean", "description": "Whether to test AI generation latency" }
                }
            }
        },
        {
            "name": "restart_napcat",
            "description": "Trigger restart or launch of the NapCat protocol supervisor process",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }
    ])
}

async fn handle_tool_call(svc: &Arc<Services>, id: Option<Value>, name: &str, args: &Value) -> JsonRpcResponse {
    match name {
        "send_message" => {
            let target_type = args.get("target_type").and_then(|v| v.as_str()).unwrap_or("private");
            let target_id = match args.get("target_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return tool_text_response(id, "Missing required argument 'target_id'".into(), true),
            };
            let message = match args.get("message").and_then(|v| v.as_str()) {
                Some(m) => m,
                None => return tool_text_response(id, "Missing required argument 'message'".into(), true),
            };

            let res = svc.onebot.send_msg(target_type, target_id, message).await;

            match res {
                Ok(resp) => {
                    let msg_id = resp.get("data").and_then(|d| d.get("message_id")).map(|v| v.to_string()).unwrap_or_else(|| "-".into());
                    tool_text_response(
                        id,
                        format!("Message sent successfully to {} ({}), message_id: {}", target_id, target_type, msg_id),
                        false,
                    )
                }
                Err(e) => tool_text_response(id, format!("Failed to send message: {}", e), true),
            }
        }

        "get_chat_history" => {
            let target_id = match args.get("target_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return tool_text_response(id, "Missing required argument 'target_id'".into(), true),
            };
            let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20).clamp(1, 100) as usize;

            match svc.db.get_messages_by_target(target_id, limit) {
                Ok(messages) => tool_json_response(id, &messages),
                Err(e) => tool_text_response(id, format!("Failed to read history: {}", e), true),
            }
        }

        "list_contacts" => {
            let filter_type = args.get("target_type").and_then(|v| v.as_str()).unwrap_or("all");
            let report = eazyqq_lib::services::contacts::sync_roster(&svc.db, &svc.onebot).await;
            let filtered: Vec<_> = report
                .rules
                .into_iter()
                .filter(|c| match filter_type {
                    "friend" => c.target_type == "friend",
                    "group" => c.target_type == "group",
                    _ => true,
                })
                .collect();
            tool_json_response(id, &filtered)
        }

        "trigger_group_summary" => {
            let target_id = match args.get("target_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return tool_text_response(id, "Missing required argument 'target_id'".into(), true),
            };
            let hours = args.get("sliding_window_hours").and_then(|v| v.as_i64()).unwrap_or(6).clamp(1, 720) as i32;
            let custom_prompt = args.get("custom_prompt").and_then(|v| v.as_str()).map(|s| s.to_string());

            let mut req = eazyqq_lib::services::summarizer::SummaryRequest::new(target_id, hours);
            req.custom_prompt = custom_prompt;
            req.min_messages = 1;

            match eazyqq_lib::services::summarizer::generate(&svc.db, &svc.ai, &req).await {
                Ok(outcome) => tool_text_response(
                    id,
                    format!(
                        "Summary for Group {}:\n\n{}\n\n(Scanned: {}, Usable: {})",
                        target_id, outcome.summary.summary_text, outcome.scanned, outcome.usable
                    ),
                    false,
                ),
                Err(e) => tool_text_response(id, format!("Summary generation failed: {}", e), true),
            }
        }

        "extract_document" => {
            let path_str = match args.get("file_path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return tool_text_response(id, "Missing required argument 'file_path'".into(), true),
            };
            let path = PathBuf::from(path_str);
            if !path.exists() {
                return tool_text_response(id, format!("File does not exist: {}", path_str), true);
            }

            match eazyqq_lib::services::group_files::summarize_file(&svc.ai, path_str).await {
                Ok(doc) => tool_json_response(id, &doc),
                Err(e) => tool_text_response(id, format!("Failed to extract document: {}", e), true),
            }
        }

        "simulate_rule_match" => {
            let sender_id = match args.get("sender_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return tool_text_response(id, "Missing 'sender_id'".into(), true),
            };
            let target_id = match args.get("target_id").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => return tool_text_response(id, "Missing 'target_id'".into(), true),
            };
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return tool_text_response(id, "Missing 'content'".into(), true),
            };
            let is_group = args.get("is_group").and_then(|v| v.as_bool()).unwrap_or(false);

            let event = if is_group {
                serde_json::json!({
                    "post_type": "message",
                    "message_type": "group",
                    "sub_type": "normal",
                    "group_id": target_id,
                    "user_id": sender_id,
                    "self_id": "10000",
                    "sender": { "nickname": "SimulationSender" },
                    "raw_message": content,
                    "message": content,
                })
            } else {
                serde_json::json!({
                    "post_type": "message",
                    "message_type": "private",
                    "sub_type": "friend",
                    "user_id": target_id,
                    "self_id": "10000",
                    "sender": { "nickname": "SimulationSender" },
                    "raw_message": content,
                    "message": content,
                })
            };

            let outcome = eazyqq_lib::services::ws_listener::handle_onebot_event(
                None,
                &event,
                &svc.db,
                &svc.onebot,
                &svc.ai,
                true,
            )
            .await;

            tool_json_response(id, &json!({
                "action": outcome.action,
                "summary": outcome.summary(),
                "bypassed": outcome.bypassed,
                "recorded": outcome.recorded,
                "mode": outcome.mode,
                "detail": outcome.detail
            }))
        }

        "update_target_rule" => {
            let target_id = match args.get("target_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return tool_text_response(id, "Missing required argument 'target_id'".into(), true),
            };

            let report = eazyqq_lib::services::contacts::sync_roster(&svc.db, &svc.onebot).await;
            if let Some(mut rule) = report.rules.into_iter().find(|r| r.target_id == target_id) {
                if let Some(mode) = args.get("ai_mode").and_then(|v| v.as_str()) {
                    rule.mode = mode.to_string();
                    rule.enabled = mode != "ignore";
                }
                if let Some(sw) = args.get("is_summary_whitelist").and_then(|v| v.as_bool()) {
                    rule.is_summary_whitelist = sw;
                }
                rule.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);

                match svc.db.upsert_rule(&rule) {
                    Ok(_) => tool_text_response(id, format!("Updated rule for {}: mode={}, summary_whitelist={}", target_id, rule.mode, rule.is_summary_whitelist), false),
                    Err(e) => tool_text_response(id, format!("Failed to save rule: {}", e), true),
                }
            } else {
                tool_text_response(id, format!("Target {} not found in contacts", target_id), true)
            }
        }

        "get_system_health" => {
            let napcat_ok = svc.napcat.is_alive().await;
            let onebot_ok = svc.onebot.get_login_info().await.is_ok();
            let db_ok = svc.db.get_all_rules().is_ok();
            let ai_has_key = svc.ai.has_api_key();

            let deep = args.get("deep").and_then(|v| v.as_bool()).unwrap_or(false);
            let ai_probe = if deep && ai_has_key {
                match svc.ai.generate_reply(&[], "ping", "mcp_probe").await {
                    Ok(_) => "healthy".to_string(),
                    Err(e) => format!("probe failed: {}", e),
                }
            } else {
                "not_probed".to_string()
            };

            tool_json_response(id, &json!({
                "napcat_connected": napcat_ok,
                "onebot_http_ok": onebot_ok,
                "database_ok": db_ok,
                "ai_key_configured": ai_has_key,
                "active_ai_provider": svc.ai.current().provider,
                "active_model": svc.ai.model(),
                "deep_ai_status": ai_probe
            }))
        }

        "restart_napcat" => {
            match svc.napcat.launch_if_needed() {
                Ok(_) => tool_text_response(id, "NapCat supervisor launch triggered".into(), false),
                Err(e) => tool_text_response(id, format!("NapCat launch error: {}", e), true),
            }
        }

        _ => tool_text_response(id, format!("Unknown tool '{}'", name), true),
    }
}

pub async fn run_mcp_server(svc: Arc<Services>) -> Result<(), String> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = rpc_err(None, -32700, &format!("Parse error: {}", e));
                if let Ok(serialized) = serde_json::to_string(&resp) {
                    let _ = stdout.write_all(format!("{}\n", serialized).as_bytes()).await;
                    let _ = stdout.flush().await;
                }
                continue;
            }
        };

        let response = match req.method.as_str() {
            "initialize" => rpc_ok(
                req.id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "eazyqq-mcp",
                        "version": "0.3.0-beta"
                    }
                }),
            ),

            "notifications/initialized" => {
                // MCP client acknowledgment notification, no response required
                continue;
            }

            "ping" => rpc_ok(req.id, json!({})),

            "tools/list" => rpc_ok(
                req.id,
                json!({
                    "tools": mcp_tool_definitions()
                }),
            ),

            "tools/call" => {
                let tool_name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let empty_args = json!({});
                let tool_args = req.params.get("arguments").unwrap_or(&empty_args);
                handle_tool_call(&svc, req.id, tool_name, tool_args).await
            }

            _ => rpc_err(req.id, -32601, &format!("Method not found: {}", req.method)),
        };

        if let Ok(serialized) = serde_json::to_string(&response) {
            let _ = stdout.write_all(format!("{}\n", serialized).as_bytes()).await;
            let _ = stdout.flush().await;
        }
    }

    Ok(())
}
