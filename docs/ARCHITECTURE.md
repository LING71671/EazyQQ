# EazyQQ 系统架构规格书 (ARCHITECTURE.md)

## 1. 系统总体架构

EazyQQ 采用本地优先、高并发、前后端同构的桌面应用与无头智能体双模架构：
- **桌面 GUI 运行时**：基于 Tauri 2.0 (Rust) + React 19 (TypeScript, Vite, TailwindCSS v4)。
- **无头 Agent 引擎**：独立的编译二进制工具（`eazyqq_cli`），集成 21 个子命令及标准 JSON-RPC 2.0 stdio 模型上下文协议（MCP）服务端。

```text
+---------------------------------------------------------------------------------+
|                                外部交互接口                                     |
|   +--------------------------+   +------------------------------------------+   |
|   |  桌面端操作界面 (GUI)    |   |  外部 AI 智能体 (Cursor, Claude, AGY)    |   |
|   +------------+-------------+   +--------------------+---------------------+   |
+----------------|--------------------------------------|-------------------------+
                 | (Tauri IPC invoke/listen)            | (stdio JSON-RPC 2.0 / CLI)
+----------------v--------------------------------------v-------------------------+
|                                 宿主核心进程                                    |
|   +------------------------------------+  +---------------------------------+   |
|   | Tauri Commands 与事件总线          |  | eazyqq_cli 引擎与 MCP 服务端    |   |
|   | (system, protocol, chat, ai 域)    |  | (9 大 MCP 工具, 21 个 CLI 命令) |   |
|   +-----------------+------------------+  +----------------+----------------+   |
|                     \                                     /                     |
|                      v                                   v                      |
|   +-------------------------------------------------------------------------+   |
|   |                            领域服务层                                   |   |
|   |  - protocol  : OneBot 11 WS 客户端、NapCat 守护进程、心跳探测器        |   |
|   |  - security  : 默认拒绝路由决策引擎、防抖冷却器、触发词匹配器           |   |
|   |  - ai        : 多厂商模型调度客户端、SSE 实时流式解析器                 |   |
|   |  - workflows : 群聊长文智能提炼、群文件同步、本地文档提取器             |   |
|   |  - identity  : 机器指纹衍生计算、多账号沙盒隔离体系                     |   |
|   |  - infra     : 结构化日志追踪、敏感数据脱敏过滤器、诊断包生成器         |   |
|   +--------------------+------------------------------+---------------------+   |
|                        |                              |                         |
|                        v                              v                         |
|   +-----------------------------+   +---------------------------------------+   |
|   | 本地持久化 (SQLite WAL)     |   | NapCat OneBot 11 协议子进程           |   |
|   | - 规则、消息流水、草稿箱    |   | - 官方 NTQQ 原生内存注入 Hook 核心    |   |
|   | - 群聊简报、系统自动化配置  |   | - WS: 3001, HTTP: 3000, WebUI: 6099   |   |
|   +-----------------------------+   +---------------------------------------+   |
+---------------------------------------------------------------------------------+
```

---

## 2. 核心服务模块设计

### 2.1 协议通信层 (`src-tauri/src/services/protocol/`)
- **NapCat 进程管理**：负责管理 `NapCatWinBootMain.exe` 的生命周期，支持智能检测系统 QQNT 注册表与安装路径、异常崩溃后自动拉起与限频保护机制。
- **WebSocket 事件总线**：与底层维持 `ws://127.0.0.1:3001` 本地回环长连接，具备指数退避算法的自动断线重连保障。
- **全链路心跳监控**：每隔 15 秒轮询 NapCat 控制平面、OneBot HTTP、WebSocket 总线及 QQ 登录状态。

### 2.2 规则安全与路由引擎 (`src-tauri/src/services/security/`)
- **严格默认拒绝策略（Default-Deny）**：所有未配置的好友与群聊默认全部旁路静默。仅当目标对象被显式加入规则表时，才允许流水持久化或触发 AI 推理。
- **四种执行模式**：
  - `Auto-Reply（自动秒回）`：命中触发词或 @ 规则后立即生成并直接发送回复。
  - `Copilot（草稿箱）`：生成带置信度与思考过程的拟答建议，交由用户人工审核确认。
  - `Summary-Only（只读总结）`：仅记录会话流水，供手动或定时周期简报使用。
  - `Ignore（忽略）`：完全不予理睬，零计算与存储开销。

### 2.3 本地数据持久化 (`src-tauri/src/services/storage/`)
- **数据库引擎**：基于 `rusqlite` 的本地 SQLite，强制开启高并发 PRAGMA：
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA foreign_keys = ON;
  PRAGMA cache_size = -64000;
  PRAGMA busy_timeout = 5000;
  ```
- **自动迁移机制**：应用启动时自动检测并迁移全部表结构：`contact_rules`, `messages`, `pending_drafts`, `group_files`, `group_summaries`, `app_settings`。

### 2.4 机器与多账号沙盒隔离 (`src-tauri/src/services/identity/`)
所有运行期数据收敛在 `EazyQQ_Data/`，按机器指纹与登录 QQ 号严格分级隔离：
```text
EazyQQ_Data/
├── bootstrap.json                     # 应用全局状态（上次登录账号、系统配置）
└── accounts/
    └── <machine_id>/                  # 依据硬件标识 FNV-1a 计算的唯一机器指纹
        ├── _unbound/                  # 扫码绑定前的临时会话区
        └── <qq_uin>/                  # 独立账号沙盒
            ├── eazyqq.db              # 专属 SQLite 数据库
            ├── logs/                  # 轮转追踪日志
            ├── files/                 # 下载的群文件资产
            └── diagnostics/           # 一键导出的诊断压缩包
```

---

## 3. 无头 CLI 引擎与模型上下文协议 (MCP)

### 3.1 命令行系统 (`src-tauri/src/bin/eazyqq_cli/`)
- 包含 21 个子命令，完整覆盖扫码登录、收发消息、规则配置、总结提炼、群文件同步、系统配置与诊断等全部功能。
- 所有数据查询子命令均支持 `--json` 标志，原生支持管道脚本调用。
- 通过 `eazyqq_cli schema --json` 自省命令，向外部智能体导出全套子命令与 MCP 工具的 JSON Schema 规格。

### 3.2 标准 MCP 服务端 (`eazyqq_cli mcp`)
- 遵循 Anthropic Model Context Protocol 规范的 JSON-RPC 2.0 stdio 服务端。
- 向外部开放 9 项核心工具能力：
  - `send_message`: 发送私聊或群聊消息。
  - `get_chat_history`: 分页拉取历史聊天上下文。
  - `list_contacts`: 获取好友与群聊列表及当前生效的规则。
  - `trigger_group_summary`: 针对目标群聊执行滑动窗口长文简报提炼。
  - `extract_document`: 提取本地 PDF、DOCX、TXT 或 Markdown 文本。
  - `simulate_rule_match`: 模拟单条消息的规则匹配与路由分支。
  - `update_target_rule`: 动态调整特定对象的路由模式、触发条件与 Prompt。
  - `get_system_health`: 获取依赖链路健康度与端口状态。
  - `restart_napcat`: 平滑重启协议端底层进程。
- **标准输出无污染保障**：运行 MCP 模式时，所有后台 `tracing` 与系统日志强制重定向至日志文件或 `stderr`，确保 `stdout` 通道传输纯净的单行 JSON-RPC 响应报文。
