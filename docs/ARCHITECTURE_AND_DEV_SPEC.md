# EazyQQ：系统架构与全栈开发技术规格书

## 1. 系统概述与核心目标

**EazyQQ** 是一款面向完全不懂代码、只会用手机扫码的普通用户的开箱即用桌面端个人 QQ 智能助手。系统深度融合了腾讯 QQ 原生通信协议与现代大语言模型（LLM），核心目标与业务定位如下：

1. **零命令行、小白开箱即用（Zero-CLI Experience）**：
   - 用户无需安装 Python、.NET 运行时或 Docker，无需配置任何环境变量，直接双击 `.exe` 启动。
   - 自动检测并复用 Windows 系统中已安装的官方 QQ NT 核心环境，在桌面界面上以高清二维码展示，用户拿出手机 QQ 扫码即可完成登录与凭证自动持久化。
2. **四象限颗粒化消息路由策略（Granular Message Routing）**：
   - **全自动秒回模式（Auto-Reply）**：按触发规则（@我、关键词、全部消息）由 AI 全自动生成并发送回复。
   - **人机协同草稿审核模式（Copilot / Draft）**：AI 针对最新消息生成拟回复内容并置于客户端「草稿箱」中，由用户一键点击发送、微调编辑或驳回，杜绝 AI 乱说话。
   - **纯消息总结模式（Summary-Only）**：针对消息繁杂的高频群聊或联系人，AI 保持静默完全不发言，仅在后台收集聊天流，支持随时一键生成今日简报或定时输出结构化纪要。
   - **直通忽略模式（Ignore）**：普通聊天，AI 完全不介入，保持原生聊天状态。
3. **文件管理与 AI 智能文档解析（File Intelligence）**：
   - 提供群文件与私聊文件的可视化层级资源管理器，支持一键批量下载到指定本地目录。
   - 本地原生文档解析：提取 PDF、DOCX、TXT、Markdown、代码文件（Python、TS、C++ 等）及图片内容。
   - AI 一键智能摘要：自动生成文档长文概括、核心论点与待办行动项。
4. **本地优先与数据隐私（Local-First & High Performance）**：
   - 采用本地 SQLite + Drizzle ORM，所有聊天记录、规则配置、Prompt 模板均安全存储在用户本机，拒绝数据泄露。

---

## 2. 核心架构原则：低耦合、原子化、严禁硬编码

### 2.1 服务间严格解耦与控制反转（Inversion of Control & Event-Driven）
系统禁止编写任何高耦合的巨石代码（Monolithic Bloat）。核心业务划分为独立的原子化服务模块，通过强类型接口与事件总线（Event Bus）交互：

```
[ 协议适配层 (IProtocolAdapter) ] <--- Typed Events ---> [ 路由决策引擎 (IRuleEngine) ]
                                                                |
                                                                v
[ 本地持久化 (IStorageService) ] <--- Typed Events ---> [ AI 调度中枢 (IAIProvider) ]
                                                                |
                                                                v
[ 文件与文档服务 (IFileService) ] <--- Typed Events ---> [ 消息总结引擎 (ISummaryService) ]
```

- **协议层可替换性**：协议适配层仅暴露统一的 `IProtocolAdapter` 接口（登录、登出、二维码流、发消息、取群文件）。底层无论是使用 NapCat、Lagrange 还是未来的官方开放平台，上层的 AI 逻辑和前端界面无需改动一行代码。
- **大模型多源热插拔**：AI 层抽象出统一的 `IAIProvider` 接口，内置 `DeepSeekProvider`、`OpenAIProvider`、`ClaudeProvider`、`OllamaProvider` 及 `CustomCompatibleProvider`，各 Provider 独立封装流式传输与错误处理。

### 2.2 前端原子化组件架构（Atomic Design System）
- 前端严格按照原子设计体系划分：
  - `atoms/`：基础原子（高纯度无副作用组件：按钮、标签、开关、输入框、二维码渲染容器、状态指示灯）。
  - `molecules/`：组合分子（联系人卡片项、草稿操作条、文件行项目、模型选择器）。
  - `organisms/`：功能有机体（扫码登录弹窗、联系人规则配置面板、实时草稿流审核台、群文件管理器、总结简报流、参数设置中心）。
- 视图组件与状态完全解耦，状态逻辑统一收敛至原子化状态钩子（`useContacts`, `useRoutingRules`, `useDraftBoard`, `useFiles`, `useSummaries`, `useConfig`）。

### 2.3. **严禁硬编码与全参数动态可配置（Zero-Hardcoding）**：
   - **铁律**：代码中严禁出现任何未经声明的魔法数字（Magic Numbers）、固定端口、预设写死的系统提示词、固定模型标识或静态超时时间。
   - **集中类型化配置架构**：所有配置项均在 `config.schema.ts` 中通过 `Zod` 进行严格的模式定义，提供稳健的默认值。
   - **动态热重载与界面可视化**：所有参数均在客户端「设置面板」中分类暴露，用户修改后即时热重载生效，无需重启软件，并提供「测试连通性」与「恢复默认值」机制。
4. **一切函数均严格遵守单一职责原则（Single Responsibility Principle, SRP）**：
   - **函数粒度原子化**：一个函数只做一件事。严禁编写超过 40 行的“大包大揽”面条函数。
   - **纯函数优先与无副作用**：数据转换、校验、计算等逻辑均编写为纯函数（Pure Functions），无隐式外部依赖与可变状态；I/O 与副作用严格收敛在 Service 边界内。
5. **深度结构化目录体系（严禁文件无序平铺）**：
   - 代码文件按领域与职责分层，采用多级目录清晰隔离，杜绝在一个文件夹下堆积数十个文件的混乱结构。
6. **全链路结构化日志与一键开发者诊断打包（Structured Logging & Diagnostic Bundler）**：
   - 系统内置生产级分级日志（DEBUG / INFO / WARN / ERROR），自动对 API Key、密码与敏感 Token 进行正则脱敏。
   - 提供「一键导出诊断日志包」功能，小白点击后自动抓取系统环境、子进程输出与最近错误链，压缩为单个 `.zip`，便于直接发送给开发者排错定位。

### 2.4 强制性 SQLite 性能 PRAGMA 配置
所有连接本地 SQLite 数据库的初始化流程中，必须严格执行以下 PRAGMA，彻底消除数据库锁死（database is locked）及卡顿问题：
```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA cache_size = -64000;
PRAGMA busy_timeout = 5000;
```

---

## 3. 系统技术栈与运行拓扑

| 技术领域 | 选型技术 | 核心考量与规范 |
| :--- | :--- | :--- |
| **桌面端外壳** | **Tauri 2.0 (Rust 1.93+)** | 复用系统内置 Edge WebView2，**安装包仅 ~12MB（较传统方案轻 80%+）**，内存仅占 ~40MB，冷启动 0.3s。 |
| **前端框架** | React 19 + TypeScript + Vite 6 | 极致的 HMR 响应、原子化组件拆分与强类型契约。 |
| **样式与视觉** | TailwindCSS v4 (必选) | 严格遵循 Impeccable 设计规范，采用**白色清新通透质感（Light Mode First）**，深色可作为主题自由切换。 |
| **本地数据库** | `rusqlite` (内置静态编译 SQLite) | 原生 C/Rust 级别极速 I/O，规避 Node 原生模块跨平台编译风险，强制开启 WAL 模式。 |
| **底层协议引擎** | NapCatQQ (内置 OneBot 11) | 挂载官方 NTQQ 核心，**无需第三方签名服务器**，协议原生稳定，群文件 API 完备。 |
| **包管理器** | `pnpm` 专享 | 速度快、幽灵依赖拦截、确定性依赖构建。 |

---

## 4. 详细原子功能模块设计

### 模块一：小白零门槛扫码登录与自愈机制
- **执行流程**：
  1. 软件启动后，主进程自动探测 Windows 注册表与常见路径（如 `C:\Program Files\Tencent\QQNT\QQ.exe`），定位已安装的官方 QQ NT 路径。
  2. 启动内置 NapCat 核心守护进程，建立本地安全 WebSocket（默认端口 3001，带动态安全 Token）。
  3. 检测本地 `data/napcat` 是否已存在合法的 `session.token` 与 `device.json`：
     - 若有效：协议端自动静默恢复登录，前端直接平滑切入主界面；
     - 若无效/首次启动：主进程通过 OneBot/WebUI 接口获取最新登录二维码 Base64 字节流，通过 IPC 推送到前端界面。
  4. 前端展示精致的扫码卡片：
     - `等待扫码`：展示带刷新倒计时的高清二维码；
     - `已扫码待手机确认`：动态呼吸灯动效，提醒用户手机点击确认；
     - `登录成功`：平滑渐变转场进入控制台。整个过程完全无需看黑窗口或手动配置。

### 模块二：四象限消息规则评估引擎
当收到 QQ 新消息事件（`message.private` 或 `message.group`）时，系统严格执行原子化路由评估：

```
                      收到 QQ 新消息事件
                              |
               [ 从 SQLite 查询该联系人/群策略 ]
                              |
       +---------------+------+-------+---------------+
       |               |              |               |
 [ 自动回复模式 ] [ 人机草稿模式 ] [ 纯总结模式 ]   [ 直通忽略 ]
       |               |              |               |
 触发条件命中?    直接触发 AI      存入该群滚动      丢弃/仅作本地
 (@我/关键词/全)  生成拟回复       消息缓存池        会话日志留存
       |               |              |
 触发 AI 生成     呈现在前端       等待触发总结
       |          草稿箱待审       (定时/手动/阈值)
 调用 OneBot 发送 (一键发送/微调)
```

1. **自动秒回模式（Auto-Reply）**：
   - 触发条件支持灵活切换：仅当被 `@` 时、包含特定关键词（如“在吗”、“售价”、“帮助”）或对所有私聊消息全量生效。
   - 极速流式响应，支持配置按需冷却间隔避免密集消息堆叠。
2. **人机草稿模式（Copilot / Draft）**：
   - 适合高重要度私聊或商业沟通。AI 根据对话上下文生成得体的回复建议，实时推送到前端「待办草稿箱」。
   - 提供「一键发送」、「快捷微调」、「重新生成」与「忽略」操作。
3. **纯消息总结模式（Summary-Only）**：
   - 针对 500 人大群或水群。AI 绝不在群内发送任何消息，仅在本地按时间滑动窗口缓存群聊发言。
   - 支持三种触发方式：随时手动点击「生成近4小时简报」、设置每日固定时间自动输出（如每天 18:00 晚报）、或群未读累积超阈值（如满 100 条）自动聚合。

### 模块三：多模型供应源调度中枢（AI Engine）
- **支持的模型商**：
  - **本地 OpenCode 智能引擎（强推首选，免配 API Key，100% 对齐云端 API 能力）**：
    - **零门槛即用**：直接与宿主机已安装的 `opencode-ai` 无缝联动（基于 `@opencode-ai/sdk` 或 `opencode serve` / `opencode run`）。
    - **完全不需要小白用户去注册或购买任何 API Key**，全自动调用本机已经授权的大模型（Claude、GPT-4o、DeepSeek 等）或本地 Agent 沙箱。
    - **与云端 API 能力 100% 完全对齐（API Parity）**：
      - *打字机流式传输（Streaming）*：通过 OpenCode JSON 事件流或 SDK WebSocket 实现逐字流式打字机吐字，毫秒级响应；
      - *思考链深度展示（Thinking Blocks）*：原生支持深度思考过程流式提取（如 DeepSeek-R1 / Claude 的思维链），前端可展开/折叠查看思考逻辑；
      - *多轮会话上下文（Session Continuity）*：通过 OpenCode 的 `--session <targetId>` 机制，自动将每个 QQ 群或好友映射为独立的会话 Session，完美保持上下文记忆；
      - *随时中断与取消生成（Abort Controller）*：用户点击停止或撤回时，即时发送中断信号，停止生成并释放资源；
      - *人设与 Prompt 严格注入*：动态通过 `--prompt` 或系统指令注入当前联系人的自定义人设。
    - **自主执行与工具调用**：具备代码与复杂指令的自主执行能力，可自动挂载 `EazyQQ_Data` 工作区进行深度群文件分析与总结。
  - **DeepSeek**（极高性价比云端 API，支持 deepseek-chat 及 deepseek-reasoner R1）。
  - **OpenAI**（GPT-4o, GPT-4o-mini 等）。
  - **Anthropic Claude**（Claude 3.5 Sonnet 等）。
  - **本地离线 Ollama**（自动探测 `http://127.0.0.1:11434`，离线运行开源模型）。
  - **自定义兼容易语言/反代端点**（支持任意兼容 OpenAI API 规范的地址与模型）。
- **统一流式接口与上下文截断**：
  - 支持滑动窗口记忆管理（保留最近 N 轮对话，自动计算 Token 消耗），确保不超上下文上限。
  - 支持流式传输（Server-Sent Events），桌面端实时打字机效果呈现。

### 模块四：群与私聊文件资源管理器与文档智能
- **文件抓取能力**：
  - 调用 OneBot 11 提供的群文件标准接口：`get_group_root_files`（根目录文件）、`get_group_files_by_folder`（子文件夹文件）、`get_group_file_url`（下载直链）。
  - 私聊与群聊消息中的即时附件流捕获（自动解析 `[CQ:file]` 与 `[CQ:image]` 段）。
- **文件可视化管理**：
  - 呈现直观的文件树列表，展示文件名、文件大小、上传者昵称及时间戳。
  - 支持指定默认本地下载目录，异步多线程下载并附带进度条。
- **本地智能解析与摘要**：
  - 本地原生文本提取：针对 `.pdf`（基于 pdf-parse）、`.docx`（基于 mammoth）、`.txt` / `.md` / `.json` / `.csv` 及各种代码源文件。
  - 针对大文件实行分块摘要机制（Chunking & Map-Reduce），由大模型一键提炼出「执行摘要」、「核心要点」、「行动待办」。

---

## 5. 本地数据库 Schema 设计（Drizzle ORM + SQLite）

系统严格执行无外键约束性能损耗但保留完整字段索引的设计，核心数据模型定义如下：

```typescript
// src/common/db/schema.ts
import { sqliteTable, text, integer, index } from 'drizzle-orm/sqlite-core';

// 1. 联系人与群聊元数据表
export const contacts = sqliteTable('contacts', {
  id: text('id').primaryKey(), // 格式如 'friend:123456' 或 'group:987654'
  targetType: text('target_type').notNull(), // 'friend' | 'group'
  targetId: text('target_id').notNull(),
  name: text('name').notNull(),
  avatarUrl: text('avatar_url'),
  remark: text('remark'),
  updatedAt: integer('updated_at').notNull(),
});

// 2. 独立颗粒度路由规则表（与联系人严格 1:1 解耦外键）
export const routingRules = sqliteTable('routing_rules', {
  id: text('id').primaryKey(),
  targetId: text('target_id').notNull().unique(), // 关联 contacts.id
  mode: text('mode').notNull().default('ignore'), // 'auto_reply' | 'copilot' | 'summary_only' | 'ignore'
  triggerCondition: text('trigger_condition').notNull().default('at_me'), // 'all' | 'at_me' | 'keyword'
  keywords: text('keywords'), // 逗号分隔关键词
  systemPrompt: text('system_prompt'), // 该群/人的专属人设 Prompt
  modelId: text('model_id'), // 该群/人专属覆盖的大模型名称
  cooldownSeconds: integer('cooldown_seconds').default(5),
  enabled: integer('enabled', { mode: 'boolean' }).notNull().default(true),
});

// 3. 消息流与历史记录表
export const messages = sqliteTable('messages', {
  id: text('id').primaryKey(), // OneBot message_id
  targetId: text('target_id').notNull(),
  senderId: text('sender_id').notNull(),
  senderName: text('sender_name').notNull(),
  content: text('content').notNull(),
  rawJson: text('raw_json'),
  isFromMe: integer('is_from_me', { mode: 'boolean' }).notNull().default(false),
  aiReplyStatus: text('ai_reply_status').default('none'), // 'none' | 'auto_replied' | 'draft_pending' | 'summarized'
  timestamp: integer('timestamp').notNull(),
}, (t) => [
  index('idx_messages_target').on(t.targetId),
  index('idx_messages_timestamp').on(t.timestamp),
]);

// 4. 人工待审草稿表（Copilot 模式专用）
export const draftReplies = sqliteTable('draft_replies', {
  id: text('id').primaryKey(),
  targetId: text('target_id').notNull(),
  replyToMsgId: text('reply_to_msg_id').notNull(),
  generatedContent: text('generated_content').notNull(),
  status: text('status').notNull().default('pending'), // 'pending' | 'sent' | 'dismissed'
  createdAt: integer('created_at').notNull(),
});

// 5. 群与私聊文件元数据表
export const chatFiles = sqliteTable('chat_files', {
  id: text('id').primaryKey(), // OneBot file_id
  targetId: text('target_id').notNull(),
  fileName: text('file_name').notNull(),
  fileSize: integer('file_size').notNull(),
  fileUrl: text('file_url'),
  localPath: text('local_path'),
  downloadStatus: text('download_status').notNull().default('remote'), // 'remote' | 'downloading' | 'downloaded'
  aiSummary: text('ai_summary'),
  uploadedAt: integer('uploaded_at').notNull(),
});

// 6. 群消息总结与简报记录表
export const groupSummaries = sqliteTable('group_summaries', {
  id: text('id').primaryKey(),
  targetId: text('target_id').notNull(),
  summaryText: text('summary_text').notNull(),
  keyPoints: text('key_points'), // 结构化 JSON 字符串
  startTime: integer('start_time').notNull(),
  endTime: integer('end_time').notNull(),
  createdAt: integer('created_at').notNull(),
});

// 7. 集中系统参数与凭证配置表（键值对存储）
export const appSettings = sqliteTable('app_settings', {
  key: text('key').primaryKey(),
  value: text('value').notNull(),
});
```

---

## 6. 全参数可配置清单与模式定义（Zod Schema）

为彻底落实“严禁硬编码”的要求，所有系统参数均纳入统一模式治理：

```typescript
// src/common/config/config.schema.ts
import { z } from 'zod';

export const AppConfigSchema = z.object({
  // 1. 协议与网络底层配置
  protocol: z.object({
    wsHost: z.string().default('127.0.0.1'),
    wsPort: z.number().int().min(1024).max(65535).default(3001),
    httpPort: z.number().int().min(1024).max(65535).default(3000),
    accessToken: z.string().default(''),
    reconnectIntervalMs: z.number().int().min(1000).default(3000),
    maxReconnectAttempts: z.number().int().min(1).default(10),
    startupTimeoutMs: z.number().int().min(5000).default(30000),
    customQqPath: z.string().default(''), // 若为空则自动探测系统安装的官方 QQ NT
  }),

  // 2. AI 模型与推理参数
  ai: z.object({
    provider: z.enum(['opencode', 'deepseek', 'openai', 'claude', 'ollama', 'custom']).default('opencode'),
    apiKey: z.string().default(''),
    baseUrl: z.string().default(''),
    modelName: z.string().default('default'),
    opencodeServerUrl: z.string().default('http://127.0.0.1:4096'),
    temperature: z.number().min(0).max(2).default(0.7),
    topP: z.number().min(0).max(1).default(0.95),
    maxTokens: z.number().int().min(100).max(32000).default(2048),
    contextWindowRounds: z.number().int().min(1).max(50).default(12),
    streamResponse: z.boolean().default(true),
    requestTimeoutMs: z.number().int().min(5000).default(60000),
  }),

  // 3. 提示词模板全量可编辑
  prompts: z.object({
    defaultPersona: z.string().default('你是一个得体、干练且亲切的个人 QQ 专属智能助理。'),
    autoReplyTemplate: z.string().default('根据以下对话历史，以真实主人的语气直接回复：\n\n【上下文】：\n{context}\n\n【最新消息】：{message}'),
    copilotDraftTemplate: z.string().default('请为主人生成一条高情商、准确的拟回复草稿供其审核：\n\n【上下文】：\n{context}\n\n【对方最新消息】：{message}'),
    summaryPromptTemplate: z.string().default('深度分析以下群聊记录，提取核心结构化简报：\n1. 【讨论核心议题】\n2. 【达成的决策/结论】\n3. 【明确的待办事项(Todo)】\n4. 【重要文件与链接】\n\n聊天记录：\n{chatLog}'),
    fileSummaryTemplate: z.string().default('深度阅读并解析以下文件内容，提炼执行概要、核心要点与关键细节：\n\n文件名：{fileName}\n\n内容全文：\n{fileContent}'),
  }),

  // 4. 发送速率与交互间隔配置
  rateLimit: z.object({
    minTypingDelayMs: z.number().int().min(0).max(5000).default(500),
    maxTypingDelayMs: z.number().int().min(0).max(10000).default(1500),
    globalCooldownSeconds: z.number().min(0).max(60).default(1),
    perContactCooldownSeconds: z.number().min(0).max(300).default(2),
    botNicknames: z.array(z.string()).default(['小助手', 'Bot', '助手']),
    keywordDelimiter: z.string().default(','),
  }),

  // 5. 总结服务配置
  summary: z.object({
    defaultSlidingWindowHours: z.number().min(1).max(72).default(6),
    thresholdMessageCount: z.number().int().min(10).max(500).default(50),
    cronScheduleExpression: z.string().default('0 18 * * *'), // 默认每日 18:00
    autoGenerateOnThreshold: z.boolean().default(true),
    autoGenerateOnSchedule: z.boolean().default(true),
  }),

  // 6. 文件与文档处理配置
  files: z.object({
    downloadDirectory: z.string().default(''), // 默认到用户系统的 Downloads/EazyQQ
    maxAutoDownloadSizeBytes: z.number().int().default(50 * 1024 * 1024), // 50MB
    allowedExtensions: z.array(z.string()).default(['.txt', '.md', '.pdf', '.docx', '.csv', '.json', '.jpg', '.png']),
    maxTextExtractSizeBytes: z.number().int().default(5 * 1024 * 1024), // 5MB 文本提取上限
  }),

  // 7. 桌面端应用行为
  ui: z.object({
    theme: z.enum(['light', 'dark', 'system']).default('light'),
    minimizeToTray: z.boolean().default(true),
    startAtLogin: z.boolean().default(false),
    notifyOnDraftReady: z.boolean().default(true),
    notifyOnAutoReply: z.boolean().default(false),
    notifyOnSummaryReady: z.boolean().default(true),
    soundEffects: z.boolean().default(true),
  }),
});

export type AppConfig = z.infer<typeof AppConfigSchema>;
```

---

## 7. UI/UX 视觉体系与交互规范（Impeccable 标准：白色清新通透）

系统默认采用**白色清新、素雅通透的现代桌面质感（Light Mode First）**，深色模式作为可选主题随时可切换。整体视觉节奏轻盈、高对比度、清晰明快，杜绝任何压抑与沉闷感：

1. **三栏式清爽工作台布局**：
   - **左侧导航窄栏（64px）**：浅雾白底色（`#f1f5f9`），配精致的深灰/天青高亮图标与微光呼吸徽标，包含「会话与策略」、「实时草稿箱」、「群文件中心」、「智能简报流」、「设置中心」与「使用说明书 (F1)」。
   - **中间列表索引栏（300px）**：纯白底色（`#ffffff`）搭配超轻柔边框（`#e2e8f0`），群与好友列表清晰排布，右侧带有醒目清新的策略胶囊药丸：
     - `[自动秒回]`：薄荷青绿（Mint Emerald `#059669`）；
     - `[草稿待审]`：暖阳琥珀（Sun Amber `#d97706`）；
     - `[纯消息总结]`：鸢尾紫晶（Iris Violet `#7c3aed`）；
     - `[直通静默]`：温润柔灰（Muted Slate `#64748b`）。
   - **右侧主交互区（自适应宽度）**：
     - **底色背景**：珍珠云白（`#f8fafc`），悬浮卡片为纯白高亮（`#ffffff`），四周环绕细腻的柔和漫反射阴影（`shadow-sm` / `shadow-md`）。
     - **策略配置时**：清爽大卡片选项，选中时呈现电青色（`#0284c7`）细边框与微光浅色填充，点选即保存，零确认负担。
     - **文件中心时**：网格/树状自由切换，不同文件类型配以清新的饱和彩色图标，悬停呈现「AI 摘要解析」蓝色胶囊按钮。
     - **草稿审核时**：对话气泡呈现上下文，AI 草稿采用淡青高亮卡片（`bg-sky-50 border-sky-200`）优雅展示，底部醒目配置「一键发送 (Ctrl+Enter)」、「微调」与「丢弃 (Esc)」。
2. **调色板与材质规范（白色清新基调）**：
   - **主背景（Canvas）**：珍珠白 `#f8fafc`；
   - **卡片/容器（Surface）**：纯白 `#ffffff`，搭配 `backdrop-blur-md` 磨砂通透质感；
   - **边框线（Borders）**：轻柔浅灰 `#e2e8f0`，圆角标准 `rounded-xl` (12px)；
   - **文字阶梯（Typography）**：
     - 主标题/正文：沉稳深炭 `#0f172a`（高对比度，久看清晰不刺眼）；
     - 次级文本/说明：中灰 `#475569`；
     - 辅助提示/占位符：柔灰 `#94a3b8`；
   - **交互强调色（Accent Primary）**：晨曦蓝（Sky Blue `#0284c7` / `#0ea5e9`）；
   - **深色主题（Dark Mode Theme）**：作为可选项完整保留，随时可在右上角或设置面板一键切换为高级沉浸深色（`#090d16` 底色）。
3. **微交互与动效**：
   - 所有 Hover、展开与切页均具备 150ms ~ 200ms 的平滑贝塞尔过渡，扫码登录界面的二维码容器外侧环绕微弱的晨曦蓝呼吸光晕。

---

## 8. 全生命周期详细结构化日志与一键开发者诊断系统（Logging & Diagnostics）

### 8.1 结构化日志分级与轮转策略
系统采用生产级结构化日志体系（基于 `winston` 或 `pino`），文件日志按照日期与大小滚动切分（单个日志上限 10MB，自动保留最近 14 天）：
- `logs/app.log`：常规运行日志（包含 INFO/WARN/ERROR 详细上下文与调用栈）。
- `logs/protocol.log`：专门记录 NapCat 子进程的 stdout/stderr 输出、心跳探测与 WebSocket 状态更迭。
- `logs/audit.log`：敏感操作审计（包含规则变更、用户登录登出、AI 回复发送记录）。

### 8.2 自动敏感信息脱敏过滤器（Sanitizer）
日志向控制台或文件刷写前，统一经过脱敏流水线，确保用户隐私资产不泄露：
- **API Key 脱敏**：所有匹配 `sk-[a-zA-Z0-9]{20,}` 或配置中的 Key，脱敏为 `sk-****...***abcd`。
- **Token 与密码脱敏**：将 QQ 会话 Token、登录凭证哈希化或打码。
- **消息内容脱敏选项**：支持在设置中开启“隐私调试模式”，开启后聊天内容在日志中仅保留 `message_id`、长度与时间，不输出正文。

### 8.3 小白专属「一键导出诊断日志包」机制
当用户遇到问题时，他们不会使用终端查看日志，也不会翻找 AppData 隐藏路径。
- **功能入口**：客户端设置中心醒目展示「一键生成诊断日志包」按钮。
- **一键执行逻辑**：
  1. 自动收集最近 3 天的 `app.log`、`protocol.log`、系统平台版本（如 Windows 11 64位）、当前软件版本、内存占用及数据库 Schema 版本；
  2. 自动剔除数据库密码与未脱敏凭证；
  3. 打包生成带有时间戳的压缩包：`EazyQQ-Diagnostic-YYYYMMDD-HHmmss.zip`；
  4. 自动调用 Windows Explorer 打开并高亮选中该压缩包，同时界面提示：“已为您生成诊断日志包，请直接将此文件发送给开发者排查问题”。

---

## 9. 严禁平铺的深度结构化代码目录树规范（Project Hierarchy）

系统严格执行领域驱动设计（DDD）与关注点分离的分层架构，**严禁在一个目录下平铺堆放十几个以上文件**：

```
b:/EazyQQ/
├── .impeccable/                    # Impeccable 视觉与上下文元数据配置
├── docs/                           # 完整中文技术规格书、开源生态分析与安全规范
│   ├── ARCHITECTURE_AND_DEV_SPEC.md
│   ├── OPEN_SOURCE_ECOSYSTEM_ANALYSIS.md
│   └── SYSTEM_EXPANSION_AND_SAFETY_SPEC.md
├── resources/                      # 本地静态资源与嵌入式二进制产物
│   ├── icons/                      # 应用多尺寸图标
│   └── napcat/                     # 内置 NapCat NTQQ 协议引擎绿色执行体
├── src/
│   ├── common/                     # 主进程与渲染进程共享的无状态通用层
│   │   ├── config/                 # 集中 Zod 校验模式与默认参数
│   │   │   ├── config.schema.ts
│   │   │   └── default-config.ts
│   │   ├── constants/              # 全局常量定义（路由模式、状态枚举）
│   │   ├── db/                     # Drizzle 数据库 Schema 与迁移定义
│   │   │   ├── schema.ts
│   │   │   └── migrations/
│   │   ├── ipc/                    # 强类型 IPC 通道契约与 DTO
│   │   │   ├── channels.ts
│   │   │   └── contracts.ts
│   │   └── types/                  # 全局通用 TypeScript 类型定义
│   │
│   ├── main/                       # Electron 主进程核心逻辑
│   │   ├── core/                   # 窗口管理、生命周期、托盘与主入口
│   │   │   ├── app-lifecycle.ts
│   │   │   ├── main-window.ts
│   │   │   ├── system-tray.ts
│   │   │   └── index.ts
│   │   ├── ipc/                    # IPC 控制器与事件处理器
│   │   │   ├── handlers/           # 按业务域拆分的独立 IPC Handler
│   │   │   │   ├── auth.handler.ts
│   │   │   │   ├── contacts.handler.ts
│   │   │   │   ├── drafts.handler.ts
│   │   │   │   ├── files.handler.ts
│   │   │   │   └── settings.handler.ts
│   │   │   └── ipc-router.ts
│   │   └── services/               # 单一职责的原子化业务服务
│   │       ├── ai/                 # 大模型调度与多 Provider 适配器
│   │       │   ├── providers/      # 各厂商独立适配器
│   │       │   │   ├── deepseek.provider.ts
│   │       │   │   ├── openai.provider.ts
│   │       │   │   ├── claude.provider.ts
│   │       │   │   └── ollama.provider.ts
│   │       │   ├── context-builder.ts
│   │       │   └── ai.service.ts
│   │       ├── file/               # 文件抓取、下载与本地提取
│   │       │   ├── extractors/     # 文档解析器（按文件格式独立拆分）
│   │       │   │   ├── pdf.extractor.ts
│   │       │   │   ├── docx.extractor.ts
│   │       │   │   └── text.extractor.ts
│   │       │   ├── downloader.ts
│   │       │   └── file.service.ts
│   │       ├── logger/             # 结构化日志、脱敏器与诊断打包服务
│   │       │   ├── diagnostic-bundler.ts
│   │       │   ├── sanitizer.ts
│   │       │   └── logger.service.ts
│   │       ├── protocol/           # NTQQ 协议引擎守护与 OneBot 11 客户端
│   │       │   ├── process-supervisor.ts
│   │       │   ├── onebot-client.ts
│   │       │   └── protocol.service.ts
│   │       ├── routing/            # 消息规则评估与四象限流转
│   │       │   ├── evaluators/     # 独立评估策略
│   │       │   ├── rate-limiter.ts
│   │       │   └── routing.engine.ts
│   │       ├── storage/            # SQLite 本地持久化与 Repository 层
│   │       │   ├── repositories/   # 按表拆分的数据访问层
│   │       │   │   ├── contacts.repo.ts
│   │       │   │   ├── messages.repo.ts
│   │       │   │   ├── drafts.repo.ts
│   │       │   │   └── files.repo.ts
│   │       │   └── storage.service.ts
│   │       └── summary/            # 群消息滑动窗口缓存与简报调度
│   │           ├── buffer-manager.ts
│   │           ├── summary-scheduler.ts
│   │           └── summary.service.ts
│   │
│   ├── preload/                    # Electron 安全预加载桥梁
│   │   └── index.ts
│   │
│   └── renderer/                   # 前端渲染层 (Vite + React 19 + TailwindCSS)
│       ├── assets/                 # 静态样式、字体与插画
│       ├── components/             # 严格原子化组件库 (Atomic Design)
│       │   ├── atoms/              # 纯无副作用原子组件 (Button, Badge, Input, Switch, QRCode)
│       │   ├── molecules/          # 复合分子组件 (ContactCard, DraftRow, FileItem, ModelBadge)
│       │   └── organisms/          # 复杂业务有机体 (SidebarRail, ContactList, DraftBoard, FileTree)
│       ├── features/               # 独立业务功能视图页 (按模块严格隔离)
│       │   ├── auth/               # 扫码登录页
│       │   ├── contacts/           # 联系人与规则配置工作区
│       │   ├── drafts/             # 待审草稿箱视图
│       │   ├── files/              # 群文件智能浏览与摘要中心
│       │   ├── summaries/          # 今日简报与群历史纪要视图
│       │   └── settings/           # 参数配置中心 (分类标签式)
│       ├── hooks/                  # 原子化自定义业务状态 Hook
│       ├── stores/                 # Zustand 状态分片 (无巨石 Store)
│       ├── App.tsx                 # 根应用外壳与路由切换
│       ├── main.tsx                # React 挂载入口
│       └── index.css               # TailwindCSS 核心指令与主题变量
│
├── electron-builder.yml            # Windows 打包与便携安装配置
├── electron.vite.config.ts         # 主进程与渲染进程一体化构建配置
├── package.json                    # 依赖清单与 pnpm 脚本
├── tsconfig.json                   # 根 TypeScript 配置
└── PRODUCT.md                      # 产品真值定义书
```

---

## 10. 函数单一职责（SRP）与代码编写规范准则

为保证系统的极致稳定性、代码可读性与自动化单测覆盖，开发过程中强制执行以下编程准则：

1. **单函数职责纯粹化**：
   - 每个函数有且仅有一个明确做的事情。函数命名必须精准反映其动作（例如：`extractTextFromPdfFile` 绝不能在内部附带保存数据库的操作；`saveDraftToDatabase` 绝不能在内部附带大模型生成动作）。
   - 单个函数长度建议控制在 20 ~ 30 行以内，严禁编写超过 40 行的混合逻辑函数。
2. **纯函数优先（Pure Functions First）**：
   - 规则匹配计算、Token 消耗统计、文本段落切片、时间格式化、脱敏过滤等纯计算函数，必须严格实现为纯函数：相同输入永远输出相同结果，无隐式外部变量引用，便于以 100% 覆盖率进行单元测试。
3. **副作用显式化与隔离**：
   - 所有涉及文件系统 I/O、数据库操作、网络请求及子进程唤起的副作用代码，必须严格隔离在对应 `services/` 类的私有方法或专门的 Adapter 之中，严禁在 Controller/Handler 或 UI 组件内部直接拼写 SQL 或直接发 HTTP 请求。
4. **入参不可变性与强类型约束**：
   - 严禁对函数入参对象进行原地修改（In-place mutation），始终返回新创建的对象或深拷贝数据副本。
   - 所有函数入参与返回值必须标注明确的 TypeScript 类型，严禁滥用 `any`。

---

## 11. 启动前置依赖环境自检体系（Pre-flight Dependency Health Checker）

为了给完全不懂技术的小白用户提供最坚固的保障，EazyQQ 在主进程启动之初，会执行毫秒级的前置依赖环境全自动自检。整个过程由原子化的 `DependencyCheckService` 驱动：

```
                    应用冷启动 (App Boot)
                             |
             +---------------v---------------+
             | Pre-flight Dependency Scanner |
             +---------------+---------------+
                             |
         +-------------------+-------------------+
         |                   |                   |
  [ 1. QQ NT 检测 ]   [ 2. OpenCode 检测 ] [ 3. 端口与存储检测 ]
         |                   |                   |
  探测注册表与默认路径  校验 CLI / 连通性 / 模型   检查 3001/4096 端口占用
  (QQ.exe 存在与版本)  (opencode models 可用)   检查 EazyQQ_Data 读写权限
         |                   |                   |
         +-------------------+-------------------+
                             |
                  汇总自检状态结果 (Health Report)
                             |
             +---------------+---------------+
             | 全部检测通过 (All Green)?     |
             +-------+---------------+-------+
                 是  |           否  |
                     v               v
           直接静默平滑放行    进入「小白修复向导」面板
           进入扫码或主界面    (清晰中文指引，可一键切换或修复)
```

### 11.1 四项核心依赖自检矩阵（Check Matrix）

| 自检检查项 | 检查逻辑与指令 | 判定就绪标准 | 异常时的自动化应对与降级方案 |
| :--- | :--- | :--- | :--- |
| **1. 官方 QQ NT 环境** | 检查 Windows 注册表 `HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Tencent\QQNT` 及默认安装目录 `C:\Program Files\Tencent\QQNT\QQ.exe`。 | 找到合法的 NTQQ 二进制文件，版本支持注入。 | 弹出一键下载官方 QQ NT 安装包的引导按钮，提示“请先安装官方新版 QQ 后重试”。 |
| **2. OpenCode 智能引擎** | 执行 `where.exe opencode`，探测是否存在；并向 `http://127.0.0.1:4096` 探活或运行轻量 `opencode models` 检验授权状态。 | CLI 正常响应，已绑定模型授权，能正常推理。 | 若未安装或未授权，弹出卡片：“检测到本地暂未授权 OpenCode，您可以点击一键授权，或随时在设置中切换为 DeepSeek/OpenAI 云端 Key”。 |
| **3. 本地网络端口占用** | 尝试绑定或探测本地 `3001`（NapCat WebSocket）与 `4096`（OpenCode 默认端口）。 | 目标端口处于未被外部恶意占用的健康状态。 | 若端口被占，系统自动顺延（如 3001 -> 3002）并动态写入本地配置，用户全程零感知。 |
| **4. 本地数据工作区权限** | 在用户指定数据目录（`EazyQQ_Data/`）创建临时测试探针文件并读取销毁。 | 读写、删除权限均正常，磁盘剩余空间 > 500MB。 | 若遭遇权限阻断（如只读目录），自动回退至用户标准的 `Documents/EazyQQ/` 路径。 |

### 11.2 前端视觉呈现：极简「就绪扫描器」
- **小白零负担体验**：自检耗时通常在 200ms ~ 400ms 以内。自检通过时，界面不产生任何阻断，直接以平滑渐入动画进入主界面；
- **异常可视化**：仅当关键依赖缺失时，才以 Impeccable 极具美感的大卡片呈现“环境自检报告”，每个项目带有鲜明的指示灯（绿勾/琥珀警示），并附带人话解释与「一键修复/切换」按钮，彻底告别看不懂的调用栈报错。

---

## 12. 100% 全图形化防呆设计与内置交互式说明书（Foolproof UI & Built-in Manual）

系统完全摒弃任何多余的弹窗式强制新手引导（拒绝“下一步、下一步”的遮罩打扰），启动即直入工作台。系统依托**全链路防呆设计（Poka-Yoke）**规避误操作，并配备**随时一键查阅的内置图形化说明书**：

### 12.1 全链路图形化防呆机制（Poka-Yoke System）
1. **即时性合法输入约束**：
   - 端口设置只允许正整数输入，超出 `1024 ~ 65535` 范围实时标红警示并阻止保存；
   - API Key 粘贴时自动智能剔除首尾隐藏空格或制表符（`trim`）；
   - 滑动条（如温度、速率限制、抖动延迟）配备人话状态标签与安全边界限位，杜绝极端失控数值。
2. **破坏性动作双重防御确认**：
   - 「全量清空会话数据」、「清除历史文件缓存」、「批量切换全量联系人策略」均要求弹出高防呆阻断模态框，清晰说明影响范围并要求显式点击确认；
   - 消息发送按钮具备 1000ms 物理防抖节流，点击瞬间置为不可用并附带 Spinner，杜绝连击重发。
3. **关联属性动态置灰与自动联动（State Cascade）**：
   - 当某个群被标记为「直通忽略」时，该群专属的关键词配置、人设 Prompt 与模型选择器自动收起并半透明置灰，直观告知用户当前不生效；
   - 当检测到本地 OpenCode 未就绪且云端 API 凭证未填写时，主界面的「自动回复开启」开关自动锁定并悬浮提示：“需先配置 AI 供应源方可开启自动回复”。
4. **脏数据与未保存保护（Dirty State Shield）**：
   - 用户在参数面板做过任何修改后，若试图未保存切换侧边栏，系统自动弹出轻量弹窗提醒，支持「保存并切换」、「放弃修改」或「取消」。

### 12.2 内置图形化说明书与帮助中心（Built-in Help Center）
客户端左侧导航栏常设「使用说明书」图标（全局快捷键 `F1`），点击以原生内嵌抽屉形式展开结构化图文手册：
- **第一章：四象限策略与人设配置实战**（图文演示什么是自动秒回、什么是草稿审核、什么是纯消息总结、如何配置不同群专属的人设）；
- **第二章：本地 OpenCode 零配置免 API 互联手册**（讲解如何配合本地 OpenCode 实现免买 API Key 的极智体验）；
- **第三章：群文件自动同步与本地知识目录解读**（带截图指引用户在 Windows 资源管理器中找到原始 `messages.jsonl` 与 `files/` 的位置，并指导如何供其他 AI 联动）；
- **第四章：快捷键与高级自定义提示词技巧**（常用快捷键速查与如何给不同好友/群打造专属个性化 Prompt 指南）；
- **第五章：常见故障自检与一键报障**（如何一键生成诊断日志包直发开发者）。
- **全局浮动气泡解释（Contextual Help Tooltips）**：
  - 界面上每一个选项、滑块与开关右侧均内置灰度小问号 `?` 图标，鼠标悬停即以黑色轻量气泡输出白话解释：“这个设置是什么意思”、“建议保持什么值”、“改动可能带来什么影响”，彻底告别查阅外部文档的割裂体验。

---

## 13. 相关设计与规范文档索引（Documentation Suite Index）

- **业务定位与产品真理**：[PRODUCT.md](file:///b:/EazyQQ/PRODUCT.md)
- **Operate 设计系统与组件规范**：[DESIGN.md](file:///b:/EazyQQ/DESIGN.md)
- **完整 API 与数据交互契约规范书**：[API_SPECIFICATION.md](file:///b:/EazyQQ/docs/API_SPECIFICATION.md)
- **开源生态技术调研与对比**：[OPEN_SOURCE_ECOSYSTEM_ANALYSIS.md](file:///b:/EazyQQ/docs/OPEN_SOURCE_ECOSYSTEM_ANALYSIS.md)
- **守护进程与系统扩展安全规范**：[SYSTEM_EXPANSION_AND_SAFETY_SPEC.md](file:///b:/EazyQQ/docs/SYSTEM_EXPANSION_AND_SAFETY_SPEC.md)

