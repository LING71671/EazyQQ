# EazyQQ

零命令行的个人 QQ 智能助手客户端。Tauri 2 (Rust) + React 19 + Tailwind v4，本地 SQLite 持久化，通过 NapCat (OneBot 11) 接管真实 QQ 会话。

---

## 当前状态（v0.10）

| 阶段 | 内容 | 状态 |
| :--- | :--- | :---: |
| Phase 0 | 窗口外壳与视觉基调 | 已完成 |
| Phase 1 | 真实 QQ 协议与扫码鉴权 | 已完成 |
| Phase 2 | 联系人同步与双重白名单（默认拒绝） | 已完成 |
| Phase 3 | AI 草稿箱（生成 / 微调 / 真实发送） | 已完成 |
| Phase 4 | 群文件同步与文档综述 | 部分完成（下载受协议端版本阻塞） |
| Phase 5 | 定时群聊智能简报 | 已完成 |
| Phase 6 | 托盘常驻、健康自检、诊断包导出 | 已完成 |

### ⚠️ 两个已知阻塞（均非代码问题）

**1. GUI 窗口黑屏 → 白屏 → 卡死（必须处理，否则界面不可用）**

**1. 窗口白屏 / 黑屏 —— 已修复**

现象：窗口能创建、标题栏正常，但内容区只有一片背景色，随后变黑且不再响应。

根因（已用崩溃转储与对照实验确认）：**Chromium 的 GPU 子进程以访问违规（`0xC0000005`）死亡**，
浏览器主进程判定「GPU 不可用」后主动终止，渲染进程随之无法合成画面，窗口只剩配置的背景色 `#f8fafc`。

> 早先的结论是「Windhawk 注入导致」，**该结论已被证伪**：机器上有 19 个正常工作的 `msedgewebview2.exe`，
> 其中 9 个同样加载了 `windhawk.dll`（连 Windhawk 的 mod DLL 也注入了）；且崩溃栈上 95~99 个返回地址
> **全部落在 `msedge.dll` 内，指向 `windhawk.dll` 的为 0**。因此无需再对 Windhawk 做任何排除操作。

已采用的修复（写入 `tauri.conf.json`，无需用户操作）：

```json
"additionalBrowserArgs": "--no-sandbox"
```

逐参数隔离测试表明：`--no-sandbox` 单独使用即可恢复正常，而 `--disable-gpu`、
`--use-angle=swiftshader`、`--disable-gpu-compositing`、`--disable-features=Vulkan` **均无效**，
说明问题出在**沙箱/进程环境**，而非显卡驱动或渲染后端。

**安全权衡（需知晓）**：`--no-sandbox` 会关闭 Chromium 的渲染进程沙箱。本应用只加载本地打包内容、
不浏览任意网页，风险相对可控，但**这仍是一项真实的安全降级**，应视为临时规避手段。
建议后续**临时完全停用 Windhawk（需管理员）复测**：若停用后无需该参数即可正常，则应改为保留排除方案，
而不是长期关闭沙箱。完整排查过程见 [ISSUE.md](ISSUE.md) 的 ISSUE-013。

**2. 群文件下载：需要 NapCat ≥ 4.18.28**

NapCat 的 packet 机制依赖一张「QQ 版本 → 偏移」映射表，**QQ 版本超出表的上限就无法使用 packetBackend**，
群文件下载会失败（消息收发、草稿、简报不受影响）。

| NapCat | 偏移表 x64 上限 |
| :--- | :--- |
| 4.9.81（原先安装的版本） | 9.9.25-42941 |
| **4.18.28（当前，已升级）** | **9.9.35-52892** |

**本机已完成升级**：`B:\EazyQQ\napcat` 已从 4.9.81 更新到 4.18.28，偏移表由 53 个版本扩展到 70 个，
本机的 `A:\NTQQ`（9.9.28-46928）现已落在表内，**无需降级 QQ**。原 `config/` 已保留，
并留有备份 `config.backup-<时间戳>`。

判断当前 QQ 版本是否被支持（NapCat 运行后）：

```bash
grep "偏移数据" B:/EazyQQ/napcat/logs/*.log | tail -1     # 4.18.28 起日志改为输出到 QQ 控制台
grep -oE "9\.9\.[0-9]+-[0-9]+-x64" B:/EazyQQ/napcat/napcat.mjs | sort -u | tail -3
```

---

## 关于 Chromium GPU 进程崩溃（影响本机所有 Chromium 系程序）

本机存在一个**机器级**问题：Chromium 的 GPU 子进程无法启动，报
`GPU process exited unexpectedly` 并在重试数次后以
`FATAL: GPU process isn't usable. Goodbye.` 终止宿主进程。

这**不是 EazyQQ 的缺陷**——QQ 自身（Electron）会打印完全相同的日志并以同样方式死亡。
两者的表现分别是：

- EazyQQ（WebView2）：窗口先白后黑，随后界面才加载出来；
- QQ：进程直接退出（这也是 NapCat 启动后 QQ 消失的原因）。

EazyQQ 侧的规避手段是 `tauri.conf.json` 中的 `"additionalBrowserArgs": "--no-sandbox"`
（逐参数验证过：`--disable-gpu`、`--use-angle=swiftshader`、`--disable-gpu-compositing`、
`--disable-features=Vulkan` 均无效，只有 `--no-sandbox` 生效，说明问题在**沙箱/进程环境**而非显卡驱动）。

**这是真实的安全降级**（关闭了渲染进程沙箱），建议按以下顺序从根上解决：

1. **临时完全停用 Windhawk 后复测**——若不再需要 `--no-sandbox`，则应改用排除方案而非长期关闭沙箱；
2. **禁用 `GameViewer Virtual Display Adapter`**（虚拟显示器驱动，Chromium GPU 崩溃的经典诱因；
   本机适配器为 NVIDIA RTX 5060 Laptop / Intel UHD / GameViewer 虚拟显示器）；
3. 更新或重装显卡驱动；
4. 以上均无效时，再为 QQ 与 EazyQQ 统一关闭 Chromium GPU 加速。

确认后请相应调整 `tauri.conf.json` 的 `additionalBrowserArgs`。

---

## 快速开始

### 前置

1. **NapCat 协议端**运行中，且 QQ 已登录（HTTP `127.0.0.1:3000`、WebSocket `127.0.0.1:3001`、WebUI `127.0.0.1:6099`）。
2. **大模型 API Key**（默认使用 TokenRhythm），写入 `app_settings.tokenrhythm_api_key` 或环境变量 `TOKENRHYTHM_API_KEY`。

### 图形界面

```powershell
pnpm install
pnpm tauri dev
```

### 命令行通道（推荐，无需点击界面）

```bash
cd src-tauri
cargo build --bin eazyqq_cli
./target/debug/eazyqq_cli.exe            # 查看全部命令
```

常用命令：

```bash
# 协议与登录
eazyqq_cli status                         # 协议进程 / 登录态一览
eazyqq_cli login-info                     # 当前登录账号
eazyqq_cli quick-login-list               # 列出可免扫码登录的账号（含真实头像）
eazyqq_cli quick-login --uin <QQ号>       # 免扫码快速登录
eazyqq_cli qr --save qr.txt               # 获取真实登录二维码

# 大模型供应商（省 token 可切到本地端点）
eazyqq_cli ai-config                      # 查看实际生效的供应商 / 端点 / 模型
eazyqq_cli ai-set --provider ollama       # 切到本地 Ollama（免 Key、不走云端）
eazyqq_cli ai-set --provider openai --base-url http://127.0.0.1:1234/v1 --model local-model
eazyqq_cli ai-test                        # 对当前供应商发起真实连通测试

# 联系人与白名单（默认全部拒绝）
eazyqq_cli contacts                       # 同步真实好友/群并列出规则（含触发条件与冷却）
eazyqq_cli contacts --type group --search EazyQQ
eazyqq_cli rule --target 1104661022 --mode copilot --summary on --interval-hours 2
eazyqq_cli rule --target 1104661022 --trigger at_me          # 仅 @ 我时才响应
eazyqq_cli rule --target 1104661022 --trigger keyword --keywords "进度,排期"

# 消息与 AI
eazyqq_cli send --target 1104661022 --text "你好"       # 真实发送
eazyqq_cli ask --target 1104661022 --text "对方问：进度如何？"  # 生成候选回复（不发送）
eazyqq_cli drafts                                        # 待审核草稿
eazyqq_cli draft-regenerate --id <草稿ID> --instruction "再简短些"
eazyqq_cli draft-send --id <草稿ID>                      # 放行并真实发送

# 群聊简报
eazyqq_cli summarize --target 1104661022 --hours 6       # 立即生成真实简报
eazyqq_cli scheduler-tick                                # 强制执行一次定时调度
eazyqq_cli whitelist-groups

# 群文件
eazyqq_cli files --target <群号>
eazyqq_cli file-summarize --path <本地文件路径>            # 支持 txt/md/docx/xlsx/pptx

# 诊断
eazyqq_cli health --deep                                 # 全链路自检（含真实模型调用）
eazyqq_cli export                                        # 导出脱敏诊断包 zip
eazyqq_cli log-tail --lines 100

# 调试后门：注入模拟消息，走完整处理链路（无需第二个账号）
eazyqq_cli simulate --target 1104661022 --text "测试内容" --sender-name "测试者"
eazyqq_cli simulate --target 1104661022 --text "连发" --repeat 3   # 同进程连发，可验证回复冷却
```

所有命令都支持 `--json`，便于脚本消费。

### 回复触发与冷却

白名单只决定「谁可以被处理」，触发条件决定「什么消息才值得响应」：

| 触发条件 | 行为 |
| :--- | :--- |
| `all` | 任何消息都触发（私聊默认） |
| `at_me` | 仅当消息 @ 到本账号（或 @全体成员）时触发（群聊默认） |
| `keyword` | 仅当命中配置的关键词时触发 |

`cooldown_seconds` 是同一会话两次回复之间的最小间隔，用于避免连发消息导致机器人刷屏。
未知或空的触发条件一律**拒绝触发**（fail closed），不会退化成「回复所有消息」。

---

## 安全模型（重要）

**默认拒绝。** 所有好友与群聊初始状态均为「直通忽略」，既不接管消息也不记录流水。

- **未入白名单的会话完全旁路**：不落库、不产生草稿、不调用大模型。
- 两条白名单**彼此独立**：
  - **消息接管白名单**：`mode` 设为 `auto_reply`（自动秒回）或 `copilot`（草稿审核）才会介入。
  - **群总结白名单**：`is_summary_whitelist` 独立开关 + 独立的 `summary_interval_hours` 周期。
- 安全边界**只由数据库规则决定**，代码中不存在任何写死的账号或群号。

---

## 构建与测试

> **Windows + Git Bash 注意**：Git Bash 自带的 `/usr/bin/link.exe` 会遮蔽 MSVC 链接器。请**不要**把 MSVC bin 目录前置到 `PATH`（该目录携带一套 CRT DLL，会遮蔽 System32 的系统库，导致测试二进制 `STATUS_ENTRYPOINT_NOT_FOUND`）。改用显式链接器路径：

```bash
cd src-tauri
export RUSTUP_HOME="A:/DevEnv/Rust/Rustup"
export CARGO_HOME="A:/DevEnv/Rust/Cargo"
export LIB="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44.35207\\lib\\x64;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\10.0.26100.0\\ucrt\\x64;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\10.0.26100.0\\um\\x64"
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44.35207\\bin\\Hostx64\\x64\\link.exe"

cargo build --bins      # GUI + CLI
cargo test --lib        # 单元测试
```

在 PowerShell 中直接使用 `pnpm tauri dev` 无需上述处理。

### 测试覆盖

`cargo test --lib` 覆盖安全关键与易错逻辑：

- 白名单策略真值表（默认拒绝、简报白名单不触发 AI、`enabled=false` 不执行）
- 消息链路不变量（未白名单零落库、我方消息不触发 AI、非消息事件忽略）
- 噪声过滤与刷屏去重
- 文件名净化（防路径穿越）
- ZIP 读写往返、CRC32 标准校验值、脱敏规则
- 调度配置解析（缺省回退、区间钳制、非法 JSON 不 panic）
- Office 文档 XML 文本抽取与实体解码

---

## 目录结构

```
src/                     前端 (React 19 + Tailwind v4)
  components/layout/     侧边栏、顶部标题栏（拖拽 + 窗口控制）
  components/common/     BootSplash（启动动画）、ErrorBoundary
  hooks/useWindowDrag    无边框窗口拖拽
  views/                 各功能页面
  api/                   与 Rust 通信的类型化客户端
src-tauri/src/
  lib.rs                 应用装配：日志、托盘、监听器、调度器
  commands.rs            暴露给前端的 IPC 命令
  services/
    policy.rs            ★ 唯一白名单策略出口
    ws_listener.rs       OneBot 事件处理（消息 / 通知）
    contacts.rs          名册同步（默认拒绝落库）
    summarizer.rs        真实简报生成
    scheduler.rs         配置驱动定时调度
    group_files.rs       群文件同步与文档综述
    document.rs          docx/xlsx/pptx/pdf 文本抽取
    archive.rs           最小 ZIP 读取器
    diagnostics.rs       脱敏诊断包（自实现 ZIP 写入）
    logging.rs           文件日志 + panic 崩溃报告
  bin/eazyqq_cli.rs      无界面控制通道
```

---

## 文档索引

| 文档 | 内容 |
| :--- | :--- |
| [ISSUE.md](ISSUE.md) | 20 条缺陷归档：现象、根因、修复、验证 |
| [ROADMAP.md](ROADMAP.md) | 阶段规划、验收标准、v0.10 交付说明 |
| [PRODUCT.md](PRODUCT.md) | 产品定位与需求 |
| [DESIGN.md](DESIGN.md) | 设计规范 |
| [docs/ARCHITECTURE_AND_DEV_SPEC.md](docs/ARCHITECTURE_AND_DEV_SPEC.md) | 架构与开发规范 |
| [docs/API_SPECIFICATION.md](docs/API_SPECIFICATION.md) | 接口契约 |
| [docs/SYSTEM_EXPANSION_AND_SAFETY_SPEC.md](docs/SYSTEM_EXPANSION_AND_SAFETY_SPEC.md) | 安全与扩展规范 |
