# EazyQQ：完整 API 与数据交互契约规范书

本规范书定义了 EazyQQ 系统内外部的全部 API 交互接口、数据传输对象（DTO）、主渲染双向推送事件、本地智能引擎通信、底层协议动作及数据持久化契约。

文档面向主进程开发、前端界面开发、AI 引擎集成开发以及自动化测试。全书分为七大核心模块：
1. **统一 API 响应信封与错误码体系（Response Envelope & Error Codes）**
2. **主渲染进程强类型 IPC 契约（Electron Main <-> Renderer）**
3. **Preload 上下文桥接接口定义（`window.electronAPI`）**
4. **本地 OpenCode 智能引擎接口契约（OpenCode SDK & REST/SSE）**
5. **底层 QQ 协议事件与动作契约（NapCat OneBot 11 WebSocket API）**
6. **本地数据访问层 DAO 契约（Drizzle ORM Repository）**
7. **透明文件系统数据交互规范（`EazyQQ_Data` JSONL & Markdown）**

---

## 1. 统一 API 响应信封与错误码体系

### 1.1 统一响应信封（ApiResponse Envelope）

所有主进程向渲染进程返回的 IPC 调用结果，以及内部服务层方法调用结果，均统一遵循以下标准数据信封结构：

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
  suggestedAction?: string; // 给前端呈现给用户的防呆指引动作（如：打开F1说明书、点击重新检测）
}
```

### 1.2 系统标准错误码字典（System Error Dictionary）

| 错误代码 | 标识符 | 含义说明 | 建议前端恢复动作 |
| :--- | :--- | :--- | :--- |
| **1001** | `ERR_NTQQ_NOT_INSTALLED` | 未在宿主机默认路径检测到 NTQQ | 弹出路径配置弹窗，指引用户选择安装目录 |
| **1002** | `ERR_OPENCODE_NOT_FOUND` | 未检测到本地 OpenCode CLI 运行时 | 提示“正在使用备用云端模型”或引导快速安装 |
| **1003** | `ERR_PORT_NAPCAT_CONFLICT` | NapCat 本地通信端口（3001）被占用 | 引导一键释放端口或在设置中修改端口号 |
| **1004** | `ERR_PORT_OPENCODE_CONFLICT` | OpenCode 本地端口（4096）被占用 | 提示检测占用进程，或自动切至随机空闲端口 |
| **1005** | `ERR_STORAGE_READONLY` | 指定的 `EazyQQ_Data` 目录无写入权限 | 引导更换存储目录至用户应用数据目录 |
| **2001** | `ERR_PROTOCOL_DISCONNECTED` | 与 NapCat 本地核心的 WebSocket 断开 | 界面显示连接断开状态栏，后台执行指数退避重连 |
| **2002** | `ERR_QRCODE_EXPIRED` | 登录二维码超时已失效 | 界面展示“二维码已失效，点击刷新”覆层 |
| **2003** | `ERR_LOGIN_TIMEOUT` | 等待扫码确认超时（超过 120 秒） | 自动停止轮询并提示用户点击重新生成 |
| **2004** | `ERR_SEND_MESSAGE_FAILED` | 调用底层协议发送消息失败 | 草稿箱保留该卡片并标注“发送失败，点击重试” |
| **3001** | `ERR_AI_SESSION_TIMEOUT` | AI 推理生成草稿超时（超过 30 秒） | 降级取消本次自动推理，记录诊断日志 |
| **3002** | `ERR_AI_STREAM_ABORTED` | 用户主动中止或会话上下文被重置 | 界面清理打字机动态光标，恢复就绪状态 |
| **3003** | `ERR_AI_INVALID_RESPONSE` | 大模型返回空内容或不可解析的数据 | 记录原始模型输出，提示用户重试或微调提示词 |
| **4001** | `ERR_FILE_NOT_FOUND` | 远程群文件已过期或被群主删除 | 在文件列表标记该文件不可用，禁用下载按钮 |
| **4002** | `ERR_FILE_DOWNLOAD_ABORTED`| 用户主动取消下载或网络异常中断 | 清理 `.tmp` 临时分块文件，支持断点续传 |
| **4003** | `ERR_FILE_PARSE_FAILED` | 文档格式不支持或已损坏无法提取文本 | 提示“纯文本提取失败，仅支持下载原始文件” |
| **5001** | `ERR_RULE_VALIDATION_FAILED`| 路由策略参数校验不合法（Zod 抛出） | 表单字段高亮红框并显示校验错误提示文案 |
| **5002** | `ERR_DRAFT_NOT_FOUND` | 指定的待审草稿不存在或已被他人处理 | 前端平滑移除该待办卡片，无需报错打扰用户 |

---

## 2. 主渲染进程强类型 IPC 契约规范（IPC Contract）

主进程（Main）与渲染进程（Renderer）之间通过 `ipcMain.handle` 与 `ipcRenderer.invoke` 进行请求-响应交互；通过 `webContents.send` 与 `ipcRenderer.on` 进行单向事件推送。

### 2.1 协议状态与鉴权（Auth & Protocol API）

#### `protocol:get-status`
- **方向**：Renderer -> Main
- **功能**：拉取当前 NTQQ 协议引擎运行状态与登录会话信息。
- **请求参数**：无
- **返回类型**：`ApiResponse<ProtocolStatusDto>`
  ```typescript
  export interface ProtocolStatusDto {
    isConnected: boolean;
    loginStatus: 'unlogged' | 'waiting_scan' | 'scanned' | 'logged_in';
    qrcodeBase64?: string;
    qqNumber?: string;
    nickname?: string;
    avatarUrl?: string;
    connectTime?: number;
  }
  ```

#### `protocol:refresh-qrcode`
- **方向**：Renderer -> Main
- **功能**：主动强制重新生成登录二维码。
- **请求参数**：无
- **返回类型**：`ApiResponse<{ qrcodeBase64: string; expiresInSeconds: number }>`

#### `protocol:logout`
- **方向**：Renderer -> Main
- **功能**：退出当前登录的 QQ 账号，清空会话状态。
- **请求参数**：无
- **返回类型**：`ApiResponse<void>`

---

### 2.2 联系人与规则配置（Contacts & Rules API）

#### `contacts:get-list`
- **方向**：Renderer -> Main
- **功能**：拉取好友与群聊列表及其配置的 AI 处理策略。
- **请求参数**：
  ```typescript
  export interface GetContactsRequest {
    type?: 'all' | 'friend' | 'group';
    searchKeyword?: string;
    pageIndex?: number; // 默认 1
    pageSize?: number;  // 默认 50
  }
  ```
- **返回类型**：`ApiResponse<{ list: ContactItemDto[]; total: number }>`
  ```typescript
  export interface ContactItemDto {
    id: string; // 格式："friend:10001" 或 "group:20002"
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

  export interface RoutingRuleDto {
    id: string;
    targetId: string;
    mode: 'auto_reply' | 'copilot' | 'summary_only' | 'ignore';
    triggerCondition: 'all' | 'at_me' | 'keyword';
    keywords: string[];
    systemPrompt?: string;
    modelId?: string;
    cooldownSeconds: number;
    enabled: boolean;
  }
  ```

#### `contacts:update-rule`
- **方向**：Renderer -> Main
- **功能**：更新特定联系人或群聊的个性化 AI 处理策略。
- **请求参数**：
  ```typescript
  export interface UpdateRoutingRuleDto {
    targetId: string;
    mode?: 'auto_reply' | 'copilot' | 'summary_only' | 'ignore';
    triggerCondition?: 'all' | 'at_me' | 'keyword';
    keywords?: string[];
    systemPrompt?: string;
    modelId?: string;
    cooldownSeconds?: number;
    enabled?: boolean;
  }
  ```
- **返回类型**：`ApiResponse<RoutingRuleDto>`

#### `contacts:batch-update-mode`
- **方向**：Renderer -> Main
- **功能**：批量调整多个联系人/群聊的运行模式（配合防呆二次确认弹窗）。
- **请求参数**：
  ```typescript
  export interface BatchUpdateModeRequest {
    targetIds: string[];
    mode: 'auto_reply' | 'copilot' | 'summary_only' | 'ignore';
  }
  ```
- **返回类型**：`ApiResponse<{ affectedCount: number }>`

---

### 2.3 历史消息与草稿审批箱（Messages & Drafts API）

#### `messages:get-history`
- **方向**：Renderer -> Main
- **功能**：分页查询指定会话的历史记录。
- **请求参数**：
  ```typescript
  export interface GetMessagesRequest {
    targetId: string;
    limit: number;
    offset: number;
  }
  ```
- **返回类型**：`ApiResponse<MessageItemDto[]>`
  ```typescript
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
  ```

#### `messages:send`
- **方向**：Renderer -> Main
- **功能**：直接在界面中向指定会话发送文本或富文本消息。
- **请求参数**：
  ```typescript
  export interface SendMessageRequest {
    targetId: string;
    content: string;
    replyToMessageId?: string;
  }
  ```
- **返回类型**：`ApiResponse<{ messageId: string }>`

#### `drafts:get-pending`
- **方向**：Renderer -> Main
- **功能**：拉取当前待人工审核的全部 AI 生成草稿。
- **请求参数**：无
- **返回类型**：`ApiResponse<PendingDraftDto[]>`
  ```typescript
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
  ```

#### `drafts:send`
- **方向**：Renderer -> Main
- **功能**：用户核准草稿（支持在发送前做即时内容修改），发送至 QQ 会话。
- **请求参数**：
  ```typescript
  export interface SendDraftRequest {
    draftId: string;
    finalContent?: string;
  }
  ```
- **返回类型**：`ApiResponse<{ sentMessageId: string }>`

#### `drafts:dismiss`
- **方向**：Renderer -> Main
- **功能**：驳回并作废草稿，不向 QQ 发送任何消息。
- **请求参数**：`{ draftId: string }`
- **返回类型**：`ApiResponse<void>`

#### `drafts:regenerate`
- **方向**：Renderer -> Main
- **功能**：携带微调指令要求 AI 重新生成草稿（旧草稿将被替换）。
- **请求参数**：
  ```typescript
  export interface RegenerateDraftRequest {
    draftId: string;
    customInstruction?: string; // 例如“语气更正式一点”、“提取关键行动项”
  }
  ```
- **返回类型**：`ApiResponse<PendingDraftDto>`

---

### 2.4 群文件与文档知识库（Files & Intelligence API）

#### `files:get-group-files`
- **方向**：Renderer -> Main
- **功能**：获取指定群的文件树形列表。
- **请求参数**：
  ```typescript
  export interface GetGroupFilesRequest {
    groupId: string;
    folderId?: string; // 若留空则拉取群文件根目录
  }
  ```
- **返回类型**：`ApiResponse<GroupFileItemDto[]>`
  ```typescript
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
  ```

#### `files:download-file`
- **方向**：Renderer -> Main
- **功能**：触发指定群文件的后台静默下载。
- **请求参数**：`{ groupId: string; fileId: string; fileName: string }`
- **返回类型**：`ApiResponse<{ taskId: string; localSavePath: string }>`

#### `files:cancel-download`
- **方向**：Renderer -> Main
- **功能**：取消正在进行的文件下载任务。
- **请求参数**：`{ taskId: string }`
- **返回类型**：`ApiResponse<void>`

#### `files:summarize-file`
- **方向**：Renderer -> Main
- **功能**：调用本地解析引擎与大模型对已下载的文档提取结构化摘要。
- **请求参数**：`{ localFilePath: string }`
- **返回类型**：`ApiResponse<FileSummaryResultDto>`
  ```typescript
  export interface FileSummaryResultDto {
    fileName: string;
    fileSize: number;
    totalChars: number;
    summaryText: string;
    keyTakeaways: string[];
    actionItems: string[];
  }
  ```

#### `files:sync-all-group-files`
- **方向**：Renderer -> Main
- **功能**：全量比对并自动同步指定群内的所有新增文件到本地。
- **请求参数**：`{ groupId: string }`
- **返回类型**：`ApiResponse<{ totalFound: number; startedDownloadCount: number }>`

#### `files:open-local-folder`
- **方向**：Renderer -> Main
- **功能**：在操作系统文件资源管理器中直接定位并选中指定文件或打开目录。
- **请求参数**：`{ targetPath: string }`
- **返回类型**：`ApiResponse<void>`

---

### 2.5 智能简报服务（Summary API）

#### `summary:generate-now`
- **方向**：Renderer -> Main
- **功能**：即时基于滑动时间窗口汇总指定会话的消息脉络。
- **请求参数**：
  ```typescript
  export interface GenerateSummaryRequest {
    targetId: string;
    slidingWindowHours: number; // 默认 6 小时
    forwardToMyPhone?: boolean; // 是否同步将简报推送回自己的手机 QQ
  }
  ```
- **返回类型**：`ApiResponse<GroupSummaryDto>`
  ```typescript
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
  ```

#### `summary:get-history`
- **方向**：Renderer -> Main
- **功能**：分页拉取历史生成的简报归档列表。
- **请求参数**：`{ targetId?: string; limit?: number; offset?: number }`
- **返回类型**：`ApiResponse<GroupSummaryDto[]>`

---

### 2.6 系统配置与环境自检（Config & Health API）

#### `config:get`
- **方向**：Renderer -> Main
- **功能**：拉取完整的系统集中配置对象。
- **请求参数**：无
- **返回类型**：`ApiResponse<AppConfig>`

#### `config:update`
- **方向**：Renderer -> Main
- **功能**：保存配置项修改并触发系统各模块的热重载。
- **请求参数**：`Partial<AppConfig>`
- **返回类型**：`ApiResponse<AppConfig>`

#### `config:test-ai-connection`
- **方向**：Renderer -> Main
- **功能**：测试当前配置的 AI 模型连通性与延迟。
- **请求参数**：`{ provider: 'opencode' | 'openai' | 'deepseek'; modelId?: string }`
- **返回类型**：`ApiResponse<{ isSuccess: boolean; latencyMs: number; replySnippet?: string }>`

#### `health:check-dependencies`
- **方向**：Renderer -> Main
- **功能**：全面执行前置运行环境自检。
- **请求参数**：无
- **返回类型**：`ApiResponse<DependencyHealthReport>`
  ```typescript
  export interface DependencyHealthReport {
    isAllReady: boolean;
    qqNt: { ready: boolean; path: string; error?: string };
    openCode: { ready: boolean; version?: string; activeModel?: string; error?: string };
    ports: { napcatPort: number; opencodePort: number; isConflict: boolean };
    storage: { workspacePath: string; isWritable: boolean; freeSpaceMb: number };
  }
  ```

#### `diagnostics:export-bundle`
- **方向**：Renderer -> Main
- **功能**：一键打包自动脱敏的系统运行日志，并在文件管理器中高亮显示 ZIP 包。
- **请求参数**：无
- **返回类型**：`ApiResponse<{ zipFilePath: string }>`

---

### 2.7 主进程向渲染进程的实时推送事件（Push Events）

主进程在发生状态变更或收到异步事件时，通过 WebContents 通道推送到渲染层：

| 事件通道名称 | 携带载荷类型 | 触发时机 |
| :--- | :--- | :--- |
| `event:protocol-status-changed` | `ProtocolStatusDto` | 协议引擎连接、断开、扫码进度变更 |
| `event:draft-created` | `PendingDraftDto` | 产生新的 AI 审核草稿，待办红点增加 |
| `event:draft-updated` | `PendingDraftDto` | 草稿重新生成完成或被修改 |
| `event:draft-removed` | `{ draftId: string }` | 草稿被批准发送或被用户作废移除 |
| `event:message-received` | `MessageItemDto` | 收到新的好友私聊或群聊消息 |
| `event:file-download-progress` | `FileDownloadProgressPayload` | 群文件后台下载分块传输中（0-100%） |
| `event:file-download-completed` | `{ taskId: string; localPath: string }` | 文件下载落盘校验完毕 |
| `event:summary-progress` | `{ progress: number; currentStep: string }` | 长文本简报生成中的进度提示 |
| `event:toast` | `{ level: 'info' \| 'success' \| 'warning' \| 'error'; title: string; message: string }` | 全局无侵入浮动轻量通知 |

```typescript
export interface FileDownloadProgressPayload {
  taskId: string;
  fileId: string;
  percent: number;        // 0.0 - 100.0
  transferredBytes: number;
  totalBytes: number;
  speedBytesPerSec: number;
}
```

---

## 3. Preload 上下文桥接接口定义（`window.electronAPI`）

在 `preload/index.ts` 中通过 `contextBridge.exposeInMainWorld('electronAPI', api)` 导出，严格保证渲染进程不直接接触 Node 原生接口：

```typescript
export interface IElectronAPI {
  // 协议与鉴权
  getProtocolStatus: () => Promise<ApiResponse<ProtocolStatusDto>>;
  refreshQrCode: () => Promise<ApiResponse<{ qrcodeBase64: string; expiresInSeconds: number }>>;
  logout: () => Promise<ApiResponse<void>>;

  // 联系人与路由
  getContacts: (req: GetContactsRequest) => Promise<ApiResponse<{ list: ContactItemDto[]; total: number }>>;
  updateRule: (dto: UpdateRoutingRuleDto) => Promise<ApiResponse<RoutingRuleDto>>;
  batchUpdateMode: (dto: BatchUpdateModeRequest) => Promise<ApiResponse<{ affectedCount: number }>>;

  // 消息与草稿
  getMessages: (req: GetMessagesRequest) => Promise<ApiResponse<MessageItemDto[]>>;
  sendMessage: (req: SendMessageRequest) => Promise<ApiResponse<{ messageId: string }>>;
  getPendingDrafts: () => Promise<ApiResponse<PendingDraftDto[]>>;
  sendDraft: (req: SendDraftRequest) => Promise<ApiResponse<{ sentMessageId: string }>>;
  dismissDraft: (draftId: string) => Promise<ApiResponse<void>>;
  regenerateDraft: (req: RegenerateDraftRequest) => Promise<ApiResponse<PendingDraftDto>>;

  // 文件管理
  getGroupFiles: (req: GetGroupFilesRequest) => Promise<ApiResponse<GroupFileItemDto[]>>;
  downloadFile: (groupId: string, fileId: string, fileName: string) => Promise<ApiResponse<{ taskId: string; localSavePath: string }>>;
  cancelDownload: (taskId: string) => Promise<ApiResponse<void>>;
  summarizeFile: (localFilePath: string) => Promise<ApiResponse<FileSummaryResultDto>>;
  syncAllGroupFiles: (groupId: string) => Promise<ApiResponse<{ totalFound: number; startedDownloadCount: number }>>;
  openLocalFolder: (targetPath: string) => Promise<ApiResponse<void>>;

  // 简报生成
  generateSummary: (req: GenerateSummaryRequest) => Promise<ApiResponse<GroupSummaryDto>>;
  getSummaryHistory: (targetId?: string) => Promise<ApiResponse<GroupSummaryDto[]>>;

  // 配置与自检
  getConfig: () => Promise<ApiResponse<AppConfig>>;
  updateConfig: (cfg: Partial<AppConfig>) => Promise<ApiResponse<AppConfig>>;
  testAiConnection: (provider: string, modelId?: string) => Promise<ApiResponse<{ isSuccess: boolean; latencyMs: number }>>;
  checkDependencies: () => Promise<ApiResponse<DependencyHealthReport>>;
  exportDiagnosticsBundle: () => Promise<ApiResponse<{ zipFilePath: string }>>;

  // 事件监听器注册与解绑
  onProtocolStatusChanged: (callback: (status: ProtocolStatusDto) => void) => () => void;
  onDraftCreated: (callback: (draft: PendingDraftDto) => void) => () => void;
  onDraftUpdated: (callback: (draft: PendingDraftDto) => void) => () => void;
  onDraftRemoved: (callback: (data: { draftId: string }) => void) => () => void;
  onMessageReceived: (callback: (msg: MessageItemDto) => void) => () => void;
  onFileDownloadProgress: (callback: (p: FileDownloadProgressPayload) => void) => () => void;
  onFileDownloadCompleted: (callback: (data: { taskId: string; localPath: string }) => void) => () => void;
  onToast: (callback: (toast: { level: string; title: string; message: string }) => void) => () => void;
}

declare global {
  interface Window {
    electronAPI: IElectronAPI;
  }
}
```

---

## 4. 本地 OpenCode 智能引擎接口契约（OpenCode SDK & REST/SSE）

EazyQQ 内置的 `OpenCodeProvider` 通过本地守护进程或直接调用 `@opencode-ai/sdk` 与 `opencode serve` 交互。宿主机本地默认端口为 `http://127.0.0.1:4096`。

### 4.1 服务健康与可用模型发现

#### `GET /health`
- **说明**：检查本地 OpenCode 运行时是否存活就绪。
- **返回体**：
  ```json
  {
    "status": "ok",
    "version": "1.0.4",
    "activeProvider": "local"
  }
  ```

#### `GET /models`
- **说明**：拉取当前 OpenCode 默认可用的模型列表。
- **返回体**：
  ```json
  {
    "models": [
      {
        "id": "opencode-default",
        "name": "OpenCode Local Assistant",
        "contextWindow": 32768,
        "supportsThinking": true
      }
    ]
  }
  ```

### 4.2 会话生命周期与上下文工作区绑定

#### `POST /session`
- **说明**：为指定的群聊或联系人初始化专用隔离工作区。
- **请求体**：
  ```json
  {
    "id": "eazyqq_group_987654321",
    "title": "项目讨论群(987654321)",
    "dir": "B:/EazyQQ_Data/groups/项目讨论群_987654321"
  }
  ```
- **返回体**：`{ "sessionId": "eazyqq_group_987654321", "created": true }`

### 4.3 流式推理与思考链提取（Prompt SSE Contract）

#### `POST /session/:id/prompt`
- **说明**：提交输入 Prompt，接收 SSE 流式响应，包括打字机正文与思考过程。
- **请求体**：
  ```json
  {
    "prompt": "基于最近的消息流水，请撰写一段专业、简短的回复",
    "thinking": true,
    "system": "你是一个沉稳专业的个人工作助理，代表用户在工作群中沟通。"
  }
  ```
- **SSE 事件帧格式规范**：

```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive

event: thinking
data: {"delta": "分析群成员提问的意图，确认开会时间在下周二..."}

event: thinking
data: {"delta": "整理语气，避免口语化，给出明确行动项。"}

event: text
data: {"delta": "收到，"}

event: text
data: {"delta": "下周二上午10点开会，材料已整理完毕。"}

event: done
data: {"finishReason": "stop", "promptTokens": 142, "completionTokens": 38, "totalTimeMs": 420}
```

### 4.4 任务中断契约

#### `POST /session/:id/abort`
- **说明**：立即掐断当前正在推理的后台推理线程，释放 GPU/CPU 资源。
- **返回体**：`{ "aborted": true }`

---

## 5. 底层 QQ 协议事件与动作契约（NapCat OneBot 11 WebSocket API）

EazyQQ 与本地 NapCat 核心通过 `ws://127.0.0.1:3001` 进行双向 JSON-RPC 通信。

### 5.1 接收事件结构（Inbound Events）

#### 1. 私聊消息事件 (`message.private`)
```json
{
  "time": 1774176000,
  "self_id": 23456789,
  "post_type": "message",
  "message_type": "private",
  "sub_type": "friend",
  "message_id": 10001,
  "user_id": 12345678,
  "message": [
    { "type": "text", "data": { "text": "你好，请把这份技术方案发我一下" } }
  ],
  "raw_message": "你好，请把这份技术方案发我一下",
  "font": 0,
  "sender": {
    "user_id": 12345678,
    "nickname": "张三"
  }
}
```

#### 2. 群聊消息事件 (`message.group`)
```json
{
  "time": 1774176005,
  "self_id": 23456789,
  "post_type": "message",
  "message_type": "group",
  "sub_type": "normal",
  "message_id": 10002,
  "group_id": 987654321,
  "user_id": 87654321,
  "anonymous": null,
  "message": [
    { "type": "at", "data": { "qq": "23456789" } },
    { "type": "text", "data": { "text": " 下周开会时间定了吗？" } }
  ],
  "raw_message": "[CQ:at,qq=23456789] 下周开会时间定了吗？",
  "font": 0,
  "sender": {
    "user_id": 87654321,
    "nickname": "李四",
    "card": "产品-李四",
    "role": "member"
  }
}
```

#### 3. 群文件上传通知 (`notice.group_upload`)
```json
{
  "time": 1774176010,
  "self_id": 23456789,
  "post_type": "notice",
  "notice_type": "group_upload",
  "group_id": 987654321,
  "user_id": 87654321,
  "file": {
    "id": "/12345678-abcd-1234-abcd-1234567890ab",
    "name": "需求规格说明书v2.docx",
    "size": 1048576,
    "busid": 102
  }
}
```

### 5.2 核心调用动作列表（Outbound Actions）

EazyQQ 向 NapCat 发送的标准动作请求结构如下：
```typescript
export interface OneBotAction<P = Record<string, unknown>> {
  action: string;
  params: P;
  echo: string; // 唯一 UUID，用于请求与响应回调匹配
}
```

#### 常用动作定义：
1. **`get_login_info`**：获取已登录账号信息（QQ 号、昵称）。
2. **`get_friend_list`**：全量拉取好友列表。
3. **`get_group_list`**：全量拉取已加入的群聊列表。
4. **`get_group_member_list`**：拉取指定群的全部成员列表。
5. **`send_private_msg`**：发送私聊消息（参数：`{ user_id: number; message: MessageSegment[] }`）。
6. **`send_group_msg`**：发送群聊消息（参数：`{ group_id: number; message: MessageSegment[] }`）。
7. **`delete_msg`**：撤回已发送消息（参数：`{ message_id: number }`）。
8. **`get_group_root_files`**：获取指定群的文件根目录列表。
9. **`get_group_files_by_folder`**：获取指定群特定子目录下的文件列表。
10. **`get_group_file_url`**：解析获取群文件的 HTTP 下载直链地址。

---

## 6. 本地数据访问层 DAO 契约（Drizzle ORM Repository）

主进程使用 Drizzle ORM 对 SQLite 数据库进行原子事务管理与访问，所有数据访问均封装为单一职责 Repository 接口：

### 6.1 `IContactRepository`
```typescript
export interface IContactRepository {
  upsertContact(contact: NewContactEntity): Promise<void>;
  getContactById(id: string): Promise<ContactEntity | null>;
  listContacts(filter?: { type?: 'friend' | 'group'; keyword?: string }): Promise<ContactEntity[]>;
  updateLastMessage(id: string, snippet: string, timestamp: number): Promise<void>;
}
```

### 6.2 `IRoutingRuleRepository`
```typescript
export interface IRoutingRuleRepository {
  getRuleByTargetId(targetId: string): Promise<RoutingRuleEntity | null>;
  saveRule(rule: NewRoutingRuleEntity): Promise<void>;
  batchUpdateMode(targetIds: string[], mode: string): Promise<number>;
  listActiveRules(): Promise<RoutingRuleEntity[]>;
}
```

### 6.3 `IDraftRepository`
```typescript
export interface IDraftRepository {
  createDraft(draft: NewDraftEntity): Promise<DraftEntity>;
  getDraftById(id: string): Promise<DraftEntity | null>;
  listPendingDrafts(): Promise<DraftEntity[]>;
  updateDraftContent(id: string, content: string, thinking?: string): Promise<void>;
  markStatus(id: string, status: 'sent' | 'dismissed'): Promise<void>;
  clearExpiredDrafts(beforeTimestamp: number): Promise<number>;
}
```

### 6.4 `IGroupFileRepository`
```typescript
export interface IGroupFileRepository {
  upsertFiles(files: NewGroupFileEntity[]): Promise<void>;
  listFilesByGroup(groupId: string, folderId?: string): Promise<GroupFileEntity[]>;
  updateDownloadStatus(fileId: string, status: 'downloading' | 'downloaded', localPath?: string): Promise<void>;
  saveExtractedSummary(fileId: string, summary: string): Promise<void>;
}
```

---

## 7. 透明文件系统数据交互规范（`EazyQQ_Data` JSONL & Markdown）

为了使外部 AI、统计脚本及用户无需启动数据库即可直接感知并处理数据，EazyQQ 维护一套强格式规范的文件系统结构。

### 7.1 工作区全景目录结构

```
EazyQQ_Data/
├── groups/
│   └── 研发团队_987654321/
│       ├── messages.jsonl          # 增量追加的会话消息流水（行式 JSON）
│       ├── summaries/              # 定期与按需生成的 Markdown 总结简报
│       │   ├── 2026-09-21-今日总结.md
│       │   └── 2026-09-20-技术研讨纪要.md
│       └── files/                  # 群文件全量同步目录
│           ├── 接口文档v1.pdf
│           └── 架构图.png
├── friends/
│   └── 张三_12345678/
│       ├── messages.jsonl
│       └── files/
└── logs/                           # 按日滚动的系统与诊断日志
    ├── eazyqq-2026-09-21.log
    └── crash.log
```

### 7.2 会话流水行格式定义（`messages.jsonl` Schema）

文件由单行独立的合法 JSON 字符串构成，支持外部工具通过按行流式扫描：

```typescript
export interface JsonlMessageRow {
  id: string;                      // OneBot 消息 ID
  targetId: string;                // "group:987654321" 或 "friend:12345678"
  senderId: string;                // 发送者 QQ 号
  senderName: string;              // 发送者昵称或群名片
  content: string;                 // 去除冗余控制符的纯文本内容
  isAtMe: boolean;                 // 是否 @ 了本账号
  hasAttachment: boolean;          // 是否包含文件、图片或链接
  timestamp: number;               // Unix 时间戳（秒）
  isoTime: string;                 // ISO 8601 格式时间字符串
}
```

### 7.3 总结简报文件规范（`summaries/*.md` Schema）

每份简报头部包含 YAML Frontmatter 元数据，正文采用层级明确的结构化 Markdown：

```markdown
---
id: "summary_987654321_1774176000"
targetId: "group:987654321"
targetName: "研发团队"
timeRangeStart: "2026-09-21T09:00:00.000Z"
timeRangeEnd: "2026-09-21T18:00:00.000Z"
messageCount: 142
generatedAt: "2026-09-21T18:05:00.000Z"
modelUsed: "opencode-default"
---

# 研发团队 群聊重点速览 (2026-09-21)

## 核心要点
- 下周二上午10:00召开整体架构评审会议。
- 前端组件库全量切至 TailwindCSS v4，规范已更新。

## 决议与行动项
- [ ] @张三 完成 SQLite WAL 模式写入压测（截止周一）。
- [ ] @李四 提交 Preload 上下文桥接强类型代码。

## 提及文件
- [接口文档v1.pdf](file:///B:/EazyQQ_Data/groups/研发团队_987654321/files/接口文档v1.pdf)
```

---

## 8. 契约测试与验证方法（Contract Verification）

为确保全套 API 契约在后续迭代中绝对不发生隐式破坏，测试套件须包含：
1. **IPC 契约对齐测试**：验证 `preload/index.ts` 暴露的通道名与 `src/main/ipc/*.ts` 注册的处理器 100% 一一对应；
2. **DTO 边界校验测试**：使用 `zod` 对所有 IPC 请求参数在主进程入口处执行静态与运行时双重验证；
3. **OpenCode 模拟桩测试**：通过模拟 SSE 流帧（`thinking`、`text`、`done`）验证打字机与思考链解析的稳健性；
4. **JSONL 幂等性测试**：验证在极高频消息写入时，`messages.jsonl` 不发生跨行错位、文件句柄泄漏或脏数据注入。
