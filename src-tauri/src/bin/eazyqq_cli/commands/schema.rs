use crate::args::Args;
use crate::format::print_json;
use serde_json::json;

pub fn cmd_schema(_args: &Args) -> Result<(), String> {
    let schema = json!({
        "name": "eazyqq_cli",
        "version": "0.3.0-beta",
        "description": "EazyQQ Headless Command Line Interface & AI Agent Controller",
        "commands": [
            {
                "name": "status",
                "description": "Show system and account status",
                "flags": ["--json"]
            },
            {
                "name": "health",
                "description": "Perform dependency and connection diagnostics",
                "flags": ["--json", "--deep"]
            },
            {
                "name": "selftest",
                "description": "Run offline engine self-tests (ZIP, redaction, migrations)",
                "flags": ["--json"]
            },
            {
                "name": "contacts",
                "description": "List contacts (friends and groups)",
                "flags": ["--type <friend|group>", "--json"]
            },
            {
                "name": "chat",
                "description": "Send a private or group QQ message",
                "flags": ["--target <ID>", "--message <TEXT>", "--group", "--json"]
            },
            {
                "name": "history",
                "description": "Read stored message history for a target",
                "flags": ["--target <ID>", "--limit <N>", "--json"]
            },
            {
                "name": "summary",
                "description": "Generate an AI summary for group messages",
                "flags": ["--target <ID>", "--hours <N>", "--prompt <TEXT>", "--json"]
            },
            {
                "name": "extract-file",
                "description": "Extract text from Office or text documents",
                "flags": ["--path <FILEPATH>", "--json"]
            },
            {
                "name": "rule-set",
                "description": "Configure contact or group dispatch rules",
                "flags": ["--target <ID>", "--ai-mode <on|off|auto>", "--summary-whitelist <true|false>", "--json"]
            },
            {
                "name": "mcp",
                "description": "Start Model Context Protocol (stdio) server for external AI agents",
                "flags": []
            },
            {
                "name": "schema",
                "description": "Export machine-readable JSON schema for all commands and tools",
                "flags": ["--json"]
            }
        ],
        "mcp_tools": [
            {
                "name": "send_message",
                "description": "Send a QQ message to a user or group",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_type": { "type": "string", "enum": ["private", "group"] },
                        "target_id": { "type": "string", "description": "QQ number or Group ID" },
                        "message": { "type": "string", "description": "Message text" }
                    },
                    "required": ["target_type", "target_id", "message"]
                }
            },
            {
                "name": "get_chat_history",
                "description": "Retrieve recent chat messages for a friend or group",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_id": { "type": "string", "description": "Target QQ or Group ID" },
                        "limit": { "type": "integer", "description": "Maximum number of messages (default: 20)" }
                    },
                    "required": ["target_id"]
                }
            },
            {
                "name": "list_contacts",
                "description": "List contacts with unread badges and summary whitelist states",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_type": { "type": "string", "enum": ["all", "friend", "group"] }
                    }
                }
            },
            {
                "name": "trigger_group_summary",
                "description": "Generate an AI summary for a group using sliding window and noise filters",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_id": { "type": "string", "description": "Group ID" },
                        "sliding_window_hours": { "type": "integer", "description": "Hours to look back (default: 6)" },
                        "custom_prompt": { "type": "string", "description": "Custom summary instruction" }
                    },
                    "required": ["target_id"]
                }
            },
            {
                "name": "extract_document",
                "description": "Extract text content from an Office document (.docx/.pptx/.xlsx) or text file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string", "description": "Path to local file" }
                    },
                    "required": ["file_path"]
                }
            },
            {
                "name": "simulate_rule_match",
                "description": "Simulate rule matching and AI pipeline action for an incoming message",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sender_id": { "type": "string", "description": "Sender QQ number" },
                        "target_id": { "type": "string", "description": "Friend QQ or Group ID" },
                        "content": { "type": "string", "description": "Message content" },
                        "is_group": { "type": "boolean", "description": "Whether message is in a group" }
                    },
                    "required": ["sender_id", "target_id", "content"]
                }
            },
            {
                "name": "update_target_rule",
                "description": "Update rule policy for a specific contact or group",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_id": { "type": "string", "description": "Friend QQ or Group ID" },
                        "ai_mode": { "type": "string", "enum": ["on", "off", "auto"] },
                        "is_summary_whitelist": { "type": "boolean" }
                    },
                    "required": ["target_id"]
                }
            },
            {
                "name": "get_system_health",
                "description": "Query connectivity status of NapCat, OneBot, SQLite DB, and AI endpoint",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "deep": { "type": "boolean", "description": "Whether to test AI generation latency" }
                    }
                }
            },
            {
                "name": "restart_napcat",
                "description": "Trigger restart of the NapCat protocol supervisor process",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    });

    print_json(&schema);
    Ok(())
}
