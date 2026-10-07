# EazyQQ 接口与数据契约规范手册 (API_SPECIFICATION.md)

本文档是 EazyQQ 系统的权威接口与数据契约规范，涵盖主进程与渲染层 Tauri IPC 命令、Webview 实时事件总线、外部大模型上下文协议（MCP）服务、CLI 自动化通道及全量数据传输对象（DTO）。

---

## 1. 统一响应信封与错误码体系

### 1.1 统一数据信封 (ApiResponse)
所有 Tauri IPC 命令返回及内部服务输出均使用统一的泛型数据信封包装：

```typescript
export interface ApiResponse<T = void> {
  success: boolean;       // 请求是否成功执行
  data?: T;               // 业务响应载荷（仅在 success 为 true 时存在）
  error?: ApiErrorPayload;// 错误详细信息（仅在 success 为 false 时存在）
  timestamp: number;      // 毫秒级 Unix 时间戳
}

export interface ApiErrorPayload {
  code: number;           // 数字型系统错误码
  message: string;        // 错误简述
  details?: Record<string, unknown>; // 诊断扩展明细
  suggestedAction?: string; // 防呆建议恢复动作
}
```

### 1.2 系统标准错误码字典

| 错误码 | 标识常量 | 含义说明 | 建议恢复动作 |
| :--- | :--- | :--- | :--- |
| **1001** | `ERR_NTQQ_NOT_FOUND` | 未检测到系统官方 QQNT 安装路径 | 指引用户在设置面板中手动指定 QQ 安装主目录 |
| **1002** | `ERR_PORT_CONFLICT` | NapCat (3001) 或 WebUI (6099) 端口被占用 | 提示关闭占用进程或在设置中修改端口 |
| **1003** | `ERR_STORAGE_READONLY` | `EazyQQ_Data` 存储目录无写入权限 | 检查操作系统目录权限并尝试管理员重新运行 |
| **2001** | `ERR_PROTOCOL_DISCONNECTED`| 底层 OneBot WebSocket 断开连接 | 触发自动指数退避重连或在界面点击一键重启协议 |
| **2002** | `ERR_QRCODE_EXPIRED` | 登录二维码过期失效 | 前端重新请求 `refresh_qrcode` 刷新二维码 |
| **2003** | `ERR_LOGIN_TIMEOUT` | 等待扫码确认超时（超过 120 秒） | 提示用户点击重新获取二维码 |
| **2004** | `ERR_SEND_MESSAGE_FAILED` | OneBot 消息发送失败 | 保留待发内容并提示检查网络与登录态 |
| **3001** | `ERR_AI_SESSION_TIMEOUT` | 大模型推理响应超时（超过 60 秒） | 记录日志并建议调小上下文消息条数 |
| **3002** | `ERR_AI_STREAM_ABORTED` | 流式推理被用户或系统异常中断 | 清理动态打字机状态并恢复就绪 |
| **3003** | `ERR_AI_INVALID_KEY` | 大模型 API Key 无效或余额不足 | 引导在设置面板检查供应商凭证与端点 |
| **4001** | `ERR_FILE_NOT_FOUND` | 群文件在远程服务器已失效或删除 | 禁用下载并标注文件不可用 |
| **4002** | `ERR_FILE_PARSE_FAILED` | 文档格式不支持或纯文本提取异常 | 提示仅支持原样下载，不支持生成摘要 |
| **5001** | `ERR_RULE_VALIDATION_FAILED`| 规则配置参数校验不合法 | 提示用户修正表单项 |
| **5002** | `ERR_DRAFT_NOT_FOUND` | 操作的待审草稿不存在或已处理 | 静默移除列表项 |

---

## 2. Tauri IPC 命令全集 (共 44 项)

前端使用 `@tauri-apps/api/core` 的 `invoke(cmd, args)` 唤起。所有命令均已在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册。

### 2.1 协议连接与账号鉴权 (6 项)

#### `get_protocol_status`
- **前端对应**：`api.getProtocolStatus()`
- **参数**：无
- **返回值**：`ApiResponse<ProtocolStatusDto>`
- **说明**：获取协议端是否连接、当前登录状态（`unlogged` / `waiting_scan` / `scanned` / `logged_in`）、Base64 二维码、当前 QQ 号、昵称、头像及免扫码快速登录账号列表。

#### `refresh_qrcode`
- **前端对应**：`api.refreshQrCode()`
- **参数**：无
- **返回值**：`ApiResponse<{ qrcodeBase64: string; expiresInSeconds: number }>`
- **说明**：强制协议端重新向腾讯鉴权中枢申请新的登录二维码。

#### `quick_login`
- **前端对应**：`api.quickLogin(uin)`
- **参数**：`{ uin: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：对选中的历史 UIN 发起免扫码凭证登录。

#### `get_quick_login_accounts`
- **前端对应**：`invoke('get_quick_login_accounts')`
- **参数**：无
- **返回值**：`ApiResponse<QuickLoginAccountDto[]>`
- **说明**：读取本地缓存的所有可用快速登录账号信息。

#### `logout`
- **前端对应**：`invoke('logout')`
- **参数**：无
- **返回值**：`ApiResponse<void>`
- **说明**：注销当前 QQ 会话，清理登录凭证缓存。

#### `get_chain_status`
- **前端对应**：`invoke('get_chain_status')`
- **参数**：无
- **返回值**：`ApiResponse<ChainStatusDto>`
- **说明**：查询 NapCat 控制面、QQ 会话、OneBot 接口、数据库、大模型等 8 节点的逐项连通状态。

---

### 2.2 联系人与规则控制 (4 项)

#### `get_contacts`
- **前端对应**：`api.getContacts()`
- **参数**：无
- **返回值**：`ApiResponse<ContactItemDto[]>`
- **说明**：拉取全部好友与群聊列表，并注入本地数据库中生效的 `RoutingRuleDto`、未读消息数及最后消息剪影。

#### `update_rule`
- **前端对应**：`api.updateRule(rule)`
- **参数**：`{ rule: RoutingRuleDto }`
- **返回值**：`ApiResponse<RoutingRuleDto>`
- **说明**：原子化更新指定对象（`friend:xxx` 或 `group:xxx`）的路由模式、触发条件、关键词、系统提示词、冷却时长与简报白名单标记。

#### `batch_update_mode`
- **前端对应**：`api.batchUpdateMode(targetIds, mode)`
- **参数**：`{ targetIds: string[]; mode: RuleMode }`
- **返回值**：`ApiResponse<void>`
- **说明**：批量将一组联系人或群聊切换至指定路由模式（如全部重置为 `ignore` 或全部设为 `copilot`）。

#### `mark_read`
- **前端对应**：`api.markRead(targetId)`
- **参数**：`{ targetId: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：将目标会话的未读角标计数清零，并更新本地消息已读时间戳。

---

### 2.3 消息交互与草稿审核 (7 项)

#### `get_messages`
- **前端对应**：`api.getMessages(targetId, limit)`
- **参数**：`{ targetId: string; limit?: number }`
- **返回值**：`ApiResponse<MessageItemDto[]>`
- **说明**：以时间正序检索本地 SQLite 中存储的该会话历史消息流水，支持指定条数限制。

#### `send_message`
- **前端对应**：`api.sendMessage(targetId, content)`
- **参数**：`{ targetId: string; content: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：通过 OneBot HTTP 接口向好友或群聊发送原始文本消息。

#### `trigger_ai_reply`
- **前端对应**：`api.triggerAiReply(targetId, contextSnippet)`
- **参数**：`{ targetId: string; contextSnippet: string }`
- **返回值**：`ApiResponse<string>`
- **说明**：直接基于给定的上下文片段唤起大模型推理，生成单条候选回复文本（不直接外发）。

#### `get_pending_drafts`
- **前端对应**：`api.getPendingDrafts()`
- **参数**：无
- **返回值**：`ApiResponse<PendingDraftDto[]>`
- **说明**：读取所有命中 `copilot` 模式并已生成的待审核草稿列表。

#### `send_draft`
- **前端对应**：`api.sendDraft(draftId, finalContent)`
- **参数**：`{ draftId: string; finalContent?: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：放行指定草稿（支持覆盖最终发送文本），调用底层发送外化并将其状态移出待办。

#### `dismiss_draft`
- **前端对应**：`api.dismissDraft(draftId)`
- **参数**：`{ draftId: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：人工驳回草稿，将其标记为已丢弃，从草稿箱中移除。

#### `regenerate_draft`
- **前端对应**：`api.regenerateDraft(draftId, promptCorrection)`
- **参数**：`{ draftId: string; promptCorrection?: string }`
- **返回值**：`ApiResponse<PendingDraftDto>`
- **说明**：携带人工补充修改意见重新调用大模型生成新的草稿内容。

---

### 2.4 群文件与文档知识库 (4 项)

#### `get_group_files`
- **前端对应**：`api.getGroupFiles(groupId, folderId)`
- **参数**：`{ groupId: string; folderId?: string }`
- **返回值**：`ApiResponse<GroupFileItemDto[]>`
- **说明**：通过 OneBot 协议拉取指定群聊根目录或特定子文件夹下的文件清单与下载状态。

#### `download_file`
- **前端对应**：`api.downloadFile(groupId, fileId, fileName)`
- **参数**：`{ groupId: string; fileId: string; fileName: string }`
- **返回值**：`ApiResponse<{ taskId: string; localSavePath: string }>`
- **说明**：在后台线程异步拉取群文件直链并流式落盘到账号沙盒 `files/` 目录。

#### `summarize_file`
- **前端对应**：`api.summarizeFile(localFilePath)`
- **参数**：`{ localFilePath: string }`
- **返回值**：`ApiResponse<FileSummaryResultDto>`
- **说明**：本地提取 PDF / DOCX / TXT / MD / 代码文件正文，调用模型输出核心摘要、论点与待办事项。

#### `open_folder`
- **前端对应**：`api.openFolder(targetPath)`
- **参数**：`{ targetPath: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：调用 Windows 资源管理器打开目标文件所在目录并高亮定位。

---

### 2.5 群聊消息简报与流式长文总结 (4 项)

#### `generate_summary`
- **前端对应**：`api.generateSummary(targetId, slidingWindowHours)`
- **参数**：`{ targetId: string; slidingWindowHours?: number }`
- **返回值**：`ApiResponse<GroupSummaryDto>`
- **说明**：基于滑动时间窗口历史消息，同步阻塞调用大模型生成群聊结构化简报并存库。

#### `generate_summary_stream`
- **前端对应**：`api.generateSummaryStream(targetId, slidingWindowHours)`
- **参数**：`{ targetId: string; slidingWindowHours?: number }`
- **返回值**：`ApiResponse<GroupSummaryDto>`
- **说明**：开启流式生成，通过 `summary-chunk` 事件逐 Token 广播增量文本，完成后触发 `summary-end` 事件并落盘持久化。

#### `get_summary_history`
- **前端对应**：`api.getSummaryHistory(targetId)`
- **参数**：`{ targetId?: string }`
- **返回值**：`ApiResponse<GroupSummaryDto[]>`
- **说明**：检索本地数据库中已归档的历史简报记录列表。

#### `delete_summary`
- **前端对应**：`api.deleteSummary(id)`
- **参数**：`{ id: string }`
- **返回值**：`ApiResponse<void>`
- **说明**：根据 ID 物理删除单条历史群聊简报。

---

### 2.6 系统配置与大模型供应商管理 (6 项)

#### `get_config`
- **前端对应**：`api.getConfig()`
- **参数**：无
- **返回值**：`ApiResponse<AppConfig>`
- **说明**：从 SQLite `app_settings` 表读取当前全局配置。

#### `update_config`
- **前端对应**：`api.updateConfig(config)`
- **参数**：`{ config: Partial<AppConfig> }`
- **返回值**：`ApiResponse<AppConfig>`
- **说明**：合并更新配置项并持久化，即时热重载生效。

#### `test_ai_connection`
- **前端对应**：`api.testAiConnection(provider, modelId)`
- **参数**：`{ provider: string; modelId?: string }`
- **返回值**：`ApiResponse<{ isSuccess: boolean; latencyMs: number }>`
- **说明**：向目标模型接口发起轻量 ping 请求，测定端到端响应延迟。

#### `fetch_provider_models`
- **前端对应**：`api.fetchProviderModels(provider, baseUrl, apiKey)`
- **参数**：`{ provider: string; baseUrl?: string; apiKey?: string }`
- **返回值**：`ApiResponse<string[]>`
- **说明**：请求 OpenAI 兼容接口的 `/v1/models` 端点拉取可用模型列表。

#### `get_qq_path`
- **前端对应**：`api.getQqPath()`
- **参数**：无
- **返回值**：`ApiResponse<string>`
- **说明**：读取当前 NapCat `qq_path.txt` 指向的 QQNT 可执行文件路径。

#### `set_qq_path`
- **前端对应**：`api.setQqPath(path)`
- **参数**：`{ path: string }`
- **返回值**：`ApiResponse<string>`
- **说明**：手动修正并保存 QQNT 的安装根路径。

---

### 2.7 链路健康自检与协议运维 (6 项)

#### `check_dependencies`
- **前端对应**：`api.checkDependencies()`
- **参数**：无
- **返回值**：`ApiResponse<DependencyHealthReport>`
- **说明**：自检 QQNT 二进制就绪、端口占用、存储目录写入权限及磁盘剩余空间。

#### `restart_napcat`
- **前端对应**：`invoke('restart_napcat')`
- **参数**：无
- **返回值**：`ApiResponse<string>`
- **说明**：终止当前 NapCat 进程，清理残留端口并重新调起。

#### `get_napcat_version`
- **前端对应**：`api.getNapCatVersion()`
- **参数**：无
- **返回值**：`ApiResponse<string>`
- **说明**：读取本地集成的 NapCat 运行时核心版本号。

#### `check_napcat_update`
- **前端对应**：`api.checkNapCatUpdate()`
- **参数**：无
- **返回值**：`ApiResponse<NapCatUpdateInfo>`
- **说明**：请求 NapNeko 官方 Release 接口比对是否有更新版本。

#### `upgrade_napcat`
- **前端对应**：`api.upgradeNapCat(downloadUrl)`
- **参数**：`{ downloadUrl?: string }`
- **返回值**：`ApiResponse<string>`
- **说明**：下载最新 NapCat 压缩包解压覆盖并重启。

#### `export_diagnostics_bundle`
- **前端对应**：`api.exportDiagnosticsBundle()`
- **参数**：无
- **返回值**：`ApiResponse<{ zipFilePath: string }>`
- **说明**：自动收集系统硬件信息、链路状态、脱敏日志打包为 `.zip`。

---

### 2.8 窗口控制与系统更新 (7 项)

#### `app_minimize_window`
- **前端对应**：`api.minimizeWindow()`
- **参数**：无
- **返回值**：`ApiResponse<boolean>`
- **说明**：根据配置将窗口最小化至系统托盘或任务栏。

#### `app_toggle_maximize_window`
- **前端对应**：`api.toggleMaximizeWindow()`
- **参数**：无
- **返回值**：`ApiResponse<boolean>`
- **说明**：在窗口最大化与正常尺寸间切换。

#### `app_close_window`
- **前端对应**：`api.closeWindow()`
- **参数**：无
- **返回值**：`ApiResponse<boolean>`
- **说明**：触发关闭。若启用了 `closeToTray` 则隐藏窗口至托盘，否则正常退出。

#### `app_start_drag_window`
- **前端对应**：`api.startDragWindow()`
- **参数**：无
- **返回值**：`ApiResponse<void>`
- **说明**：自绘无边框标题栏鼠标按下拖拽事件通知。

#### `app_show_window`
- **前端对应**：`api.showWindow()`
- **参数**：无
- **返回值**：`ApiResponse<void>`
- **说明**：托盘点击后唤醒并置前主窗口。

#### `app_get_window_behavior`
- **前端对应**：`api.getWindowBehavior()`
- **参数**：无
- **返回值**：`ApiResponse<WindowBehaviorDto>`
- **说明**：查询 `minimizeToTray` 与 `closeToTray` 当前策略开关。

#### `check_app_update`
- **前端对应**：`api.checkAppUpdate()`
- **参数**：无
- **返回值**：`ApiResponse<AppUpdateInfo>`
- **说明**：从 GitHub Releases 检查 EazyQQ 桌面端应用自身是否有更新版本。

---

## 3. Webview 实时推送事件总线

前端通过 `@tauri-apps/api/event::listen(eventName, callback)` 监听：

| 事件标识符 | 载荷类型 | 触发时机与用途 |
| :--- | :--- | :--- |
| `event:protocol-status-changed` | `ProtocolStatusDto` | 协议端状态机流转时（二维码生成、扫码成功、离线）广播 |
| `new-chat-message` | `MessageItemDto` | 收到任何好友或群聊的新消息流水时即时推送给前端更新会话视图 |
| `new-draft` | `PendingDraftDto` | 命中草稿模式后 AI 推理完毕，向草稿箱推送新卡片并更新未读徽标 |
| `summary-chunk` | `SummaryChunkPayload` | 流式长文总结生成中实时输出的增量 Token 片段（`{ chunk: string }`） |
| `summary-end` | `SummaryEndPayload` | 流式长文总结生成结束，广播完整结构化简报实体（`{ summary: GroupSummaryDto }`） |

---

## 4. 模型上下文协议 (MCP) 工具规格

执行 `eazyqq_cli mcp` 会启动标准 JSON-RPC 2.0 stdio 服务端，供 Cursor、Claude Desktop 或 Antigravity 调度使用。

### 4.1 标准工具全量列表

```json
[
  {
    "name": "send_message",
    "description": "向指定的 QQ 好友或群聊发送文本消息",
    "parameters": {
      "type": "object",
      "properties": {
        "target_id": { "type": "string", "description": "目标标识，如 'friend:10001' 或 'group:20002'" },
        "content": { "type": "string", "description": "要发送的文本正文" }
      },
      "required": ["target_id", "content"]
    }
  },
  {
    "name": "get_chat_history",
    "description": "检索指定对象的最近聊天历史记录",
    "parameters": {
      "type": "object",
      "properties": {
        "target_id": { "type": "string", "description": "目标标识" },
        "limit": { "type": "integer", "description": "最多获取消息条数，默认 20" }
      },
      "required": ["target_id"]
    }
  },
  {
    "name": "list_contacts",
    "description": "列出本账号的所有好友或群聊以及当前绑定的路由规则",
    "parameters": {
      "type": "object",
      "properties": {
        "target_type": { "type": "string", "enum": ["friend", "group"], "description": "过滤好友或群聊" }
      }
    }
  },
  {
    "name": "trigger_group_summary",
    "description": "基于历史消息滑动时间窗口为指定群聊触发 AI 结构化长文简报提炼",
    "parameters": {
      "type": "object",
      "properties": {
        "target_id": { "type": "string", "description": "群聊标识，如 'group:20002'" },
        "hours": { "type": "integer", "description": "回溯的滑动时间窗口（小时），默认 6" }
      },
      "required": ["target_id"]
    }
  },
  {
    "name": "extract_document",
    "description": "本地提取指定路径文档的纯文本内容（支持 PDF、DOCX、TXT、Markdown）",
    "parameters": {
      "type": "object",
      "properties": {
        "file_path": { "type": "string", "description": "本地绝对文件路径" }
      },
      "required": ["file_path"]
    }
  },
  {
    "name": "simulate_rule_match",
    "description": "模拟测试单条消息在安全决策引擎中的命中分支与动作",
    "parameters": {
      "type": "object",
      "properties": {
        "target_id": { "type": "string", "description": "来源会话标识" },
        "content": { "type": "string", "description": "模拟收到的消息文本" }
      },
      "required": ["target_id", "content"]
    }
  },
  {
    "name": "update_target_rule",
    "description": "更新指定好友或群聊的规则模式与参数",
    "parameters": {
      "type": "object",
      "properties": {
        "target_id": { "type": "string", "description": "目标标识" },
        "mode": { "type": "string", "enum": ["auto_reply", "copilot", "summary_only", "ignore"] },
        "trigger_condition": { "type": "string", "enum": ["all", "at_me", "keyword"] },
        "keywords": { "type": "array", "items": { "type": "string" } },
        "is_summary_whitelist": { "type": "boolean" }
      },
      "required": ["target_id", "mode"]
    }
  },
  {
    "name": "get_system_health",
    "description": "检查底层 QQNT 路径、端口冲突及运行环境健康度",
    "parameters": { "type": "object", "properties": {} }
  },
  {
    "name": "restart_napcat",
    "description": "平滑重启 NapCat 协议子进程",
    "parameters": { "type": "object", "properties": {} }
  }
]
```

---

## 5. CLI 自动化接口速查 (`eazyqq_cli`)

所有命令均支持附加 `--json` 参数以输出标准 JSON 报文：

```bash
# 协议与状态
eazyqq_cli status
eazyqq_cli login-info
eazyqq_cli quick-login-list
eazyqq_cli quick-login --uin 10001
eazyqq_cli qr [--save qrcode.png]

# 联系人与规则
eazyqq_cli contacts [--type friend|group] [--search 关键词] [--whitelist]
eazyqq_cli rule --target group:20002 --mode copilot --trigger at_me
eazyqq_cli groups
eazyqq_cli friends

# 消息与草稿
eazyqq_cli send --target group:20002 --text "自动化测试消息"
eazyqq_cli ask --target group:20002 --text "请教一个Rust编译问题"
eazyqq_cli drafts
eazyqq_cli draft-send --id draft_01 [--text "二次修改文本"]
eazyqq_cli draft-dismiss --id draft_01
eazyqq_cli draft-regenerate --id draft_01 --instruction "换成更委婉的语气"
eazyqq_cli history --target group:20002 --limit 50
eazyqq_cli mark-read --target group:20002

# 简报与文件
eazyqq_cli summarize --target group:20002 [--hours 6]
eazyqq_cli summaries [--target group:20002]
eazyqq_cli whitelist-groups
eazyqq_cli files --target group:20002
eazyqq_cli file-download --target group:20002 --file-id file_123
eazyqq_cli file-summarize --path "B:\files\meeting.docx"

# 系统配置与自省
eazyqq_cli config [--json]
eazyqq_cli set-config --key ai.model --value deepseek-chat
eazyqq_cli ai-test
eazyqq_cli chain-status
eazyqq_cli health [--deep]
eazyqq_cli export
eazyqq_cli schema --json
eazyqq_cli mcp
```

---

## 6. 全量数据传输对象 (DTO) 契约

```typescript
export interface ProtocolStatusDto {
  isConnected: boolean;
  loginStatus: 'unlogged' | 'waiting_scan' | 'scanned' | 'logged_in';
  qrcodeBase64?: string;
  qrcodeError?: string;
  qqNumber?: string;
  nickname?: string;
  avatarUrl?: string;
  connectTime?: number;
  quickLoginAccounts?: QuickLoginAccountDto[];
}

export interface QuickLoginAccountDto {
  uin: string;
  nickname: string;
  faceUrl?: string;
}

export interface ContactItemDto {
  id: string; // 格式: 'friend:10001' | 'group:20002'
  targetType: 'friend' | 'group';
  targetId: string;
  name: string;
  avatarUrl: string;
  remark?: string;
  memberCount?: number;
  rule: RoutingRuleDto;
  unreadCount: number;
  lastMessageSnippet?: string;
  lastMessageTimestamp?: number;
}

export type RuleMode = 'auto_reply' | 'copilot' | 'summary_only' | 'ignore';

export interface RoutingRuleDto {
  id: string;
  targetId: string;
  mode: RuleMode;
  triggerCondition: 'all' | 'at_me' | 'keyword';
  keywords: string[];
  systemPrompt?: string;
  modelId?: string;
  cooldownSeconds: number;
  enabled: boolean;
  isSummaryWhitelist?: boolean;
  summaryIntervalHours?: number;
}

export interface MessageItemDto {
  id: string;
  targetId: string;
  senderId: string;
  senderName: string;
  senderAvatar?: string;
  content: string;
  isFromMe: boolean;
  aiReplyStatus: 'none' | 'auto_replied' | 'draft_pending' | 'summarized';
  timestamp: number;
}

export interface PendingDraftDto {
  id: string;
  targetId: string;
  targetName: string;
  targetType: 'friend' | 'group';
  replyToMsgId: string;
  incomingMessageSnippet: string;
  generatedContent: string;
  thinkingContent?: string;
  modelUsed: string;
  confidenceScore?: number;
  createdAt: number;
}

export interface GroupFileItemDto {
  fileId: string;
  fileName: string;
  fileSize: number;
  fileUrl?: string;
  uploaderId: string;
  uploaderName: string;
  uploadTime: number;
  downloadStatus: 'remote' | 'downloading' | 'downloaded';
  localPath?: string;
  isFolder: boolean;
  folderId?: string;
}

export interface FileSummaryResultDto {
  fileName: string;
  fileSize: number;
  totalChars: number;
  summaryText: string;
  keyTakeaways: string[];
  actionItems: string[];
}

export interface GroupSummaryDto {
  id: string;
  targetId: string;
  targetName: string;
  summaryText: string;
  keyPoints: string[];
  decisions: string[];
  sharedFiles: string[];
  startTime: number;
  endTime: number;
  createdAt: number;
}

export interface SummaryChunkPayload {
  chunk: string;
}

export interface SummaryEndPayload {
  summary: GroupSummaryDto;
}

export interface DependencyHealthReport {
  isAllReady: boolean;
  qqNt: { ready: boolean; path: string; error?: string };
  openCode: { ready: boolean; version?: string; activeModel?: string; error?: string };
  ports: { napcatPort: number; opencodePort: number; isConflict: boolean };
  storage: { workspacePath: string; isWritable: boolean; freeSpaceMb: number };
}

export type AiProviderId =
  | 'opencode'
  | 'ollama'
  | 'lmstudio'
  | 'llamacpp'
  | 'vllm'
  | 'openai';

export type SummaryIntervalType = '1h' | '2h' | '4h' | '6h' | '12h' | '24h' | 'custom';

export interface AppConfig {
  ai: {
    activeProvider: AiProviderId;
    model: string;
    temperature: number;
    maxContextMessages: number;
    baseUrl?: string;
    apiKey?: string;
    providers?: Record<string, { model: string; baseUrl?: string; apiKey?: string }>;
  };
  napcat: {
    wsPort: number;
    autoRestart: boolean;
    heartbeatIntervalSec: number;
  };
  storage: {
    autoSyncFiles: boolean;
    maxFileSizeMb: number;
  };
  summary: {
    enabled: boolean;
    intervalType: SummaryIntervalType;
    customIntervalMinutes: number;
    slidingWindowHours: number;
    autoForwardToPhone: boolean;
    customPrompt?: string;
  };
  window: {
    minimizeToTray: boolean;
    closeToTray: boolean;
  };
}

export interface WindowBehaviorDto {
  minimizeToTray: boolean;
  closeToTray: boolean;
}

export interface AppUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  releaseName: string;
  releaseNotes: string;
  htmlUrl: string;
  downloadUrl?: string;
  publishedAt: string;
}

export interface NapCatUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  releaseName: string;
  releaseNotes: string;
  downloadUrl?: string;
}
```
