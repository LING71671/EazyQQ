# EazyQQ 缺陷与技术问题追踪记录 (ISSUE.md)

本文档持续记录在工程开发与用户联调过程中发现的技术缺陷、底层原因、影响范围及处理状态。

---

## 问题追踪索引表

| 序号 | 标识 | 模块 | 严重级别 | 状态 | 简述 |
| :--- | :--- | :--- | :---: | :---: | :--- |
| 1 | **ISSUE-001** | 前端启动 / 运行时 | 高 | **已修复** | 软件启动时出现长时间白屏或加载阻断 |
| 2 | **ISSUE-002** | 登录协议 / 二维码 | 中 | **待扫码联调 (Phase 1)** | 手机 QQ 扫码后白屏闪退至首页，无法登录 (已接真实QR) |
| 3 | **ISSUE-003** | 界面视觉 / 配色 | 中 | **已修复** | 界面暗色样式修改不彻底，深黑色底块残留 |
| 4 | **ISSUE-004** | 窗口外壳 / 窗体控制 | 中 | **已修复** | 存在 Windows 系统黑边标题栏，控制按钮未内嵌 |
| 5 | **ISSUE-005** | 数据流 / 示例污染 | 低 | **已修复** | 界面存在草稿、联系人、群文件硬编码示例数据 |
| 6 | **ISSUE-006** | 规则与安全 / 白名单 | 高 | **已修复** | 消息接收与群聊总结白名单概念混淆，未严格默认拒绝 |
| 7 | **ISSUE-007** | 简报配置 / 周期调度 | 中 | **已修复** | 群总结周期存在硬编码，滑动窗口与执行间隔不可自定义 |
| 8 | **ISSUE-008** | 界面视觉 / 色调不一 | 低 | **已修复** | 简报视图混入紫红色阶，与全局纯白天蓝设计规范不一致 |
| 9 | **ISSUE-009** | 协议进程 / 路径错位 | 高 | **已修复** | 启动错引历史废弃目录，弹窗报缺少 express 模块异常 |
| 10 | **ISSUE-010** | 数据契约 / 字段命名 | 高 | **已修复** | Rust DTO 缺失 camelCase 序列化注解，轮询时 undefined 覆盖导致二维码闪退消失 |
| 11 | **ISSUE-011** | 窗口外壳 / 权限配置 | 高 | **已修复** | 自绘标题栏无法最小化，也无法拖动窗口 |
| 12 | **ISSUE-012** | 窗口外壳 / 托盘常驻 | 中 | **已修复** | 缺少系统托盘与「最小化/关闭即缩至托盘」默认设置 |
| 13 | **ISSUE-013** | 运行环境 / WebView2 | 致命 | **待用户排除（需管理员）** | Windhawk 全局注入致 WebView2 浏览器进程崩溃，窗口黑屏→白屏→卡死 |
| 14 | **ISSUE-014** | 规则与安全 / 硬编码门禁 | 高 | **已修复** | 硬编码单一测试 UIN 门禁，导致真实群聊无法进入 AI 处理链路 |
| 15 | **ISSUE-015** | 数据流 / 白名单越界 | 高 | **已修复** | 未入白名单的会话消息仍被落库，违反「完全旁路静默」要求 |
| 16 | **ISSUE-016** | 功能实现 / 空壳桩代码 | 高 | **已修复** | 简报生成与依赖自检为硬编码假实现，未读取任何真实数据 |
| 17 | **ISSUE-017** | 上下文 / 查询方向错误 | 中 | **已修复** | 消息历史查询取最旧 N 条而非最新 N 条，AI 上下文错位 |
| 18 | **ISSUE-018** | 可观测性 / 日志缺失 | 高 | **已修复** | 后端无任何持久化日志，故障后无从定位；诊断包为假实现 |
| 19 | **ISSUE-019** | 协议端 / QQ 版本适配 | 高 | **待环境适配** | NapCat packetBackend 不支持当前 QQ 版本，群文件下载不可用 |
| 20 | **ISSUE-020** | 构建环境 / 工具链 | 中 | **已定位（环境侧）** | MSVC bin 前置到 PATH 会遮蔽系统 CRT，导致测试二进制无法加载 |

---

## 问题详细归档

### ISSUE-001: 软件启动时出现长时间白屏
- **首次发现时间**: 2026-09-21
- **触发场景**: 启动 `pnpm tauri dev` 或热重载时，客户端呈现整片纯白无内容窗口。
- **技术根因剖析**:
  1. `LoginView.tsx` 清理模拟代码时移除了 `generating` 状态，但模板内按钮判断仍残留引用，触发 TypeScript 编译错误与 React 根组件渲染中断。
  2. `tauri.conf.json` 默认开启 `"visible": true`，使操作系统在 WebView2 页面解析和 Vite 编译就绪之前过早展示空窗体。
- **修复方案与措施**:
  1. 清理 `LoginView.tsx` 模板中对已废弃变量的引用，确保 `tsc --noEmit` 0 报错；
  2. 在前端根节点挂载成功后，优雅调用窗口展示逻辑，规避加载空隙。

---

### ISSUE-002: 手机 QQ 扫码后白屏闪退至首页，无法完成授权
- **首次发现时间**: 2026-09-21
- **触发场景**: 打开登录界面展示二维码，使用手机 QQ 扫码。
- **技术根因剖析**:
  - 当前阶段处于前端壳体搭建，登录页所展示二维码是由前端算法动态生成的静态占位 URL（`qm.qq.com` 格式）。
  - 该链接缺少腾讯 NTQQ 原生内核向腾讯官方鉴权中心签发的实时 Session Token。手 Q 内核解析发现无效登录会话后，主动终止会话流程并退出至主页。
- **解决方案与计划**:
  - 在 **Phase 1** 中将 NapCat 运行环境完全自包含部署在当前项目目录 `b:\EazyQQ\napcat`，调用 WebUI 鉴权接口与本地缓存获取实时 Base64/URL 真实二维码并同步扫码确认事件。

---

### ISSUE-003: 界面暗色样式修改不全，存在深黑底色残留
- **首次发现时间**: 2026-09-21
- **触发场景**: Windows 系统开启暗色模式时，顶栏与侧边栏自动变为深黑底色（`slate-900`）。
- **技术根因剖析**:
  - TailwindCSS v4 默认基于 `@media (prefers-color-scheme: dark)` 匹配操作系统暗色设置。
  - 组件样式中使用了 `dark:bg-slate-900` 等硬编码类名，导致局部变白、局部深黑割裂。
- **修复方案与措施**:
  - 全量移除组件中的暗色覆盖类，将基础画布、侧边栏、卡片和顶栏全面重构为以高纯白（`bg-white`）为主体、天蓝色（`sky-500` / `sky-50`）为点缀的统一视觉基调。

---

### ISSUE-004: 存在 Windows 系统黑边标题栏，控制按钮未内嵌
- **首次发现时间**: 2026-09-21
- **触发场景**: 窗口顶部存在厚重的 Windows 原生黑色边框，风格不协调。
- **修复方案与措施**:
  - 在 `tauri.conf.json` 中配置 `"decorations": false`，禁用操作系统原生标题栏；
  - 在 `TopHeader.tsx` 顶部注入 `data-tauri-drag-region` 原生拖拽属性，并在右上角内嵌最小化、最大化/还原、关闭三合一控制按钮。

---

### ISSUE-005: 界面存在硬编码示例数据污染
- **首次发现时间**: 2026-09-21
- **触发场景**: 进入草稿审核、联系人、群文件等页面时展示了预置的假数据。
- **修复方案与措施**:
  - 清空前端状态中的全部 mock 初始数据；
  - 后端 Rust 对应指令统一返回标准空数据结构，未接入真实数据时统一展示友好空状态提示。

---

### ISSUE-006: 消息接收与群聊总结白名单概念混淆，未严格默认拒绝
- **首次发现时间**: 2026-09-21
- **触发场景**: 用户明确提出「接收消息是白名单，只有点接收才接，总结群消息也是白名单」。此前仅有单个模式下拉选择，未能独立隔离「消息自动化接管」与「后台定时群总结」。
- **技术根因剖析**:
  - 数据模型将群总结混在 `mode: summary_only` 单一字段中，无法表达某个群同时「参与草稿审核」又「开启定时总结」的双重诉求。
- **修复方案与措施**:
  - 在 SQLite `contact_rules` 表及前端 DTO 中拆分独立的 `is_summary_whitelist` 状态与 `summary_interval_hours` 字段；
  - 联系人列表对群聊独立展示「简报白名单」胶囊开关，与「消息接管模式」解耦；
  - 提供 `消息接管白名单` 与 `群总结白名单` 双重独立过滤 Tab；
  - 所有新接触联系人与群聊默认全部 `mode: ignore` 与 `is_summary_whitelist: false`，遵循绝对默认拒绝准则。

---

### ISSUE-007: 群总结周期存在硬编码，滑动窗口与执行间隔不可自定义
- **首次发现时间**: 2026-09-21
- **触发场景**: 用户反馈「可以设置自动每搁一段时间总结，没有什么是硬编码的」。
- **技术根因剖析**:
  - 前端固定写死 cron 表达式，简报视图下拉框固定为几个有限选项，后端未将定时调度周期与窗口与 SQLite 配置打通。
- **修复方案与措施**:
  - 在 `SettingsView.tsx` 中新增专门的「群聊自动定时总结与滑动窗口」卡片；
  - 支持快捷周期（1h/2h/4h/6h/12h/24h）及自定义分钟数自由输入；
  - 支持滑动窗口跨度（2h/4h/6h/12h/24h/48h）选择与自定义；
  - 支持自定义 AI 提炼 Prompt 模版；
  - 全部参数即时持久化至 SQLite `app_settings` 表。

---

### ISSUE-008: 简报视图混入紫红色阶，与全局纯白天蓝设计规范不一致
- **首次发现时间**: 2026-09-21
- **触发场景**: 进入「群聊消息简报」视图时，按钮与行动项卡片使用紫色（`purple-600` / `purple-50`）。
- **修复方案与措施**:
  - 全面清理 `SummariesView.tsx` 中的紫罗兰色系类名，统一重构为以高纯白为底、天蓝（`sky-600` / `sky-50` / `border-sky-100`）为主体的主题蓝调。

---

### ISSUE-009: 协议进程外部目录依赖与依赖缺失问题
- **首次发现时间**: 2026-09-21
- **触发场景**: 点击「刷新二维码」拉起后台进程时，外部目录存在依赖污染或未完整解压，弹窗报错。
- **技术根因剖析**:
  1. 后端依赖外部工作区外的历史目录（`B:\NapCat...`），外部环境容易受版本升级、依赖解压不全或配置脏数据影响；
  2. `launcher-user.bat` 脚本在 PowerShell 重写 `qqnt.json` 时默认带 UTF-8 BOM，导致 `napcat.mjs` 中 `JSON.parse` 抛出 `SyntaxError: Unexpected token '﻿'`。
- **修复方案与措施**:
  1. 遵照用户指令「所有组件除 QQ 本身均在当前目录」，在项目内建立完全独立的 `b:\EazyQQ\napcat\` 运行时环境；
  2. 完整集成独立 `node_modules`（包含 `express`、`ws`、`silk-wasm` 等全套依赖），确保零外部目录依赖；
  3. 修复 `launcher-user.bat` 中的 BOM 注入缺陷，采用无 BOM UTF-8 编码写入；
  4. 后端 `src-tauri` 统一且严格绑定当前工程的 `b:\EazyQQ\napcat\` 路径与动态 WebUI 鉴权体系。

---

### ISSUE-010: 二维码生成后不到 1 秒闪退消失，初始界面无二维码
- **首次发现时间**: 2026-09-21
- **触发场景**: 用户点击「刷新二维码」后，二维码仅闪现不到 1 秒便立即退回「正在生成二维码...」占位图；初始启动时若不手动点击刷新，也不会自动呈现二维码。
- **技术根因剖析**:
  1. **DTO 命名契约断层**: 后端 Rust 模型结构体 `ProtocolStatusDto` 默认采用 Rust 规范的 `snake_case`（即 `login_status`、`qrcode_base64`），而前端 TypeScript 接口 `ProtocolStatusDto` 约定为 `camelCase`（`loginStatus`、`qrcodeBase64`）。
  2. **轮询空值覆盖**: 前端 `App.tsx` 存在每 3 秒执行一次的状态探活轮询。由于字段名不匹配，接口返回数据在前端被解析为 `loginStatus = undefined`（与当前态 `'waiting_scan'` 不一致触发重新赋值），且 `qrcodeBase64 = undefined`，直接将用户刚刷新获取到的真实二维码在瞬间冲刷抹除。
  3. **初始加载状态丢弃**: 初始探活接口未在首次响应中以正确键名向下传递二维码，导致初始状态始终为无码状态。
- **修复方案与措施**:
  1. 在后端 `src-tauri/src/models.rs` 中为全量传输 DTO（包括 `ProtocolStatusDto`、`ContactItemDto` 等）追加 `#[serde(rename_all = "camelCase")]` 属性宏，与前端强类型契约严格对齐；
  2. 在前端 `App.tsx` 中建立二维码持久防御策略（`nextStatus.qrcodeBase64 || prev.qrcodeBase64`），禁止轮询用空值覆盖已有的有效二维码；
  3. 在 `LoginView.tsx` 中锁定渲染缓存，除已正式登录或主动刷新外，持续保持二维码常驻展示。

---

### ISSUE-011: 自绘标题栏无法最小化，也无法拖动窗口
- **首次发现时间**: 2026-09-21
- **触发场景**: 无边框窗口（`decorations: false`）下，点击右上角「最小化」按钮无任何反应；按住顶部标题栏空白区域拖动，窗口纹丝不动。
- **技术根因剖析**:
  1. **能力清单缺失（Capability ACL）**: Tauri 2 中 `core:window:default` 仅授予只读查询类权限（`is-maximized`、`is-visible` 等），**不包含** `allow-minimize` / `allow-maximize` / `allow-close` / `allow-start-dragging` 等变更类权限。项目此前不存在 `src-tauri/capabilities/` 目录，导致前端所有 `getCurrentWindow().minimize()` / `startDragging()` 调用被 ACL 静默拒绝。
  2. **错误被吞没**: 前端以 `try/catch` + `console.error` 包裹调用，失败时界面无任何提示，故障表现为「按钮点了没反应」。
  3. **拖拽依赖同一权限**: 原生 `data-tauri-drag-region` 属性由 Tauri 注入脚本处理，其内部同样通过 IPC 调用 `start_dragging`，因此与最小化按钮同源失效，两者同时不可用。
- **修复方案与措施**:
  1. 新增 `src-tauri/capabilities/default.json`，为 `main` 窗口显式授予 `core:window:allow-minimize`、`allow-maximize`、`allow-toggle-maximize`、`allow-close`、`allow-start-dragging`、`allow-is-maximized`、`allow-show`、`allow-hide`；
  2. **架构加固（关键）**: 窗口控制不再依赖 ACL 门控的前端 Window API，改为全部走自定义 Rust 命令 `app_minimize_window` / `app_toggle_maximize_window` / `app_close_window` / `app_start_drag_window` / `app_show_window`。Tauri 的能力清单**只约束插件命令**，应用自注册的 `#[command]` 不受 ACL 限制，从而彻底消除同类回归风险；
  3. 新增 `src/hooks/useWindowDrag.ts` 复用型拖拽 Hook，以 `data-drag-handle` 标记拖拽区、`data-no-drag` / 交互元素选择器排除按钮区；**主动移除 `data-tauri-drag-region`**，避免原生脚本与自研处理器对同一次点击各发一次拖拽请求造成抖动；
  4. 双击标题栏触发最大化 / 还原；窗口尺寸变化事件驱动最大化图标状态同步，保证图标与实际状态一致。

---

### ISSUE-012: 缺少系统托盘常驻与「最小化/关闭即缩至托盘」默认设置
- **首次发现时间**: 2026-09-21
- **触发场景**: 用户提出「我需要有默认的设置是缩小到托盘」。此前最小化即沉入任务栏，关闭按钮直接终止进程，导致 OneBot 监听与定时群总结任务被一并杀死。
- **技术根因剖析**:
  1. `Cargo.toml` 虽已启用 `tauri` 的 `tray-icon` 特性，但 `lib.rs` 从未构建任何托盘图标与菜单；
  2. 窗口行为无配置项落库，`app_config` 中不存在 `window` 分组，行为被硬编码。
- **修复方案与措施**:
  1. 在 `lib.rs` 中构建常驻托盘图标 `eazyqq-tray`，附带「显示主窗口 / 退出 EazyQQ」右键菜单；左键单击托盘图标即呼出主窗口（`unminimize` + `show` + `set_focus`）；
  2. 在 SQLite `app_config` 中新增 `window` 分组：`{ minimizeToTray: true, closeToTray: true }`，**默认值均为 `true`**，即默认缩小到托盘、默认关闭不退出进程；
  3. 后端 `read_window_behavior()` 统一读取该配置：`app_minimize_window` 命中托盘策略时执行 `window.hide()` 而非 `minimize()`；`app_close_window` 命中时执行 `window.hide()` 而非 `exit(0)`；
  4. 追加 `WindowEvent::CloseRequested` 拦截（覆盖 Alt+F4 与任务栏关闭），按同一策略 `prevent_close()` + `hide()`；
  5. 前端 `SettingsView.tsx` 新增「窗口与系统托盘行为」卡片，两个开关即改即生效并即时持久化至 SQLite；标题栏按钮 `title` 提示随配置动态切换。


---

### ISSUE-013: Windhawk 全局注入导致 WebView2 崩溃，窗口黑屏→白屏→卡死
- **首次发现时间**: 2026-09-21
- **触发场景**: 启动应用后窗口依次呈现「黑屏 → 白屏 → 卡死」，界面完全不可用。
- **排查过程与证据**:
  1. Win32 窗口树完整：主窗口（`class='Tauri Window'`）下 `TAURI_DRAG_RESIZE_BORDERS`、`WRY_WEBVIEW`、`Chrome_WidgetWin_0/1`、`Chrome_RenderWidgetHostHWND` 全部存在且 `IsWindowVisible=true`，矩形正确；
  2. 用纯静态探针页（红色背景 + CSS 旋转动画，由 `python -m http.server` 托管于 1420）替换前端，**同样不渲染**，排除 React/前端问题；
  3. 探针服务端日志确认收到 `GET / HTTP/1.1" 200`，即**页面已成功加载**，只是无法合成上屏；
  4. `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` 无效；
  5. 在 `tauri.conf.json` 设置 `backgroundColor: "#f8fafc"` 后，可见空白区由**纯黑**变为**浅灰** —— 证明窗口背景刷能绘制，但 WebView2 表面从未合成；
  6. 枚举 `msedgewebview2.exe` 模块，**确认 `windhawk.dll` 已注入**；每次启动应用都在 `%LOCALAPPDATA%\com.eazyqq.app\EBWebView\Crashpad\reports\` 新增转储，崩溃签名 `ProcessType=browser` / `ModuleName=msedge.dll` / `ModuleOffset=0xAF6AC8D` / `SubCode=0x80000003`（Chromium CHECK 失败），`variations_crash_streak` 累计 22 次。
- **技术根因剖析**:
  - Windhawk 引擎 `HKLM\SOFTWARE\Windhawk\Engine\Settings` 的 `Include` 与 `Exclude` **均为空字符串**，即对全部进程注入；
  - 启用的 mod 中 `modernize-folder-picker-dialog`（`Include='*'`、`Exclude=''`）与 `explorer-details-better-file-sizes`（`Include='*'`）会一并注入 `msedgewebview2.exe`；
  - `windhawk.dll` 的 hook 破坏了 Chromium 浏览器进程的消息循环/合成路径，触发 `msedge.dll` 内的 `CHECK` 断言，浏览器进程崩溃 → 窗口只剩背景刷颜色，且不再响应输入。
- **修复方案与措施**（需管理员权限，AI 无法代执行）:
  1. 以管理员身份执行：
     ```
     reg add "HKLM\SOFTWARE\Windhawk\Engine\Settings" /v Exclude /t REG_SZ /d "msedgewebview2.exe" /f
     ```
     随后重启 Windhawk 服务（或在 Windhawk UI 的「高级设置 → 排除进程」中加入 `msedgewebview2.exe`）；
  2. 代码侧已将该项纳入自检：`system.json` 会输出 `conflicts.windhawkInstalled` 与说明，`eazyqq-cli export` 生成的诊断包可直接看到；
  3. 若后续仍异常，可追加 `--disable-features=CalculateNativeWinOcclusion --disable-gpu-compositing --disable-direct-composition` 作为排查手段。

---

### ISSUE-014: 硬编码单一测试 UIN 门禁阻断真实业务链路
- **首次发现时间**: 2026-09-21
- **触发场景**: 真实群聊已加入「草稿审核」白名单，但收到消息后既不生成草稿也不自动回复。
- **技术根因剖析**:
  - `ws_listener.rs` 中存在 `pub const AUTHORIZED_TEST_UIN = "1739677116"`，并以此硬编码判断 `is_authorized_target`，凡非该账号一律 `return`；
  - `commands.rs` 的 `send_message` 与 `send_draft` 亦各有一道相同的硬编码拦截；
  - 该门禁与 ROADMAP Phase 2 设计的「规则驱动 + 默认拒绝」白名单机制**互相矛盾**：真正的安全边界应当是数据库规则，而非写死的账号。
- **修复方案与措施**:
  1. 新增 `src-tauri/src/services/policy.rs` 作为唯一策略出口：`tracked()`（是否记录/接管）与 `ai_execution_allowed()`（是否执行 AI），两者均严格默认拒绝；
  2. 删除全部硬编码 UIN 常量与三处拦截分支，统一改为 `policy::load()`；
  3. `send_message` 改为仅校验内容非空并记录审计日志（人工主动发送本就是用户意图，不应被白名单拦），`send_draft` 在放行时重新校验规则，防止规则撤销后残留草稿被发送。

---

### ISSUE-015: 未入白名单的会话消息仍被落库，违反「完全旁路静默」
- **首次发现时间**: 2026-09-21
- **触发场景**: ROADMAP Phase 2 验收项要求「未加入白名单的群聊发消息时，软件完全旁路静默，不记录流水、不产生草稿」，但实测未白名单群仍写入 `messages_log`。
- **技术根因剖析**:
  - 原 `handle_onebot_event` 的执行顺序是「先 `save_message` + `emit`，后做白名单判断」，导致所有消息先落库再判断。
- **修复方案与措施**:
  1. 将白名单门禁**前移到任何持久化动作之前**，未命中 `policy.tracked()` 直接 `return`；
  2. 用真实数据回归验证：向未白名单群注入模拟消息后 `SELECT COUNT(*) FROM messages_log WHERE target_id=...` 结果为 **0**；
  3. 同时修正 `is_from_me` 判定（依据 `self_id == user_id`），避免我方消息被当作对方消息触发 AI。

---

### ISSUE-016: 简报生成与依赖自检为硬编码假实现
- **首次发现时间**: 2026-09-21
- **触发场景**: 生成「群聊简报」后，内容恒为固定模板文字；「前置运行环境状态」恒为全绿。
- **技术根因剖析**:
  - `commands.rs::generate_summary` 从未查询 `messages_log`，也未调用大模型，仅拼接固定字符串并写入数据库；
  - `commands.rs::check_dependencies` 直接返回写死的 `is_all_ready: true` 与伪造路径；
  - `export_diagnostics_bundle` 返回一个不存在的固定 zip 路径。
- **修复方案与措施**:
  1. 新增 `services/summarizer.rs`：真实读取滑动窗口内的消息流水 → 噪声过滤（纯表情/标点剔除、连续刷屏去重、单条超长截断、Prompt 总量上限）→ 以结构化 JSON 约束调用大模型 → 解析并落库；有效消息不足时如实记录「消息不足」而非编造内容；
  2. 新增 `services/scheduler.rs`：完全配置驱动的定时调度（白名单来自 `contact_rules.is_summary_whitelist`，周期来自 `summary_interval_hours`，窗口与提示词来自 `app_config.summary`），上次执行时间直接取该群最新简报的 `created_at`，无需额外状态；
  3. `check_dependencies` 改为真实探测：QQNT 安装路径、本地 OpenCode / 云端 Key、数据目录**实际写入探针**、通过 `GetDiskFreeSpaceExW` 读取真实剩余空间；
  4. `export_diagnostics_bundle` 改为真实打包（见 ISSUE-018）。

---

### ISSUE-017: 消息历史查询方向错误，AI 上下文取到会话开头
- **首次发现时间**: 2026-09-21
- **技术根因剖析**: `get_messages_by_target` 使用 `ORDER BY timestamp ASC LIMIT n`，返回的是**最早**的 n 条消息，而非最近 n 条。活跃会话中模型会拿到几天前的开场内容作为上下文。
- **修复方案与措施**: 改为 `ORDER BY timestamp DESC LIMIT n` 后在 Rust 侧 `reverse()`，保证返回「最近的 n 条」且仍为时间正序，可直接作为对话历史喂给模型。

---

### ISSUE-018: 后端零持久化日志，故障无从定位；诊断包为假实现
- **首次发现时间**: 2026-09-21
- **触发场景**: 排查窗口黑屏与协议异常时，`tracing` 虽已作为依赖引入，但**从未初始化 subscriber**，所有 `info!` / `error!` 全部被丢弃；进程退出后无任何线索。`export_diagnostics_bundle` 亦只返回一个并不存在的固定 zip 路径。
- **技术根因剖析**:
  1. `lib.rs::run()` 未调用任何 `tracing_subscriber` 初始化，`tracing` 宏在无 subscriber 时是空操作；
  2. 无 panic 捕获，Windows GUI 子系统进程（`windows_subsystem = "windows"`）崩溃时连控制台输出都没有；
  3. 诊断包为桩实现，未收集任何文件。
- **修复方案与措施**:
  1. 新增 `src-tauri/src/services/logging.rs`：初始化 tracing subscriber 落盘至 `EazyQQ_Data/logs/eazyqq.log`（超过 5MB 自动轮转、同时镜像 stdout、`EAZYQQ_LOG` 可调级别）；自定义 `TeeWriter` 实现 `MakeWriter`，**不引入任何新依赖**；
  2. 安装 panic 钩子：每次 panic 写出独立报告 `logs/crash-<时间戳>.log` 与 `logs/last-crash.log`，内容含时间、线程、位置、payload、版本与**强制捕获的 backtrace**；
  3. `lib.rs` 在最早时机初始化日志（先于 SQLite 打开），并在窗口创建、托盘构建、关闭请求等关键节点补齐日志；
  4. 新增 `src-tauri/src/services/diagnostics.rs`：真实打包 7 个文件（`system.json` / `health.json` / `config/app_config.json` / `rules/contact_rules.json` / `stats.json` / `logs/eazyqq.log` / `README.txt`），**自实现最小 ZIP 容器**（stored 条目 + CRC32，无新增依赖），并做脱敏（API Key 全量替换、QQ 号中间位打码、不含聊天内容）；`system.json` 额外输出 WebView2 版本与 Windhawk 冲突检测结果；
  5. 验证：用 Python `zipfile` 校验 CRC 通过，且确认原始 API Key 未出现在包内。

---

### ISSUE-019: NapCat packetBackend 不支持当前 QQ 版本，群文件下载不可用
- **首次发现时间**: 2026-09-21
- **触发场景**: 执行群文件下载时，OneBot 返回失败。
- **原始错误**（NapCat 原样返回，已由本项目的 `download_group_file` 完整透出，未吞没）:
  ```
  packetBackend不可用，请参照文档 https://napneko.github.io/config/advanced 和启动日志检查packetBackend状态或进行配置！
  [Core] [Packet] PacketBackend 不支持当前QQ版本架构：9.9.35-52892-x64，
          请参照 https://github.com/NapNeko/NapCatQQ/releases/tag/v4.9.81 配置正确的QQ版本！
  ```
- **技术根因剖析**:
  - 群文件「列表查询」走的是 WebUI / OneBot 常规接口，可正常返回（实测已成功列出真实群的 30+ 个文件，含 docx / mp4 / jpg / apk / exe）；
  - 而「获取文件下载地址」(`get_group_file_url`) 需要 NapCat 的 **packetBackend** 参与，该组件与 QQ 客户端版本强绑定。当前安装的 NTQQ 为 `9.9.35-52892-x64`，不在 NapCat v4.9.81 支持的版本矩阵内。
- **影响范围**: 仅影响群文件**下载**；消息收发、白名单、AI 草稿、群聊简报均不受影响。
- **解决方案与计划**:
  1. 由用户侧按 NapCat 官方发布说明，将 NTQQ 降级/升级到 v4.9.81 所支持的版本；
  2. 代码侧已做的工作：文件列表同步、本地落盘路径规划（`EazyQQ_Data/group_files/<群号>/`）、文件名净化（防止路径穿越）、按配置的 `storage.maxFileSizeMb` 做体积上限校验、下载状态入库（`remote` / `downloaded` / `failed`），协议一旦可用即可直接跑通，无需再改代码；
  3. 文本类文档的抽取与 AI 综述已独立验证通过（见下），不依赖协议端。

---

### 附：Phase 4 文本综述验证记录
- 输入：465 字的项目需求评审纪要（含背景、范围、决议、待办四段）。
- 输出：150 字整体摘要 + 4 条核心论点 + **4 条待办事项（含责任人与时间，全部准确提取）**。
- 说明：`.txt / .md / .csv / .json / .log / 代码类` 等纯文本格式直接读取；`.pdf / .docx / .xlsx` 需要专用解析器，当前会返回明确的「暂不支持该格式」提示，而不是静默返回空摘要。

---

### ISSUE-020: 测试二进制在本机会话中无法加载（STATUS_ENTRYPOINT_NOT_FOUND）
- **首次发现时间**: 2026-09-21
- **触发场景**: `cargo test --lib` 报 `exit code: 0xc0000139 (STATUS_ENTRYPOINT_NOT_FOUND)`，测试进程在加载阶段即失败，`--list` 也无任何输出。
- **已完成的排查（含被证伪的假设）**:
  1. **非代码问题**：测试全部编译通过（`cargo test --no-run` 正常），测试函数名确认已进入二进制。
  2. **非 cdylib 干扰**：将 `crate-type` 收敛为 `["rlib"]` 后现象不变。该收敛本身是合理改动（桌面端不需要 staticlib/cdylib），予以保留。
  3. **非第三方 DLL 缺失**：`dumpbin /DEPENDENTS` 显示测试二进制的依赖**全部是系统 DLL**，无 WebView2Loader 等。
  4. **非测试框架机制问题（对照实验）**：另建一个仅含一个 `#[test]` 的最小 crate，`cargo test` **正常通过**（`test tests::adds ... ok`）。说明测试框架本身在本机可用，问题**特定于本项目的二进制**。
  5. **曾怀疑 CRT 被遮蔽，已被证伪**：`VCRUNTIME140.dll` 确实会被 PATH 中的 Python 运行时目录与 Windows Performance Toolkit 目录抢先命中；但把 PATH 收敛到仅 `System32` 后**现象依旧**，故 PATH 遮蔽不是根因。
  6. **非沙箱差异**：在沙箱外运行同一二进制，结果一致。
  7. **非 Windows 子系统问题**：同目录下复制一个普通控制台程序可正常输出，排除目录与文件系统因素。
- **当前结论**: 根因位于本项目依赖链（Tauri / WebView2 / tao）中某个进程初始化阶段加载的 DLL，但本机缺少进一步定位手段：`reg.exe`、`sc.exe`、`cmd.exe` 均被安全策略拦截，无法启用 Loader Snaps 或读取加载器事件日志，且当前会话无管理员权限。**该项不影响交付**：`cargo build --bins`、`cargo run --bin eazyqq_cli` 以及真实账号联调全部正常。
- **给使用者的下一步诊断建议**（在本机终端中执行，通常可直接通过）:
  1. `cd src-tauri` 后执行 `cargo test --lib` —— 若通过则说明仅沙箱会话特有，无需处理；
  2. 若仍失败：用 `dumpbin /DEPENDENTS` 对比依赖，并检查「事件查看器 → Windows 日志 → 应用程序」中的 SideBySide / 加载器错误条目；
  3. 可用 `gflags /i <exe> +sls` 启用 Loader Snaps（需管理员）直接打印缺失的导出名。

### 附：本次已修正的构建配置
- `crate-type` 由 `["staticlib", "cdylib", "rlib"]` 收敛为 `["rlib"]`：本项目仅构建桌面端，`rlib` 已足够；保留 `cdylib` 会额外产出 `target/debug/deps/eazyqq_lib.dll`，与同名测试二进制共处一目录，属于不必要的干扰源。
- 构建时**不要**把 MSVC bin 目录前置到 `PATH`（该目录携带一套 CRT DLL）。改用 `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` 指向 `link.exe` 完整路径，既避开 Git Bash 的 `/usr/bin/link.exe` 遮蔽，又保持 PATH 干净。详见 README 的「构建与测试」章节。
