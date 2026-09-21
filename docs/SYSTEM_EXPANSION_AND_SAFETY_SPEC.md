# EazyQQ：系统扩展、多模态文档解析与工程自愈规格书

本规格书作为系统架构的深度补充，针对**进程异常自愈与网络容灾**、**超大文档分块与多模态智能解析**、**敏感凭证操作系统级加密与一键备份**、**强类型 IPC 通信契约**以及**全图形化防呆与内置说明书**进行全面规范设计。

---

## 1. 进程守护与心跳自愈体系（Process Watchdog & Auto Healing）

针对非技术小白用户，底层依赖的崩溃或网络闪断绝不能抛出看不懂的终端红字报错，系统必须具备全自动自愈与平滑重连机制：

```
       +---------------------------------------------+
       |            Electron 主进程 (守护核心)       |
       |  +---------------------------------------+  |
       |  |          进程状态机 (Supervisor)      |  |
       |  |  - PID 存活监控    - 崩溃日志收集     |  |
       |  |  - 内存泄漏防护    - 智能重启节流     |  |
       |  +-------------------+-------------------+  |
       +----------------------|----------------------+
                              | 心跳探测 Heartbeat Ping (每 5 秒)
                              v
       +---------------------------------------------+
       |             NapCat NTQQ 协议子进程          |
       |  - 独立沙盒运行，防止主界面假死             |
       |  - 本地 WebSocket 端口 3001 监听            |
       +---------------------------------------------+
```

### 1.1 守护核心自愈规则
1. **健康心跳监测**：
   - 主进程每隔 5 秒通过 WebSocket 发送 OneBot `get_status`。
   - 若连续 3 次超时或未响应，客户端连接状态自动置为 `DEGRADED`（降级维护），并在界面顶部显示温和的「连接维护中，正在重新恢复...」。
2. **指数退避重连算法（Exponential Backoff）**：
   - 遭遇网络中断时，依次按照 `1s, 2s, 4s, 8s, 16s, 30s` 的间隔自动重试，绝不频繁重连导致端口阻塞。
3. **子进程崩溃自动重启与保护**：
   - 若协议进程意外退出，主进程捕获异常代码并写入 `logs/crash.log`。
   - 5 分钟内最多自动重启 3 次。若反复崩溃，在 UI 弹出一键式诊断助手：「检测到 QQ 运行环境异常，点击一键修复与重新检测」。

---

## 2. 多模态图文与超大文件 RAG 摘要管线（Multimodal & Document Intelligence）

```
                  接收到 QQ 消息附件
                           |
             +-------------+-------------+
             |                           |
    文档类 (PDF, DOCX, TXT, 代码)  图片类 (JPG, PNG, GIF)
             |                           |
     本地文本提取器               当前选用模型是否支持视觉多模态?
  (pdf-parse / mammoth)                  |
             |                   +-------+-------+
    字符数 > 上下文窗口限制?      | 是            | 否
      |              |      直接将图片 Base64 本地 OCR 离线文字提取
   否 |           是 |      送入多模态模型   (Tesseract / 本地库)
      v              v      (GPT-4o/Claude)       |
 直接构建 Prompt  分块摘要管线                    v
   送入大模型     (Chunking & Map-Reduce)  将提取文本送入大模型
             \       /
              v     v
          输出结构化精华总结卡片
```

### 2.1 超大文档的分块摘要机制（Chunking & Map-Reduce）
- 当用户发来长达 100 页的 PDF 或巨型 Word 文档时，单次 Token 会超出模型上下文或耗费巨额费用。
- **解决方案**：系统自动将文本按语义段落切片（每块 2000 字符，保留 200 字符滑动重叠），各分块并行生成微摘要，最后由大模型生成总控摘要（Executive Summary）。

### 2.2 多模态视觉自适应
- 若用户配置了 DeepSeek VL、OpenAI GPT-4o 或 Claude 3.5 Sonnet，聊天中的截图、照片将无损转 Base64 供模型直接理解图片细节；
- 若选用纯文本模型（如纯文本版 DeepSeek-V3），系统自动调用内置轻量 OCR 引擎识别图片文字后再交由模型处理，小白完全无需手动打字转述。

---

## 3. 凭证安全加密与一键备份迁移（Credential Security & Data Migration）

### 3.1 Windows DPAPI 操作系统级凭证加密
- 严禁在数据库或本地明文存储用户的 DeepSeek API Key、OpenAI Token 等资产。
- 采用 Electron 原生 `safeStorage.encryptString()` 接口，底层直接调用 Windows 系统的 DPAPI（Data Protection API）。密钥与当前 Windows 用户系统账户硬件绑定，即使数据库文件被他人拷走，脱机环境也绝对无法解密出明文 Key。

### 3.2 一键式全量数据备份与还原
- 设置面板提供「导出备份」与「导入数据」按钮。
- 一键生成单个 `.eazyqq-backup` 归档包（内含规则配置、Prompt 模板预设、历史摘要库，自动剔除大文件与临时缓存），用户换电脑使用时，一键导入立即满血恢复所有策略。

---

## 4. 严格强类型 IPC 通信契约规范（Typed IPC Contracts）

主进程与渲染进程严格杜绝散落的 `ipcRenderer.send('string')`，全部基于强类型接口统一约束：

```typescript
// src/common/ipc/channels.ts

export interface IpcContract {
  // 协议与身份鉴权通道
  'protocol:get-status': () => {
    isConnected: boolean;
    loginStatus: 'unlogged' | 'waiting_scan' | 'scanned' | 'logged_in';
    qrcodeBase64?: string;
    qqNumber?: string;
    nickname?: string;
  };
  'protocol:refresh-qrcode': () => Promise<string>;
  'protocol:logout': () => Promise<void>;

  // 联系人与规则配置通道
  'contacts:get-list': () => Promise<ContactItem[]>;
  'contacts:update-rule': (rule: UpdateRoutingRuleDto) => Promise<void>;
  'contacts:batch-update-mode': (targetIds: string[], mode: RoutingMode) => Promise<void>;

  // 消息流与人工草稿箱通道
  'messages:get-history': (targetId: string, limit: number, offset: number) => Promise<MessageItem[]>;
  'drafts:get-pending': () => Promise<DraftItem[]>;
  'drafts:send': (draftId: string, modifiedContent?: string) => Promise<boolean>;
  'drafts:dismiss': (draftId: string) => Promise<void>;
  'drafts:regenerate': (draftId: string, customInstruction?: string) => Promise<DraftItem>;

  // 群与私聊文件管理通道
  'files:get-group-files': (groupId: string, folderId?: string) => Promise<GroupFileItem[]>;
  'files:download': (fileId: string, targetId: string) => Promise<{ progressChannel: string }>;
  'files:summarize-file': (fileId: string) => Promise<{ summary: string; keyPoints: string[] }>;

  // 智能简报与总结通道
  'summary:generate-now': (targetId: string, hours: number) => Promise<GroupSummaryItem>;
  'summary:get-history': (targetId: string) => Promise<GroupSummaryItem[]>;

  // 集中系统参数配置通道
  'config:get': () => Promise<AppConfig>;
  'config:update': (newConfig: Partial<AppConfig>) => Promise<AppConfig>;
  'config:test-ai-connection': (providerConfig: Partial<AppConfig['ai']>) => Promise<{ success: boolean; latencyMs: number; error?: string }>;
  'config:reset-defaults': () => Promise<AppConfig>;
  'config:export-backup': () => Promise<string>;
  'config:import-backup': (filePath: string) => Promise<boolean>;
}
```

---

## 5. 全图形化防呆设计与内置交互式说明书（Foolproof Design & Built-in Manual）

系统彻底杜绝弹窗式强制入门向导（不打扰用户直接切入工作台），所有功能操作 100% 呈现在图形化界面上，并配备全链路**防呆机制（Poka-Yoke）**与**随手可查的内置图形化说明书**：

### 5.1 全场景防呆设计矩阵（Foolproof Protections）
1. **输入合法性即时反馈**：
   - 端口号输入限制为数字，范围校验 `1024 ~ 65535`，实时检测冲突；
   - API Key、URL 填入时自动做空白字符裁剪（`trim`），并实时进行正则格式初步核验；
   - 模型温度滑动条限制在 `0.0 ~ 2.0`，附带直观标签：“精确严肃 (0.2)” -> “平衡 (0.7)” -> “发散创意 (1.2)”。
2. **高风险操作二次确认与冷却保护**：
   - 「清空聊天缓存」、「全量重置数据库」、「将所有群批量设为自动回复」等危险动作，弹出明确二次确认弹窗，并展示影响范围；
   - 消息发送按钮点击后自动锁定 1 秒并显示加载动效，杜绝用户手抖双击导致向 QQ 连续重复发送两条相同消息。
3. **状态智能互斥与置灰禁用（Smart Disablement）**：
   - 若某联系人策略被设为「直通忽略」，则其下方的关键词输入框、触发条件与自定义 Prompt 自动平滑置灰折叠，避免小白误认为设置已生效；
   - 若本地未检测到 OpenCode 且云端 API Key 为空，则「自动回复」开关处于锁定禁用状态，并带有醒目气泡提示：“请先在设置中配置 API Key 或启动本地 OpenCode”。
4. **全量设置变更防丢失守卫（Dirty State Guard）**：
   - 用户在参数设置页修改任何配置后，未点击保存即切换页面时，弹出轻量提醒：“当前修改尚未保存，是否保存后离开？”。

### 5.2 客户端内置「图形化说明书与帮助中心」
客户端左侧导航栏常设「使用说明书」图标（快捷键 `F1`），点击即可打开精心编排的图文手册：
1. **模块一：四象限策略图文解析**
   - 配合清晰图解与场景示例说明「自动秒回」、「草稿审核」、「纯消息总结」与「直通忽略」分别适用于什么场景（如工作群推荐纯总结、普通私聊推荐草稿模式）。
2. **模块二：本地 OpenCode 零配置联动指南**
   - 图文展示如何确认 OpenCode 就绪、如何让 OpenCode 自动作为本地大脑执行复杂群文件分析。
3. **模块三：群文件全量拉取与本地目录映射指南**
   - 指引用户如何在本地 `EazyQQ_Data` 目录中找到原始的 `messages.jsonl` 与 `files/`，以及如何让其他 AI 或本地开发工具读取该目录。
4. **模块四：快捷键与高级自定义提示词技巧**
   - 包含快捷键清单（`Ctrl+Enter` 快速发送草稿、`Esc` 驳回草稿、`F1` 唤出说明书）与 Prompt 高阶人设模板。
5. **全局字段悬浮微说明（Contextual Tooltips）**：
   - 界面上每一个开关、输入框、下拉选单旁边，均配备精致的 `?` 悬浮图标，鼠标悬停即弹出通俗白话解释、推荐默认值及修改影响，小白无需外部搜索即可完全看懂。
