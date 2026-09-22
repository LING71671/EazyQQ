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

**处理方式：自适应降级（默认关闭，只在真的失败后才启用）**

`--no-sandbox` 能规避该问题，但它会关闭 Chromium 的渲染进程沙箱——**这是一项真实的安全降级**。
因此它**不写死在配置里**，否则等于让所有用户为一台机器的怪问题买单。实际机制：

1. 应用**始终以带沙箱的安全方式启动**（`tauri.conf.json` 中不含该参数）；
2. 若启动后 9 秒内前端仍未挂载，判定为「WebView 无法执行 JS」；
3. 此时**开启兼容模式**（写入 `app_settings.webview_compat_mode`）并**重启一次**；
4. 重启后从设置中读到该开关，才为 WebView2 追加 `--no-sandbox`；
5. 若已开启仍失败，则重载一次 webview，最后无论如何都会显示窗口并输出诊断——**不会无限重启**。

**效果**：健康机器永远不启用它；有问题的机器自动恢复，无需人工干预。

如需恢复沙箱（例如根因已修复）：

```bash
eazyqq_cli set-config --key webview_compat_mode --value false
```

逐参数隔离测试表明：`--no-sandbox` 单独使用即可恢复正常，而 `--disable-gpu`、
`--use-angle=swiftshader`、`--disable-gpu-compositing`、`--disable-features=Vulkan` **均无效**，
说明问题出**沙箱/进程环境**，而非显卡驱动或渲染后端。

**注意**：该现象不止影响 EazyQQ——本机的 QQ 自身（Electron）会打印完全相同的
`GPU process isn't usable. Goodbye.`。排查建议按此顺序：临时完全停用 Windhawk → 禁用虚拟显示器适配器
→ 更新显卡驱动。完整排查过程（含两次被推翻的结论）见 [ISSUE.md](ISSUE.md) 的 ISSUE-013。

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

## 关于 Chromium GPU 进程崩溃

**注意：该现象不止影响 EazyQQ。** QQ 自身（Electron）会打印完全相同的日志并以同样方式死亡：

```
GPU process exited unexpectedly        （重试数次）
FATAL: GPU process isn't usable. Goodbye.
```

两者表现分别是：EazyQQ（WebView2）窗口先白后黑、随后才加载界面；QQ 进程直接退出
（这也是 NapCat 启动后 QQ 消失的原因）。

> 关于成因，需要说明一个**已被自己推翻的结论**：曾判定为「机器级 GPU 故障」，
> 但用 Edge 做对照后发现——**Edge 带着沙箱、带着 GPU 跑得完全正常**。
> 所以本机的沙箱与 GPU 都没坏，故障只出现在**嵌入式 Chromium（WebView2 / Electron）**
> 且**由自动化 shell 启动**时。用户在交互式会话中自行启动是否受影响，仍需实测确认。
> 因此不要把它当成「这台机器坏了」来对待。

EazyQQ 侧的处理是**自适应降级**（默认关闭、失败后自动启用并记住），机制见上文「窗口白屏」一节。
如需从根上解决，建议按以下顺序排查：

1. **临时完全停用 Windhawk 后复测**——若不再需要 `--no-sandbox`，则应改用排除方案而非长期关闭沙箱；
2. **禁用 `GameViewer Virtual Display Adapter`**（虚拟显示器驱动，Chromium GPU 崩溃的经典诱因；
   本机适配器为 NVIDIA RTX 5060 Laptop / Intel UHD / GameViewer 虚拟显示器）；
3. 更新或重装显卡驱动；
4. 以上均无效时，再为 QQ 与 EazyQQ 统一关闭 Chromium GPU 加速。

确认根因已解决后，用 `eazyqq_cli set-config --key webview_compat_mode --value false` 关闭兼容模式即可恢复沙箱。

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

# 诊断与自检
eazyqq_cli napcat-doctor                                 # ★ 协议端启动路径诊断：逐步骤走查，指出第一个卡住的地方
eazyqq_cli selftest                                      # ★ 关键不变量自检（17 项：安全边界/触发/冷却/脱敏/容器）
eazyqq_cli config-audit                                  # ★ 找出「界面上能改但后端不读」的设置
eazyqq_cli chain-status                                  # ★ 全链路状态：逐环节体检并定位第一个断点
eazyqq_cli health --deep                                 # 依赖与链路自检（含真实模型调用）
eazyqq_cli export                                        # 导出脱敏诊断包 zip
eazyqq_cli log-tail --lines 100

# 调试后门：注入模拟消息，走完整处理链路（无需第二个账号）
eazyqq_cli simulate --target 1104661022 --text "测试内容" --sender-name "测试者"
eazyqq_cli simulate --target 1104661022 --text "连发" --repeat 3   # 同进程连发，可验证回复冷却
```

所有命令都支持 `--json`，便于脚本消费。

## 数据隔离（按机器 + QQ 号）

**这是隐私要求，不是可选项。** 应用存储的一切都属于某个具体的 QQ 账号：消息记录、白名单规则、
AI 草稿、群聊简报、下载的群文件，以及**含聊天原文的日志文件**。原先这些全部直接放在
`EazyQQ_Data/` 下，换个 QQ 登录就会看到上一个账号的数据。

现在的布局：

```
EazyQQ_Data/
  bootstrap.json                    仅应用级：上次使用的账号、WebView 兼容开关
  accounts/
    <机器码>/                        由 Windows MachineGuid 派生（FNV-1a 短哈希）
      <QQ号>/
        eazyqq.db                   规则 / 消息 / 草稿 / 简报
        logs/                       ★ 含聊天原文，绝不可共用
        group_files/
        diagnostics/
      _unbound/                     尚未识别账号时使用
```

**为什么要带机器码**：工作区目录可能被复制或同步到另一台机器。只按 QQ 号分区的话，
把整个文件夹拷过去就会出现「另一台机器的私密数据被当成自己的」；加上机器码即可避免。

**自动切换**：应用启动时先解析账号**再**做任何磁盘操作；若协议端报告的账号与当前不同，
会记录新账号并**重启一次**，让所有路径重新计算——绝不会把 A 账号的消息、草稿和日志写进 B 账号的目录。
重启只发生一次（重启后记录与观察值一致）。

**多个 QQ 账号**：每个账号一个目录，互不覆盖；在 NapCat 里换账号后，应用会检测到并**重启一次**切过去。
两件容易出问题的事已处理：① 账号识别出来**之前**写的数据（首次运行、尚未登录时建的规则）会从 `_unbound`
归入该账号，不会被孤立；② **安装级设置**（AI 供应商 / Key、托盘行为、简报默认值）在新账号首次使用时
自动继承，不必把 API Key 再输一遍——而账号数据（规则、消息、草稿、简报）保持各自独立。

**NapCat 侧的共享产物也会被清扫**：应用自身的数据已隔离，但 NapCat 是独立程序，其 `logsPath`/`cachePath`
是内部字段、不可配置，因此它的 `napcat/logs/` 会堆积所有账号的会话日志（**含聊天文本**），
`napcat/cache/` 还会留下登录二维码。应用在解析出账号后会把这些一并归入该账号目录
（`<账号>/napcat_logs/`），并删除二维码。**尽力而为**：被运行中的 NapCat 占用的文件会跳过，
那属于当前会话，下次启动再收。

**旧数据自动迁移**：升级到本版本后，原先散落在 `EazyQQ_Data/` 根目录的数据会被搬进对应账号目录，
不会「看起来像全丢了」。迁移是**可重入**的：即使上一次只搬了一半，下次启动也会继续补完；
同名文件（例如新旧 `eazyqq.log`）会重命名为 `xxx.legacy-<时间戳>` 保留，**绝不把含聊天原文的日志留在公共位置**。

---

### 全链路监控

链路上有 8 个环节，任何一个坏掉都会表现为「功能不工作」，但原因完全不同：

```
NapCat WebUI → QQ 登录 → OneBot HTTP → OneBot WebSocket
             → 本地数据库 → 大模型 → 定时调度 → 前端界面
```

`eazyqq_cli chain-status` 会逐环节报告状态与**影响范围**，并指出**第一个断点**——
流水线里第一个断点之后的异常都只是后果，修别处是白费力气：

```
[FAIL] NapCat WebUI       WebUI 认证网络错误: ...
       └ 影响: 无法查询登录状态 / 获取二维码
[?]    QQ 登录              WebUI 不可达，无法判断登录状态
[FAIL] OneBot WebSocket   127.0.0.1:3001 未监听，消息无法进入
       └ 影响: 收不到任何新消息（草稿与简报都会停）
[OK]   本地数据库 / 大模型

第一个断点: NapCat WebUI
提示: 链路上后续环节的异常通常是这个断点的后果，先修这里。
```

客户端运行期间，监控会以 30 秒为周期持续探测，并在**状态发生变化时**（而非每次）写入日志，
避免刷屏。状态用三值：`OK` / `?`（尚未验证）/ `FAIL`——**「尚未验证」与「已知故障」严格区分**，
不会把没检查过的东西报成正常。

### 自检与审计

三个命令覆盖"这个安装是否健康"：

| 命令 | 回答的问题 |
| :--- | :--- |
| `selftest` | 安全边界还成立吗？（默认拒绝、触发条件、冷却、路径包容性、脱敏、ZIP 结构） |
| `config-audit` | 有没有「界面上能改、后端不读」的设置？ |
| `chain-status` | 链路上哪个环节断了？ |
| `napcat-doctor` | 协议端起不来时，卡在哪一步？ |

`selftest` 与单元测试调用**同一批产品函数**（不复制逻辑）。它存在的理由是：一套跑不起来的测试
保护不了任何人——本机的测试二进制因加载器问题无法启动（ISSUE-020），所以关键不变量必须另有入口。

> 写这些检查时踩到两次**空转断言**：`snapshot()` 遍历 `Link::all()`，所以「报告含 8 个环节」永远通过；
> 文件名检查用「是否含 `..` 子串」判断，而 `_.._evil.exe` 其实是安全的。
> 现在的写法检验**性质**（是否越界、是否真有探测结论），而不是**表象**。

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
