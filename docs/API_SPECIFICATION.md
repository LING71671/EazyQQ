# EazyQQ 接口与数据契约规格书 (API_SPECIFICATION.md)

## 1. 统一 API 响应信封结构

所有 Tauri 命令与内部服务调用的返回结果均遵循标准数据信封包装：

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

### 错误码区间定义
- `1000 - 1999`：环境依赖与底层存储错误（如 `ERR_NTQQ_NOT_FOUND`、`ERR_STORAGE_READONLY`）
- `2000 - 2999`：协议连接与账号鉴权错误（如 `ERR_PROTOCOL_DISCONNECTED`、`ERR_QRCODE_EXPIRED`）
- `3000 - 3999`：AI 大模型推理与供应商错误（如 `ERR_AI_SESSION_TIMEOUT`、`ERR_AI_STREAM_ABORTED`）
- `4000 - 4999`：群文件传输与文档解析错误（如 `ERR_FILE_NOT_FOUND`、`ERR_DOC_PARSE_FAILED`）
- `5000 - 5999`：路由规则与草稿审核错误（如 `ERR_RULE_VALIDATION_FAILED`、`ERR_DRAFT_NOT_FOUND`）

---

## 2. Tauri IPC 命令契约

前端通过 `@tauri-apps/api/core::invoke(cmd, args)` 发起命令调用。

### 2.1 协议连接与鉴权 (Protocol & Auth)
| 命令名 | 参数 | 返回值类型 | 描述 |
| :--- | :--- | :--- | :--- |
| `get_protocol_status` | 无 | `ProtocolStatusDto` | 获取底层协议连接状态、二维码 Base64 及登录账号信息 |
| `refresh_qrcode` | 无 | `{ qrcodeBase64: string; expiresInSeconds: number }` | 刷新并获取最新的手 Q 扫码登录二维码 |
| `quick_login` | `{ uin: string }` | `void` | 使用本地持久化凭据快速恢复登录 |
| `logout` | 无 | `void` | 断开当前连接并清除登录态 |

### 2.2 联系人与路由规则 (Contacts & Rules)
| 命令名 | 参数 | 返回值类型 | 描述 |
| :--- | :--- | :--- | :--- |
| `get_contacts` | 无 | `ContactItemDto[]` | 获取好友与群聊全量列表及关联规则 |
| `update_routing_rule` | `{ rule: RoutingRuleDto }` | `RoutingRuleDto` | 更新指定对象的执行模式、触发词、Prompt 或冷却时间 |
| `toggle_summary_whitelist` | `{ targetId: string; enabled: boolean }` | `RoutingRuleDto` | 将指定目标加入或移出周期性群聊总结白名单 |
| `simulate_rule_evaluation` | `{ targetId: string; content: string }` | `RuleEvaluationResultDto` | 模拟单条消息的匹配命中分支，不触发实际外发动作 |

### 2.3 消息与草稿审核 (Messages & Drafts)
| 命令名 | 参数 | 返回值类型 | 描述 |
| :--- | :--- | :--- | :--- |
| `get_chat_history` | `{ targetId: string; limit?: number }` | `MessageItemDto[]` | 分页拉取指定对象的历史会话记录 |
| `send_message` | `{ targetId: string; content: string }` | `void` | 向好友或群聊发送文本消息 |
| `get_pending_drafts` | 无 | `PendingDraftDto[]` | 拉取所有处于待审状态的 AI 拟答草稿 |
| `send_draft` | `{ draftId: string; finalContent?: string }` | `void` | 审核放行并将草稿发送至目标会话 |
| `dismiss_draft` | `{ draftId: string }` | `void` | 驳回并丢弃指定的待办草稿 |
| `regenerate_draft` | `{ draftId: string; promptCorrection?: string }` | `PendingDraftDto` | 附带用户修正提示词重新请求 AI 推理草稿 |

### 2.4 简报与群文件知识库 (Summaries & Files)
| 命令名 | 参数 | 返回值类型 | 描述 |
| :--- | :--- | :--- | :--- |
| `generate_summary` | `{ targetId: string; slidingWindowHours: number }` | `GroupSummaryDto` | 同步提炼目标群聊长文简报 |
| `generate_summary_stream` | `{ targetId: string; slidingWindowHours: number }` | `GroupSummaryDto` | 开启流式提炼，通过 SSE 事件逐 Token 推送并保存结果 |
| `get_summary_history` | `{ targetId?: string }` | `GroupSummaryDto[]` | 拉取已保存的群聊历史简报列表 |
| `delete_summary` | `{ id: string }` | `void` | 删除指定的一条历史简报 |
| `sync_group_files` | `{ groupId: string }` | `GroupFileItemDto[]` | 同步指定群聊的远程文件索引 |
| `download_file` | `{ groupId: string; fileId: string; fileName: string }` | `{ taskId: string; localSavePath: string }` | 后台静默下载群文件至本地缓存 |
| `summarize_file` | `{ localFilePath: string }` | `FileSummaryResultDto` | 提取并总结本地文件核心要点与待办 |

### 2.5 系统配置与自检诊断 (System & Config)
| 命令名 | 参数 | 返回值类型 | 描述 |
| :--- | :--- | :--- | :--- |
| `get_config` | 无 | `AppConfig` | 获取当前生效的应用完整配置 |
| `update_config` | `{ config: Partial<AppConfig> }` | `AppConfig` | 更新并持久化应用配置 |
| `test_ai_connection` | `{ provider: string; modelId?: string }` | `{ isSuccess: boolean; latencyMs: number }` | 测试指定大模型端点的网络连通性与响应延迟 |
| `check_dependencies` | 无 | `DependencyHealthReport` | 自检 QQNT、端口占用、数据读写权限及依赖状态 |
| `export_diagnostics_bundle` | 无 | `{ zipFilePath: string }` | 自动打包已脱敏的运行日志与诊断信息为 zip 文件 |
| `upgrade_napcat` | `{ downloadUrl?: string }` | `string` | 下载并覆写更新 NapCat 协议核心包 |

---

## 3. 前端实时事件总线 (`tauri::Emitter`)

后端通过 Webview 窗口事件向前端主动广播实时数据流：

| 事件标识 | 载荷类型 | 触发时机 |
| :--- | :--- | :--- |
| `event:protocol-status-changed` | `ProtocolStatusDto` | 协议端连接中断、恢复或登录状态切换时 |
| `new-chat-message` | `MessageItemDto` | 收到新的群消息或私聊消息时 |
| `new-draft` | `PendingDraftDto` | 命中草稿模式后 AI 生成完毕拟答建议时 |
| `summary-chunk` | `{ chunk: string }` | 流式长文总结生成中实时输出的单个 Token 增量 |
| `summary-end` | `{ summary: GroupSummaryDto }` | 流式总结完整生成并完成落库入库时 |

---

## 4. 模型上下文协议 (MCP) 工具列表

`eazyqq_cli mcp` 提供标准 JSON-RPC 2.0 stdio 服务，开放以下 9 项核心工具：

| 工具名称 | 输入参数 | 返回值 | 功能说明 |
| :--- | :--- | :--- | :--- |
| `send_message` | `target_id: string`, `content: string` | 发送确认文本 | 发送群聊或私聊消息 |
| `get_chat_history` | `target_id: string`, `limit?: integer` | 消息对象列表 | 获取群聊或私聊历史上下文 |
| `list_contacts` | `target_type?: "friend" \| "group"` | 联系人对象列表 | 查询好友与群聊清单及其规则 |
| `trigger_group_summary` | `target_id: string`, `hours?: integer` | 结构化简报对象 | 触发滑动窗口简报提炼 |
| `extract_document` | `file_path: string` | 纯文本解析内容 | 提取本地 PDF、DOCX、TXT 或 Markdown 内容 |
| `simulate_rule_match` | `target_id: string`, `content: string` | 模拟匹配报告 | 验证单条消息命中的规则与模式 |
| `update_target_rule` | `target_id: string`, `mode: string`, ... | 更新后规则对象 | 动态修改指定目标的路由参数 |
| `get_system_health` | 无 | 链路健康报告对象 | 获取依赖链路与端口健康指标 |
| `restart_napcat` | 无 | 执行状态文本 | 重启协议端子进程并重连 |
