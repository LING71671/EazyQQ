# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack
Tauri 2.0 (Rust) + Vite 6 + React 19 + TypeScript + TailwindCSS v4 + rusqlite + OneBot 11 (NapCat NTQQ 核心)

## Users
完全零技术背景的普通用户、社群运营者、学生与办公族。他们只会使用手机 QQ 扫码登录，没有任何终端命令、JSON 配置文件或复杂机器人框架的调试经验，但渴望获得自主可控的个人 QQ AI 自动化管理、智能消息简报与群文件知识提取能力。

## Product Purpose
打造一款开箱即用、零门槛、高质感的个人 QQ 桌面端智能助手。将个人 QQ 消息与现代大语言模型（DeepSeek、OpenAI、Claude、Ollama 等）深度融合，提供基于单人与群聊维度的颗粒化控制：支持全自动秒回、人机协同草稿审核、只看不回纯消息总结，以及群文件一键下载与 AI 智能文档解析。

## Positioning
彻底打破传统机器人框架（如 NoneBot2、Koishi、Lagrange）重度依赖 Python/C# 编译环境、命令行配置及第三方签名服务器（SignServer）的高门槛壁垒。EazyQQ 采用全内置、原子化、低耦合架构，双击 `.exe` 即用，完全通过现代图形化界面完成扫码与策略配置，所有消息、规则与文件均在本地离线安全存储。

## Operating Context
- **运行环境**：Windows 桌面端应用，支持系统托盘后台静默驻留与前台窗口自由切换。
- **登录机制**：完全依靠手机 QQ 扫码认证；严禁任何手动输入密码或配置 Token 的繁琐操作。
- **通信拓扑**：主进程与内置 NapCat 核心通过本地 WebSocket / HTTP 通信；对外通过安全 HTTPS 请求大模型接口。
- **数据隐私**：所有会话历史、规则配置、Prompt 提示词模板与文件元数据均保存在本地 SQLite 数据库中。

## Capabilities and Constraints
- **零门槛扫码认证**：自动检测 Windows 系统官方 NTQQ 环境，内置协议引擎静默启动，界面实时渲染 Base64 登录二维码，支持超时自动刷新与本地 Token 会话持久化（下次启动免扫码）。
- **四象限颗粒化路由策略矩阵**：
  - `自动秒回（Auto-Reply）`：根据触发条件（@我、特定关键词、全部消息）由 AI 自主秒回。
  - `人机协同草稿模式（Copilot / Draft）`：AI 针对新消息自动生成拟回复草稿，呈现在桌面端草稿箱中，由用户一键审核、二次编辑或点击发送。
  - `纯消息总结模式（Summary-Only）`：针对消息繁杂的高频群聊或联系人，AI 保持静默不发任何消息，仅在后台收集聊天流，支持随时一键生成简报或定时输出结构化纪要。
  - `直通忽略模式（Ignore）`：普通聊天，AI 完全不介入。
- **群与私聊文件智能中心**：
  - 群文件树状层级浏览、一键批量下载至指定本地目录。
  - 智能文档解析：本地原生支持提取 PDF、DOCX、TXT、Markdown、代码文件与图片。
  - AI 一键摘要与问答：提取文档核心要点、行动项与要闻总结。
- **本地 OpenCode 智能中枢与 API 能力 100% 对齐**：
  - 首推免配 API Key 的本地互联模式，直接利用宿主机已安装的 `opencode-ai`。
  - 完整对齐云端 API 体验：毫秒级打字机流式输出（Streaming）、深度思考链过程展示（Thinking Blocks）、多轮独立会话记忆（Session 隔离）与动态人设 Prompt 注入。
- **开机启动毫秒级依赖环境自检（Pre-flight Health Checker）**：
  - 启动时自动静默自检官方 QQ NT 路径、OpenCode 就绪状态、端口冲突（3001/4096）与数据目录权限，全绿秒级放行，遇异常提供保姆级图文引导与一键降级方案。
- **高性能本地数据库**：Drizzle ORM + SQLite，强制启用 `WAL` 模式、`NORMAL` 同步与并发优化 PRAGMA。

## Brand Commitments
- **设计哲学**：严格践行 Impeccable 设计规范，默认采用**清爽通透的高级纯白视觉（Light Mode First）**，搭配微磨砂玻璃质感、细腻柔和的阴影与高对比度易读排版，深色模式（Dark Mode）作为可选主题可自由切换，彻底告别压抑沉闷。
- **语言体系**：用户交互界面全中文呈现，杜绝「WebSocket URI」、「Payload」等生涩技术术语，采用「收到消息时...」、「生成草稿供我审核」、「仅在后台总结今日讨论」等通俗语义。

## Evidence on Hand
- 基于 NTQQ OneBot 11 标准协议规范构建，经过严格验证的消息事件捕获与群文件 API 契约。
- 深度吸收 NapCatQQ、AstrBot 与现代 AI 桌面客户端的最佳工程架构实践。

## Product Principles
1. **零命令行保证（Zero-CLI Guarantee）**：用户凡是需要碰一次黑窗口、改一次 JSON 文件或配一次端口，设计即宣告失败。
2. **严禁硬编码与全参数可配置（Zero-Hardcoding & 100% Configurability）**：禁止在代码中写死任何参数、模型名、提示词或超时时间。所有配置均由 Zod Schema 统管，可在设置面板中动态修改并热重载。
3. **架构低耦合与原子化（Low Coupling & Atomic Modularity）**：协议层、规则层、大模型调度、文件服务与本地存储严格以独立契约接口解耦，具备高度可替换性。
4. **函数极致单一职责（Single Responsibility Principle, SRP）**：每个函数只做一件事，长度控制在 20-30 行，数据计算纯函数化，副作用严格隔离。
5. **深度结构化目录（Structured Hierarchy, No Flat Files）**：严禁无序平铺堆叠文件，采用清晰的多级分层与领域隔离。
6. **全链路可诊断日志与一键报障（Structured Logging & One-Click Diagnostic）**：内置自动脱敏的生产级日志体系，提供一键导出诊断包功能，方便小白用户直接发给开发者精准定位问题。
7. **100% 图形化防呆与内置说明书（Foolproof GUI & Built-in Manual）**：摒弃强制入门向导遮罩，直接进入工作台。全界面配备防呆限制（合法性约束、高危双重确认、状态智能联动、物理防抖），并常设内置交互式说明书（F1）与全局悬浮气泡提示（`?`）。
8. **确定性自主权（Predictable Autonomy）**：用户对 AI 拥有绝对的控制权，清晰区分全自动秒回与人工审核草稿。
9. **本地优先与极速性能（Local-First & High Performance）**：本地 SQLite 高并发读写，毫秒级响应，保护用户隐私。
