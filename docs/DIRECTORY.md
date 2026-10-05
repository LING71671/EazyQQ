# 开发目录索引

一份"这个目录里都是什么"的对照表。新上手或隔一段时间回来时先看这里，能省掉大量翻找。

图例：**源码** 手写并纳入版本管理 ｜ **产物** 可重建，删了不影响源码 ｜ **运行** 运行期产生的数据 ｜ **参考** 第三方资料 ｜ **工具** 编辑器／AI 助手配置

## 顶层

| 路径 | 类型 | 说明 |
| :--- | :--- | :--- |
| `src/` | 源码 | 前端（React + TypeScript）。`views/` 是页面，`components/` 是组件，`api/` 是后端契约与调用封装 |
| `src-tauri/` | 源码 | 后端（Rust）。`src/services/` 是各功能模块，`src/bin/eazyqq_cli/` 是模块化命令行入口 |
| `src-tauri/target/` | 产物 | Rust 构建输出，**约 13 GB**。`cargo clean` 可回收，重新构建需数分钟 |
| `node_modules/` | 产物 | 前端依赖，**约 164 MB**。`pnpm install` 可重建 |
| `dist/` | 产物 | 前端打包结果。`pnpm build` 生成；**开发模式下不使用**（走 Vite 服务器） |
| `docs/` | 源码 | 设计与接口文档：架构、API 规格、安全扩展、开源生态分析 |
| `tests/` | — | **空目录**，尚未使用 |
| `index.html`、`vite.config.ts`、`tsconfig.json`、`package.json` | 源码 | 前端工程配置 |
| `pnpm-lock.yaml`、`pnpm-workspace.yaml`、`.npmrc` | 源码 | 依赖锁定与仓库配置 |
| `.env.example` | 源码 | 环境变量样例（真实 `.env` 不入库） |

### 文档

| 文件 | 内容 |
| :--- | :--- |
| `README.md` | 总入口：功能、构建、命令行、隔离说明 |
| `DIRECTORY.md` | 本文件 |
| `PRODUCT.md` | 产品定位与目标用户 |
| `DESIGN.md` | 设计原则与交互约定 |
| `ROADMAP.md` | 阶段规划 |
| `ISSUE.md` | 问题档案：每条含现象、根因、修复与验证 |
| `AGENTS.md`、`CLAUDE.md`、`.cursorrules`、`.gemini/` | AI 助手的工作约定 |

## 运行期数据（不在版本管理内）

| 路径 | 说明 |
| :--- | :--- |
| `EazyQQ_Data/` | **你的全部个人数据**，按【机器码 / QQ 号】隔离。见下方结构 |
| `EazyQQ_Data/bootstrap.json` | 仅应用级：上次使用的账号、WebView 兼容开关 |
| `EazyQQ_Data/accounts/<机器码>/<QQ号>/` | 单账号数据：数据库、日志、群文件、诊断包 |
| `EazyQQ_Data/accounts/<机器码>/_unbound/` | 账号识别前的临时区，识别后会被归位并清理 |

> `EazyQQ_Data/` 里的 `eazyqq.db` 是规则、消息、草稿、简报的唯一存储；`logs/` 含聊天原文。
> **不要删除**，也不要把整个目录拷到另一台机器上当作备份——机器码变了就打不开了。

## 协议端

| 路径 | 说明 |
| :--- | :--- |
| `napcat/` | NapCat 协议后端，**约 122 MB，必需**。由安装脚本放入，不入库 |
| `napcat/config/` | NapCat 配置：`qq_path.txt` 指向 QQ 安装、`webui.json` 是 WebUI 端口与 token、`onebot11_<QQ号>.json` 是 3000/3001 端口 |
| `napcat/logs/` | NapCat 自身日志。**目录为空是重要线索**：说明它没来得及写下第一条日志 |
| `napcat/cache/qrcode.png` | 登录二维码。账号绑定后会被清理 |

## 参考与工具

| 路径 | 类型 | 说明 |
| :--- | :--- | :--- |
| `refer/` | 参考 | 第三方仓库快照（NapCatQQ、NapCatQQ-Desktop、NextChat、opencode），**约 417 MB**，仅供阅读 |
| `.workbuddy-ai/` | 工具 | 本项目的 AI 助手工作区：`memory/` 是工作记录，`tmp/` 是临时文件，`downloads/` 是安装包暂存 |
| `.cursor/`、`.agents/`、`.gemini/`、`.impeccable/` | 工具 | 各编辑器的规则与技能配置 |

## 哪些可以删

| 路径 | 可删 | 代价 |
| :--- | :--- | :--- |
| `src-tauri/target/` | 可以 | 下次构建从头编译，数分钟 |
| `node_modules/` | 可以 | `pnpm install` 重装 |
| `dist/` | 可以 | `pnpm build` 重新生成 |
| `.workbuddy-ai/tmp/`、`downloads/` | 可以 | 无（安装包如需重下） |
| `refer/` | 可以 | 失去本地参考资料，需重新克隆 |
| `EazyQQ_Data/` | **不可以** | 你的规则、消息、草稿、简报 |
| `napcat/` | **不可以** | 协议端本体，删了应用无法收发消息 |

## 命令行

日常排查用这三条，都在 `src-tauri/target/debug/` 下：

```bash
eazyqq_cli selftest          # 关键不变量自检（安全边界、触发、冷却、脱敏、路径包容性）
eazyqq_cli config-audit      # 找出「界面上能改、后端不读」的设置
eazyqq_cli chain-status      # 全链路体检，定位第一个断点
eazyqq_cli napcat-doctor     # 协议端起不来时，逐步骤指出卡在哪一步
```

支持 `--json`，便于脚本消费。
