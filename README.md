# EazyQQ

个人专属 QQ 智能助手桌面端与自动化控制系统。  
专为隐私自治、人机协同与外部 AI 智能体调度设计的双模运行架构。

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=flat-square)](LICENSE)
[![Framework](https://img.shields.io/badge/Frontend-React_19_%7C_Tailwind_CSS-38bdf8.svg?style=flat-square)](https://react.dev/)
[![Engine](https://img.shields.io/badge/Backend-Tauri_2_%7C_Rust-f97316.svg?style=flat-square)](https://tauri.app/)
[![Protocol](https://img.shields.io/badge/Protocol-OneBot_11_%7C_NapCat-6366f1.svg?style=flat-square)](https://napneko.github.io/)
[![Platform](https://img.shields.io/badge/Platform-Windows_10_%2F_11_x64-slate.svg?style=flat-square)](https://www.microsoft.com/windows)

---

## 核心设计与特性矩阵

| 核心维度 | 传统 QQ 自动化方案 | EazyQQ 工程实现 |
| :--- | :--- | :--- |
| 准入策略 | 全量监听上报，隐私易泄漏 | 严格 Default-Deny 默认拒绝，仅处理白名单对象 |
| 交互可靠性 | 大模型直接发群，容易翻车 | 引入 AI 草稿箱缓冲审查，支持人类微调与二次确认 |
| 群聊沉淀 | 人工爬楼消耗精力 | 配置化定时巡检，自动剔除水群噪声并提炼共识待办 |
| 进程自愈 | 协议端断线卡死需手动处理 | 8 节点健康监控持续嗅探，支持强杀僵尸进程一键拉起 |
| 控制通道 | 单一界面，难以二次扩展 | GUI 桌面端与 CLI 命令行双模并行，原生支持 JSON 管道 |

### 1. 严格双重独立白名单
系统在初始状态下对所有好友与群聊执行完全旁路静默，不落盘存储、不调用大模型。  
消息托管白名单与群聊简报白名单完全解耦，用户可精确指定单个对象的托管模式：手动放行、半自动草稿审核或全自动秒回。

### 2. 人机协同 AI 草稿审核
当白名单群聊收到提问或被提及触发时，系统在后台生成附带思考上下文的拟答草稿。  
用户可在桌面抽屉中直观查看草稿，输入补充修正指令触发重新推理，或直接确认发送，确保对外输出内容始终处于受控状态。

### 3. 定时群聊智能简报
内置轻量级调度引擎，支持按 1 小时至 24 小时或自定义分钟数周期性巡检。  
结合滑动时间窗口读取本地 SQLite 会话流水，自动滤除表情包、无意义刷屏与闲聊噪声，结构化输出核心议题、决议结论与待办清单。

### 4. 链路健康嗅探与进程自愈
系统持续对 8 个核心环节进行心跳与状态感知：
- NapCat 控制平面
- QQ 登录会话
- OneBot HTTP 接口
- OneBot WebSocket 事件总线
- 本地 SQLite 数据库
- 大模型推理端点
- 定时简报调度引擎
- 前端交互界面

检测到异常时，界面实时定位故障源，并支持调用系统底层能力关闭异常残留进程、重写引导加载代码并平滑重新拉起。

### 5. 协议端在线热升级与路径自适应
内置对 NapCat 协议核心的版本嗅探。支持在设置面板直接检索 NapNeko 官方 Release 通道，一键完成压缩包下载、覆盖部署与引擎重启。  
针对本地 QQNT 路径，支持通过系统注册表与运行进程动态嗅探，同时提供界面输入项以支持自定义指定非标准盘符路径。

---

## 系统完整架构

系统自上而下严格划分为表现层、IPC 路由层、领域服务层、底层协议引擎及物理持久化层。

```text
================================================================================
                                接入与展现层
================================================================================
  [ 桌面交互端 GUI ]                            [ 自动化命令行 CLI ]
  React 19 + TypeScript + Tailwind             eazyqq_cli.exe
  - 链路健康抽屉与自愈操作面板                  - 全指令原生输出结构化 JSON
  - 双白名单管理与会话规则配置                  - 供外部 AI Agent 与脚本调用
  - AI 草稿箱审核与简报聚合视图                - 支持无缝接入 MCP 工具服务生态
         │                                              │
         │ Tauri IPC Invoke                             │ 领域子命令
         ▼                                              ▼
================================================================================
                         后端 IPC 路由分层 (src-tauri/src/commands/)
================================================================================
  [ system ]    全局应用配置、依赖项巡检、诊断包导出、NapCat 核心热更新、窗口控制
  [ protocol ]  登录凭证探活、二维码获取、快速登录切换、会话安全注销
  [ chat ]      好友与群聊信息同步、会话记录查询、消息发送、群文件检索与解析
  [ ai ]        推理接口连通测试、模型列表动态嗅探、群聊简报生成与历史归档
         │
         │ 模块解耦调度
         ▼
================================================================================
                         Rust 领域服务层 (src-tauri/src/services/)
================================================================================
  protocol   :: NapCat 运行时托管、WinBootHook 动态注入、OneBot 双向通信
  ai         :: OpenAI 兼容客户端封装、OpenCode 免费通道嗅探、Prompt 装配
  security   :: Default-Deny 准入网关、请求冷却限制器、敏感词与消息触发器
  workflows  :: 8 节点全链路健康监测、定时简报调度器、群文档格式提取
  storage    :: SQLite WAL 数据库连接池、Schema 自动迁移、会话与规则读写
  identity   :: 基于机器硬件码与 QQ 号的双重隔离、多账号平滑切换与凭据重置
  infra      :: 跨进程日志分卷轮转、脱敏排障包打包、全局运行配置校验
         │                                              │
         │ DLL Hook 注入与 OneBot 协议交互              │ 物理数据落地
         ▼                                              ▼
┌─────────────────────────────────┐            ┌────────────────────────────────┐
│      本地 Windows 官方 QQ       │            │    数据持久化目录 EazyQQ_Data   │
│  - Tencent QQNT 官方主程序      │            │  accounts/<机器码>/<QQ号>/     │
│  - NapCat 协议注入运行时        │            │    - eazyqq.db (SQLite WAL)    │
│  - 3000 HTTP / 3001 WebSocket   │            │    - logs/ (脱敏运行日志)      │
│  - 6099 WebUI 控制接口          │            │    - group_files/ (群文件缓存) │
└─────────────────────────────────┘            └────────────────────────────────┘
```

---

## 模块组织架构

```text
EazyQQ/
├── src/                               # 前端工程源码
│   ├── api/                           # Tauri IPC 接口契约定义与强类型客户端
│   ├── components/                    # 通用功能组件，含健康抽屉与聊天抽屉
│   ├── views/                         # 核心页面视图：规则、草稿、文件、简报、设置
│   └── index.css                      # 现代化设计系统与样式标记
├── src-tauri/                         # Rust 原生后端工程
│   ├── src/
│   │   ├── bin/eazyqq_cli/            # 模块化命令行程序源码，按领域拆分子命令
│   │   ├── commands/                  # IPC 路由层，细分为 system, protocol, chat, ai
│   │   ├── services/                  # 7 大领域服务模块
│   │   ├── lib.rs                     # 应用生命周期管理与命令分发中心
│   │   └── main.rs                    # 桌面端程序主入口
│   ├── Cargo.toml                     # Rust 依赖声明与编译参数
│   └── tauri.conf.json                # 桌面窗口、托盘与安全权限配置
├── napcat/                            # 本地协议端运行时
├── docs/                              # 项目设计规范与开发目录索引
├── LICENSE                            # Apache 2.0 开源许可协议文本
└── README.md                          # 项目架构与使用说明
```

---

## 命令行与 AI 智能体集成

项目内置独立 CLI 工具，可通过终端交互、批处理脚本或直接作为工具供外部 AI Agent 调度。

### 核心操作指令

查询系统与协议运行状态：
```bash
cargo run --bin eazyqq_cli -- status --json
```

拉取好友与群聊列表：
```bash
cargo run --bin eazyqq_cli -- friends --json
cargo run --bin eazyqq_cli -- groups --json
```

发送好友私聊或群聊消息：
```bash
cargo run --bin eazyqq_cli -- send --to <好友QQ号> --text "消息正文" --type user
cargo run --bin eazyqq_cli -- send --to <群号> --text "消息正文" --type group
```

检索会话历史记录：
```bash
cargo run --bin eazyqq_cli -- history --to <目标ID> --limit 30 --json
```

处置 AI 待审草稿：
```bash
cargo run --bin eazyqq_cli -- drafts --json
cargo run --bin eazyqq_cli -- send-draft --id <草稿ID>
cargo run --bin eazyqq_cli -- dismiss-draft --id <草稿ID>
```

按时间窗口生成群聊总结：
```bash
cargo run --bin eazyqq_cli -- summary --to <群号> --hours 6 --json
```

---

## 编译与运行

### 环境准备
- Windows 10 或 Windows 11 操作系统，x64 架构。
- Node.js 运行时与 pnpm 包管理工具。
- Rust 工具链与 Visual Studio 2022 C++ 生成工具。
- 本地已安装腾讯官方 QQ 客户端。

### 开发与测试命令
安装前端依赖包：
```bash
pnpm install
```

启动桌面客户端开发环境：
```bash
pnpm tauri dev
```

执行前端类型静态检查：
```bash
pnpm typecheck
```

执行后端单元测试集：
```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

构建生产安装包：
```bash
pnpm tauri build
```

---

## 开源许可

本项目遵循 [Apache License 2.0](LICENSE) 协议开源。
