# EazyQQ API & Contract Specification

## 1. Unified Response Envelope

All Tauri commands and internal API results return the standard response envelope:

```typescript
export interface ApiResponse<T = void> {
  success: boolean;
  data?: T;
  error?: ApiErrorPayload;
  timestamp: number;
}

export interface ApiErrorPayload {
  code: number;
  message: string;
  details?: Record<string, unknown>;
  suggestedAction?: string;
}
```

### Standard Error Code Ranges
- `1000 - 1999`: Environment, Dependencies & Storage errors (`ERR_NTQQ_NOT_FOUND`, `ERR_STORAGE_READONLY`).
- `2000 - 2999`: Protocol & Authentication errors (`ERR_PROTOCOL_DISCONNECTED`, `ERR_QRCODE_EXPIRED`).
- `3000 - 3999`: AI Inference & LLM Provider errors (`ERR_AI_SESSION_TIMEOUT`, `ERR_AI_STREAM_ABORTED`).
- `4000 - 4999`: Group File & Document Processing errors (`ERR_FILE_NOT_FOUND`, `ERR_DOC_PARSE_FAILED`).
- `5000 - 5999`: Routing Rule & Draft errors (`ERR_RULE_VALIDATION_FAILED`, `ERR_DRAFT_NOT_FOUND`).

---

## 2. Tauri IPC Commands

Commands are invoked from the frontend using `@tauri-apps/api/core::invoke(cmd, args)`.

### 2.1 Protocol & Authentication
| Command | Arguments | Return Type | Description |
| :--- | :--- | :--- | :--- |
| `get_protocol_status` | None | `ProtocolStatusDto` | Returns connection state, QR code Base64, and login account info |
| `refresh_qrcode` | None | `{ qrcodeBase64: string; expiresInSeconds: number }` | Refreshes and returns an active NTQQ login QR code |
| `quick_login` | `{ uin: string }` | `void` | Authenticates using cached session credentials |
| `logout` | None | `void` | Disconnects current session and revokes tokens |

### 2.2 Contacts & Routing Rules
| Command | Arguments | Return Type | Description |
| :--- | :--- | :--- | :--- |
| `get_contacts` | None | `ContactItemDto[]` | Retrieves all friends and groups with active rules |
| `update_routing_rule` | `{ rule: RoutingRuleDto }` | `RoutingRuleDto` | Updates mode, triggers, prompt, or cooldown for a target |
| `toggle_summary_whitelist` | `{ targetId: string; enabled: boolean }` | `RoutingRuleDto` | Toggles target inclusion in periodic group summary |
| `simulate_rule_evaluation` | `{ targetId: string; content: string }` | `RuleEvaluationResultDto` | Simulates rule matching without executing actions |

### 2.3 Messages & Drafts
| Command | Arguments | Return Type | Description |
| :--- | :--- | :--- | :--- |
| `get_chat_history` | `{ targetId: string; limit?: number }` | `MessageItemDto[]` | Fetches chronological chat history for a target |
| `send_message` | `{ targetId: string; content: string }` | `void` | Sends message to a group or friend |
| `get_pending_drafts` | None | `PendingDraftDto[]` | Retrieves all pending AI reply drafts |
| `send_draft` | `{ draftId: string; finalContent?: string }` | `void` | Approves and dispatches draft to target |
| `dismiss_draft` | `{ draftId: string }` | `void` | Rejects and discards draft |
| `regenerate_draft` | `{ draftId: string; promptCorrection?: string }` | `PendingDraftDto` | Re-prompts AI to regenerate draft with feedback |

### 2.4 Summaries & File Intelligence
| Command | Arguments | Return Type | Description |
| :--- | :--- | :--- | :--- |
| `generate_summary` | `{ targetId: string; slidingWindowHours: number }` | `GroupSummaryDto` | Generates group summary synchronously |
| `generate_summary_stream` | `{ targetId: string; slidingWindowHours: number }` | `GroupSummaryDto` | Generates group summary with SSE chunk events |
| `get_summary_history` | `{ targetId?: string }` | `GroupSummaryDto[]` | Fetches previously saved group summaries |
| `delete_summary` | `{ id: string }` | `void` | Deletes a stored summary |
| `sync_group_files` | `{ groupId: string }` | `GroupFileItemDto[]` | Syncs remote group file catalog |
| `download_file` | `{ groupId: string; fileId: string; fileName: string }` | `{ taskId: string; localSavePath: string }` | Downloads group file in background |
| `summarize_file` | `{ localFilePath: string }` | `FileSummaryResultDto` | Extracts and summarizes local document |

### 2.5 System, Diagnostics & Maintenance
| Command | Arguments | Return Type | Description |
| :--- | :--- | :--- | :--- |
| `get_config` | None | `AppConfig` | Fetches application configuration |
| `update_config` | `{ config: Partial<AppConfig> }` | `AppConfig` | Updates and persists application configuration |
| `test_ai_connection` | `{ provider: string; modelId?: string }` | `{ isSuccess: boolean; latencyMs: number }` | Tests connectivity and latency of an AI endpoint |
| `check_dependencies` | None | `DependencyHealthReport` | Validates QQNT, ports, storage, and engine health |
| `export_diagnostics_bundle` | None | `{ zipFilePath: string }` | Packages sanitized logs and stats into a zip file |
| `upgrade_napcat` | `{ downloadUrl?: string }` | `string` | Downloads and applies latest NapCat runtime package |

---

## 3. Realtime Events (`tauri::Emitter`)

The backend pushes real-time events to the frontend via Webview events:

| Event Name | Payload Type | Description |
| :--- | :--- | :--- |
| `event:protocol-status-changed` | `ProtocolStatusDto` | Dispatched when connection or login status changes |
| `new-chat-message` | `MessageItemDto` | Dispatched on incoming QQ message |
| `new-draft` | `PendingDraftDto` | Dispatched when AI generates a new reply draft |
| `summary-chunk` | `{ chunk: string }` | Progressive SSE token chunk during streaming summary |
| `summary-end` | `{ summary: GroupSummaryDto }` | Dispatched when streaming summary completes |

---

## 4. Model Context Protocol (MCP) Tools

The `eazyqq_cli mcp` command runs a JSON-RPC 2.0 stdio server providing external LLM tools:

| Tool Name | Parameters | Returns | Description |
| :--- | :--- | :--- | :--- |
| `send_message` | `target_id: string`, `content: string` | Text confirmation | Send message to a friend or group |
| `get_chat_history` | `target_id: string`, `limit?: integer` | Array of message objects | Query historical messages |
| `list_contacts` | `target_type?: "friend" \| "group"` | Array of contacts | List contacts and their active rules |
| `trigger_group_summary` | `target_id: string`, `hours?: integer` | Summary object | Run summarization pipeline |
| `extract_document` | `file_path: string` | Extracted text content | Parse PDF, DOCX, TXT, or MD file |
| `simulate_rule_match` | `target_id: string`, `content: string` | Simulation analysis | Test routing behavior on a message |
| `update_target_rule` | `target_id: string`, `mode: string`, ... | Updated rule object | Update rule parameters |
| `get_system_health` | None | Health report object | Get status of dependencies and ports |
| `restart_napcat` | None | Operation result message | Restart the NapCat protocol subprocess |
