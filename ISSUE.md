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
| 19 | **ISSUE-019** | 协议端 / QQ 版本适配 | 高 | **根因已定位** | QQ 版本超出 NapCat 偏移表上限；需 QQ ≤ 9.9.26-44343（NapCat 4.18.28） |
| 20 | **ISSUE-020** | 构建环境 / 工具链 | 中 | **已定位（环境侧）** | MSVC bin 前置到 PATH 会遮蔽系统 CRT，导致测试二进制无法加载 |
| 21 | **ISSUE-021** | 设计假设 / 本地推理 | 中 | **已修正** | 原设计假设 OpenCode 提供 OpenAI 兼容接口，实测其 serve 端口返回 Web UI |
| 22 | **ISSUE-022** | 网络 / 代理绕过 | 高 | **已修复** | 系统 http_proxy 会拦截 loopback 请求，导致本地模型与协议端调用异常 |
| 23 | **ISSUE-023** | 凭据管理 / 快速登录 | 中 | **已修复** | 重启后未自动触发 QuickLogin，导致每次均需重复扫码 |
| 24 | **ISSUE-024** | 视觉反馈 / 异常焦虑 | 低 | **已修复** | 协议端加载等待态误用红色「刷新失败」引起用户焦虑 |
| 25 | **ISSUE-025** | 配置管理 / 多厂商隔离 | 高 | **已修复** | 切换模型 Provider 会冲刷已有自定义 Model/URL/Key，未按厂商独立隔离 |
| 26 | **ISSUE-026** | 消息渲染 / 富文本解析 | 中 | **已修复** | 漫游历史卡片展示为原始 `[CQ:json]` 字符串，气泡缺少闭合引发前端热更阻断 |
| 27 | **ISSUE-027** | 数据流 / 消息双重加载 | 高 | **已修复** | 实时推送与漫游历史的 ID 生成策略不一致，导致同一条聊天记录在气泡列表中双份渲染 |

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

---

### ISSUE-021: 设计假设有误 —— OpenCode 并不提供 OpenAI 兼容接口
- **首次发现时间**: 2026-09-22
- **触发场景**: 按 PRODUCT.md / ROADMAP Phase 3 的原始设计，把「本地 OpenCode」作为一个 OpenAI 兼容供应源接入，预设端点 `http://127.0.0.1:4096/v1`。
- **排查过程（实测而非推测）**:
  1. 启动真实服务：`opencode serve --port 4096 --hostname 127.0.0.1`，端口正常监听；
  2. 探测 `POST /v1/chat/completions` 与 `GET /v1/models`：**均返回 HTTP 200，但响应体是 OpenCode 的 HTML 单页应用**（`<title>OpenCode</title>` 及其前端资源），即 SPA 的 catch-all 回退，而不是 JSON；
  3. 进一步探测其真实路由：`/api/health` 返回 `{"healthy":true}`，`/api/agent` 返回智能体定义，说明服务端确实存在，但**其对外协议是 OpenCode 自己的会话式 API（`/api/*`），不是 OpenAI 的 `/chat/completions`**；
  4. 结论：OpenCode 是**编码智能体框架**，不是推理服务器。它自身作为客户端去调用各家模型，并不对外提供 OpenAI 兼容的推理端点。
- **额外发现**: `opencode serve` 的 `--port` 默认值是 **0（随机端口）**，因此「固定 4096」这一预设本身就不可靠；且实测其服务进程在运行数分钟后内存涨至 633MB 并出现请求无响应。
- **修复方案与措施**:
  1. **移除 `opencode` 预设**，不再对外宣称其为可用供应源；
  2. 供应源列表改为真实 OpenAI 兼容的实现：`ollama` / `lmstudio` / `llamacpp` / `vllm`（本地）+ `deepseek` / `openai` / `tokenrhythm`（云端）；
  3. 未知 provider 视为**自定义供应商**：不套用任何预设，必须显式提供 `baseUrl` 与 `model`，否则 `validate()` 返回明确错误，而不是把请求发往空地址；
  4. 新增 `eazyqq_cli ai-detect`：逐一探测常见本地端口，并区分「可用 / 需鉴权 / 不兼容 / 不可用 / 未监听」，其中**专门识别"返回 HTML 的 Web UI"这一情形**并标记为不兼容，避免用户踩同一个坑；
  5. 前端设置页同步移除 OpenCode 选项，并新增 llama.cpp / vLLM。
- **说明**: 本项目的 PRODUCT.md 与 ROADMAP Phase 3 中「本地 OpenCode（OpenAI 兼容接口）」的表述属于原始设计假设，与实测不符，已在文档中记录并在实现上纠正。

---

### ISSUE-022: 系统代理会拦截 loopback 请求，破坏本地模型与协议端调用
- **首次发现时间**: 2026-09-22
- **触发场景**: 用 `ai-detect` 探测本地端口时，**已确认无服务监听的端口**返回的却是 `HTTP 502 Bad Gateway`，而非"连接被拒绝"；进一步排查发现 `http_proxy` / `https_proxy` 环境变量被设置为 `http://127.0.0.1:9244`。
- **技术根因剖析**:
  - `reqwest` 默认读取 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量并**对所有请求生效，包括 `127.0.0.1`**；
  - 于是对本地推理端点的请求被送到代理，代理再去连本地端口，失败时统一回 502。表现为"本地模型明明起来了却连不上"，或"报错信息完全指错方向"；
  - 同理，`OneBotClient` 与 `NapCatService` 面向的都是 loopback 服务，一旦用户开启代理（本项目所在机器的 `proxy-on` 会设置 `127.0.0.1:10808`），协议端调用也会被无谓地绕经代理，增加延迟并引入额外故障点。
- **影响范围**: 所有指向 `127.0.0.1` / `localhost` 的请求。云端请求不受影响，且云端**需要**走代理。
- **修复方案与措施**:
  1. `AiService` 持有两个客户端：`client_proxied`（云端，遵循环境代理）与 `client_direct`（`.no_proxy()`，本地端点），按 `is_local_endpoint(base_url)` 逐次选择；
  2. `OneBotClient` 的控制面客户端加 `.no_proxy()`（始终 loopback）；**文件下载客户端保持走代理**，因为群文件来自远端 CDN，可能确实需要代理；
  3. `NapCatService` 客户端加 `.no_proxy()`；
  4. `ai-detect` 同样按目标地址选择客户端，使探测结论不再被代理污染。
- **验证**: 修复后探测已关闭的本地端口正确返回"端口未监听或拒绝连接"，而云端端点正确返回"需鉴权"（无 Key 时 401），二者不再混淆。

---

### ISSUE-013 更正与最终定位：不是 Windhawk，而是 Chromium 沙箱被干扰

> 本节推翻此前「Windhawk 注入导致 WebView2 崩溃」的结论。原结论建立在一个**不成立的推理**上：
> 观察到 `windhawk.dll` 存在于崩溃的 `msedgewebview2.exe` 进程中，且崩溃时间相关，就认定其为因。
> 这是把「相关」当成了「因果」。以下为重新取证的过程与最终结论。

#### 一、原结论被证伪的证据

1. **对照实验**：机器上同时有 **19 个正在运行的 `msedgewebview2.exe`**（来自其它正常工作的 WebView2 应用），
   其中 **9 个同样加载了 `windhawk.dll`**，**22 个 `chrome.exe` 中有 8 个**同样加载；
   更进一步，Windhawk 的 **mod DLL 本体**（`explorer-details-better-file-sizes_1.5_147899.dll`）也被注入进了这些**正常工作的** WebView2 进程。
   既然注入相同而结果不同，注入就不是判别因素。
2. **崩溃栈分析**（自写 minidump 解析器，从异常流的 CONTEXT 取 RSP 后逐字扫描）：
   - EazyQQ 崩溃栈上 **95~99 个返回地址全部落在 `msedge.dll` 内**（另有 6 个 `msctf.dll`，即输入法框架）；
   - **指向 `windhawk.dll` 的返回地址为 0 个** —— 它根本不在崩溃调用链上。
3. **Windhawk 配置**：已安装的 12 个 mod（explorer / taskbar / start-menu 等）**没有任何一个以 `msedgewebview2.exe` 为目标**。

#### 二、真正的根因

从崩溃转储中提取到 Chromium 的遗言：

```
ERROR:content\browser\gpu\gpu_process_host.cc:1114] GPU process exited unexpectedly: exit_code=-1073741819
FATAL:content\browser\gpu\gpu_data_manager_impl_private.cc:436] GPU process isn't usable. Goodbye.
```

`-1073741819` 即 **`0xC0000005` STATUS_ACCESS_VIOLATION**：**Chromium 的 GPU 子进程访问违规死亡**，
浏览器主进程判定「GPU 不可用」后主动终止（这正是 `msedge.dll` 中那个 `0x80000003` / CHECK）。
渲染进程随之无法合成画面，**窗口只剩下配置的背景色 `#f8fafc`**，即用户看到的「白屏」。

#### 三、定位过程（含被我自己的测试方法误导的两次）

1. **误判一：把「空白窗口」当成渲染故障**。当时用 `(pnpm dev &)` 启动前端服务，该进程随命令结束被回收，
   导致 WebView 无内容可加载——**空白是必然结果**。必须用受管后台进程保持前端存活，测试才有效。
2. **误判二：依赖 `PrintWindow` 截图判断渲染**。该方法对 WebView2 的 DirectComposition 图层存在已知局限。
   改用**不依赖截图的独立判据**：前端一旦挂载必然调用 `get_contacts`，于是在该命令中加日志作为「存活探针」。
   同时在 Vite 侧加请求日志，从服务端反向确认 WebView 的抓取行为。
3. **决定性证据**（双通道）：
   - Vite 收到请求：`index.html` → `main.tsx` → `@vite/client` → `react.js` → `App.tsx` → `react-dom_client.js`，**然后戛然而止**；
   - 后端 `get_contacts` 调用次数为 **0**；
   - 即：**页面取到一半，渲染进程死亡，React 从未挂载**。

#### 四、修复与验证

逐参数隔离测试：

| 参数 | 崩溃转储 | `get_contacts` 调用 | 结论 |
| :--- | :--- | :--- | :--- |
| `--no-sandbox` | 不新增 | 4 → 6 | **生效** |
| `--disable-gpu` | 不新增 | 6 → 6 | 无效 |
| `--disable-gpu-compositing --use-angle=swiftshader --disable-features=Vulkan` | 仍新增 | 0 | 无效（参数确已生效，见转储内的 flag 字符串） |

最终修复：`tauri.conf.json` → `app.windows[].additionalBrowserArgs = "--no-sandbox"`。

验证结果（`PrintWindow` 抓取窗口自身渲染）：
- 修复前：**98.7% 的像素为单一背景色 `(248,250,252)`** —— 只有窗口底色；
- 修复后：`(255,255,255)` 62.8% + `(249,251,252)` 29.3% + `(240,249,255)` 2.4% + 边框色 —— **真实 UI 内容**，
  界面可见完整侧边栏（账号状态 / 联系人规则 / 草稿审核 / 群文件同步 / 智能简报 / 系统设置），
  且**草稿审核徽标显示「11」**，说明前端已成功读取后端真实数据。

#### 五、结论与权衡（需使用者知晓）

- **成因**：本机上存在某种因素干扰 Chromium 的沙箱机制，使 GPU 子进程以访问违规告终。
  由于 `--no-sandbox` 可规避、而各类渲染后端参数均无效，问题出在**沙箱/进程环境**而非显卡驱动或渲染后端。
- **首选排查方向**：**临时完全停用 Windhawk（需管理员）后复测**。虽然已证明其注入不是直接原因，
  但它确实会 hook 进程创建路径，是成本最低、最值得先排除的变量；若停用后无需 `--no-sandbox` 即可正常，
  则应保留排除方案而不是长期关闭沙箱。
- **安全权衡**：`--no-sandbox` 会关闭 Chromium 的渲染进程沙箱。本应用只加载本地打包内容、不浏览任意网页，
  风险相对可控，但**这仍是一项真实的安全降级**，应视为临时规避手段而非最终方案。
- **后续**：建议在排除干扰源后移除该参数；同时把「WebView 是否真的加载了前端」纳入 `health --deep` 自检项。

---

### ISSUE-019 深挖：偏移表版本上限（结论与可行路径）

#### 一、精确定位

NapCat 的 packet 机制依赖一张 **「QQ 版本 → {send, recv} 偏移」映射表**。当 QQ 版本不在表内时，
日志输出：

```
[warn] NativePacketClient: 未找到对应版本的偏移数据: 9.9.35-52892-x64
```

（`9.9.35-52892-x64` 是 NapCat 报告的版本号；`get_group_file_url` 随后返回
`packetBackend不可用`，因此**群文件下载失败**，而文件列表、消息收发、草稿、简报均不受影响。）

从本机 NapCat 4.9.81 的 `napcat.mjs` 中解析出该表的**完整覆盖范围：共 53 个版本，x64 最高为
`9.9.25-42941`**。当前 `A:\NTQQ` 实际为 **9.9.28-46928**，已超出上限，故取不到偏移。

#### 二、官方口径

- NapCat **v4.9.81** 发布说明：「注意 QQ 版本推荐使用 **40768+** 版本，最低可以使用 40768 版本」，
  且本版「新增了 **42941** 版本的 appid.json、napi2native.json 和 packet.json 中的映射」。
- 本机安装的是 **4.9.81**，而 GitHub 上最新为 **v4.18.28**（2026-09-14），其说明中给出的最新支持版本为
  **9.9.26-44343**，并附官方直链。

结论：**版本上限取决于 NapCat 版本**。要覆盖 9.9.28 这一档，需要更新的 NapCat；而 4.18.28 的上限
（9.9.26-44343）仍低于本机的 9.9.28-46928，因此**必须同时下调 QQ 版本**。

#### 三、下载尝试（均失败，如实记录）

| 来源 | 结果 |
| :--- | :--- |
| v4.18.28 发布说明中的官方直链 `dldir1.qq.com/qqfile/qq/QQNT/40d6045a/QQ9.9.26.44343_x64.exe` | **HTTP 404**，腾讯 CDN 已下架该版本 |
| 历史版本镜像站 `rodert.github.io/qq-versions` 的 GitHub Releases | 仅保留 **9.9.30 及之后**的安装包，无 9.9.25/9.9.26 |
| 猜测的 `dldir1.qq.com` 路径规律 | 全部 404（下载路径含版本专属 hash，无法推导） |

#### 四、本机已存在的受支持版本（实测）

| 路径 | 版本 | 是否在偏移表内 | 实测结果 |
| :--- | :--- | :---: | :--- |
| `A:\NTQQ\QQ.exe` | 9.9.28-46928 | ❌ 超上限 | 可正常运行、消息链路完全可用，但无偏移数据 |
| `B:\NapCat.Shell.Windows.OneKey\NapCat.41785.Shell\QQ.exe` | 9.9.23-41785 | ✅ | **独立启动正常**（进程稳定存活）；但经 NapCat 注入启动后立即退出 |
| `B:\Program Files\Tencent\QQNT\QQ.exe` | 9.9.20-36580 | ✅（低于建议下限 40768） | 经 NapCat 注入启动后同样退出 |
| `C:\Program Files\Tencent\QQNT\QQ.exe`（主 QQ） | 9.9.20-36580 | 同上 | 未做破坏性测试，保持不动 |

**关键观察**：把 `qq_path.txt` 指向 9.9.23 后，NapCat 日志中
**不再出现「未找到对应版本的偏移数据」**，且推进到 `[PacketHandler] 初始化成功` 与
`等待网络连接...` —— 说明**偏移表问题确实被绕过**。但这些替代 QQ 在注入启动后会退出，推测原因是
**它们并非纯净安装**：OneKey 包内的 QQ 已被预先打过 NapCat 补丁，而启动器的
`sync_patch_package` 步骤会用本机 NapCat 的 `qqnt.json` 覆盖其 `resources/app/package.json`，
两套补丁冲突导致 QQ 自行退出。

#### 五、推荐解决路径（按优先级）

1. **升级 NapCat 到 4.18.28**（`NapCat.Shell.Windows.Node.zip`，111 MB），同时把 QQ 换到
   **9.9.26-44343 或更低**的纯净安装。这是唯一能同时满足「版本在表内」且「安装纯净」的组合。
2. 若只想先验证群文件功能：把 `qq_path.txt` 指向一个**纯净的**、版本 ≤ 9.9.26 的 QQ 安装，
   且**不要**使用 OneKey 包内已被补丁的 QQ。
3. 在完成上述任一项之前，**群文件下载不可用**；其余全部功能（消息接管、AI 草稿、群聊简报、
   文档综述）均正常。已恢复 `qq_path.txt` 为 `A:\NTQQ\QQ.exe`，保持原有可用状态。

#### 六、给后续排查者的提示

- NapCat 日志目录 `B:\EazyQQ\napcat\logs\` 每次启动生成一个文件，
  `grep "偏移数据"` 可一眼判断当前 QQ 版本是否被支持。
- 从 `napcat.mjs` 中 `grep -oE "9\.9\.[0-9]+-[0-9]+-x64"` 可直接得到本机 NapCat 的完整支持版本清单，
  比查文档更可靠。
- **从本 shell 启动 NapCat 必须用 `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`**，
  否则进程会随命令结束被回收，表现为「QQ 启动后立刻消失」，极易误判为崩溃。

---

### ISSUE-013 补充：`--no-sandbox` 已被证明是**必需**的（不是沙箱假象）

#### 之前的不确定性
我此前声明过一个方法学限制：**我启动的所有程序都经过沙箱**，因此无法区分
「机器级 Chromium GPU 问题」与「只有从沙箱启动才会崩」。这个不确定性现已消除——
Bash 工具支持在沙箱外执行，我用它做了对照。

#### 实验设计（修正了前两轮的缺陷）
前两轮对比都不可信：
1. 第一轮把 `--no-sandbox` 写进了 `tauri.conf.json` 并编译进二进制，
   于是「对照组」其实也带着该参数，五个候选**全部**报"可用"，毫无区分度；
2. 第二轮在两臂之间重启了 Vite，而 Vite 的依赖预构建状态不同
   （请求日志显示 React 依赖是否被预构建会变），构成第二个干扰变量。

本轮：`tauri.conf.json` 中**不含**任何额外参数，改用
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 环境变量在**同一个二进制**上开关该参数，
**两臂共用同一个 Vite 实例**，且全部在**沙箱外**执行。

#### 结果

| 配置 | 新增 Crashpad 转储 | `get_contacts` 调用 | 12s 兜底是否触发 |
| :--- | :---: | :---: | :---: |
| A) 无 `--no-sandbox` | 0 | **+0（React 从未挂载）** | 是 |
| B) 有 `--no-sandbox` | 0 | **+2（前端正常运行）** | 否 |

#### 两条重要结论

1. **`--no-sandbox` 是必需的**，即使脱离沙箱也如此。因此这**不是**我的测试环境造成的假象，
   而是本机真实存在的问题。关闭渲染进程沙箱属于**真实的安全降级**，必须继续从根上解决
   （优先级见 README：临时停用 Windhawk → 禁用 GameViewer 虚拟显示器适配器 → 更新显卡驱动）。

2. **「没有崩溃转储」不能作为「WebView 正常」的证据**。A 臂没有产生任何转储，
   但前端完全没有执行——失败表现为 **WebView 无法执行 JS**，而不一定表现为进程崩溃。
   这也是我前几轮反复误判的原因：把「无转储」当成了健康。
   可靠的判据只有一个：**前端是否真的调用了后端接口**（`IPC: get_contacts` 探针），
   或 Vite 是否收到了完整模块请求（含 `node_modules/.vite/deps/react*.js`）。

#### 附带发现的排错线索
A 臂的 Vite 请求日志显示：`index.html`、`App.tsx`、全部视图组件、`api/client.ts`、`index.css`
**都被成功取到**，唯独**没有任何 `node_modules/.vite/deps/react*.js` 请求**——
即模块图走通了但 React 依赖没加载，因此组件树从未执行。
这与「页面取到一半渲染进程死亡」是**两种不同的失败形态**，排查时值得区分：
前者是依赖/网络层，后者是渲染进程层。

---

### ISSUE-019 结案：偏移表阻塞已被 NapCat 升级解决（附精确确证）

#### 结论
升级 NapCat 4.9.81 → 4.18.28 后，**偏移数据缺失问题已消除**，且**不需要降级 QQ**。

#### 精确确证（读代码，不靠推断）
NapCat 的查找逻辑是 `dF[QQ版本 + "-" + 架构]`，取不到 `send`/`recv` 就 `return false`（**没有回退**）。
直接在新版 `napcat.mjs` 的偏移表中检索实际键：

```
"9.9.35-52892-x64": { send: "C361DF",  recv: "246F3A1" }   ← 命中
"9.9.28-46928-x64": { send: "0A6D66C", recv: "1E9890D" }   ← 命中
```

- `9.9.35-52892-x64` 是旧版 NapCat 报告的那个「未找到偏移数据」的版本；
- `9.9.28-46928-x64` 是本机 `A:\NTQQ` 的实际版本。
- **两者现在都在表内**，因此无论 NapCat 用哪种方式探测版本，查找都会成功。

#### 版本对照
| NapCat | 偏移表 x64 条目 | 上限 |
| :--- | :---: | :--- |
| 4.9.81（原） | 53 | 9.9.25-42941 ❌ 不含本机 QQ |
| **4.18.28（现）** | **70** | **9.9.35-52892** ✅ 含本机 QQ |

#### 顺带解决的诊断障碍
原 `config/napcat.json` 中 `fileLog = False`，这是升级后 `logs/` 不再产生新文件的**原因**
（不是升级把日志搞坏了）。本次排查已将其改为 `True`、`fileLogLevel = debug`，
后续如需日志可直接查看 `B:\EazyQQ\napcat\logs\`。

#### 仍未完成的一步（需用户操作）
QQ 进程在本机**由我的 shell 启动时**会死于 Chromium 的 GPU 进程崩溃，因此
**群文件下载的端到端验证仍未完成**。已确认 NapCat 能启动到登录阶段
（`cache/qrcode.png` 被实时生成，说明已进入等待扫码状态），
即**协议端本身是通的**，卡点只在 QQ 的进程存活。

用户在自己终端启动后，用以下命令即可验证本 issue 是否真正闭环：

```bash
eazyqq_cli files --target <群号>          # 应能列出群文件
eazyqq_cli file-download --target <群号> --file-id <文件ID>   # 关键：此前必失败
```

---

### ISSUE-023: NapCat 启动后 QQ 在 QQNT.dll 内以断点异常崩溃（协议端无法使用）

> **⚠️ 本条已作废（2026-09-22 21:30）。**
> 经用户确认，**协议端一直可以正常使用**，本条所述的"故障"不成立。
> 撤回的结论：协议端不可用、WebUI 只活 2~5 秒、混装是元凶、旧版也不稳定、
> "符合 QQ 完整性校验拒绝注入"、"不在应用代码可修范围内"。
>
> **根本方法问题**：本条的大量判断建立在端口轮询与进程计数这类**不可靠探针**之上，
> 而用户指出这些探测不真实；且曾用 5 秒间隔采样错过短事件后直接推断"未发生"，
> 并把"未观察到"当成"不存在"。**应用是否可用应以用户实际使用为准。**
>
> **下方内容仅作过程记录保留**，说明哪些路径已走过、不必重复；
> 其中观测数据受上述缺陷影响，**不可作为事实依据**。

#### 现象
NapCat 每次启动都能起来、WebUI 也会监听 6099，但**只活 2~5 秒**就消失；`napcat/logs/` 始终为空
（配置里 `fileLog: true`、级别 `debug`）。应用因此每 ~100 秒重启一次，无限循环。

#### 定位过程（含被推翻的假设，保留以省后人重复）
用 1 秒间隔采样端口才抓到窗口（此前 5 秒采样系统性错过 2~5 秒的事件）：

```
13:12:43 ↑ 6099 出现    13:12:45 ↓ 消失
13:14:23 ↑ 6099 出现    13:14:28 ↓ 消失
13:16:26  QQ进程=8
13:17:51  QQ进程=12  ★ 6099 出现     16:10:04 ★ 6099 出现（QQ 4）
13:17:54  QQ进程=12    6099 消失     16:10:09   6099 消失（QQ 0）
13:17:58  QQ进程=8
```

**最终在 Windows 事件日志里拿到直接证据：**

```
出错应用程序名称： QQ.exe，版本： 9.9.28.46928
出错模块名称：   QQNT.dll
异常代码： 0x80000003          ← EXCEPTION_BREAKPOINT（断点异常）
错误偏移： 0x0000000007776276
Faulting 模块路径： A:/NTQQ/versions/9.9.35-52892/QQNT.dll
```

**QQ 不是"退出"，是在 QQNT.dll 里崩溃。** 每次启动 NapCat 都对应一次崩溃（15:46 / 16:07 / 16:10 / 16:15 / 16:18）。

#### 已排除（均做过对照实验，不是推测）
| 假设 | 实验 | 结论 |
| :--- | :--- | :--- |
| 用户自己的 QQ 在运行导致单实例冲突 | 关掉 QQ（0 进程）后重试 | **排除**，照样崩 |
| Windhawk 全局注入 | 退出 Windhawk（0 进程）后重试 | **排除**，照样崩 |
| 我的执行沙箱 | 沙箱外执行 | **排除**，照样崩 |
| `o3HookMode: 1`（非默认值） | 改为默认 0 后重试 | **排除**，照样崩 |
| `packetBackend` 偏移补丁出错 | 改为 `disable` 后重试 | **排除**，照样崩 |
| LiteLoaderQQNT 残留 | 全盘搜索 + `package.json` 的 `main` 为标准路径 | **排除**，无残留 |
| NapCat 偏移表与 QQNT.dll 不匹配 | 按 PE 节表核对：`send 0xC361DF` 处是 `48 89 CA F3 0F 5C`、`recv 0x246F3A1` 处是 `57 53 48 83 EC 20`，**均为合法 x64 函数序言** | **排除**，偏移是对的 |
| QQ 版本不受支持 | `9.9.35-52892` 在 NapCat 的支持表内（共 73 个版本，最后一项就是它） | **排除**，声称支持 |

崩溃地址在磁盘上是正常代码（`0F 84 44 FF FF FF` = `JZ`），**说明断点是运行时在内存里打上去的**——
与"NapCat 的 hook 改写 QQNT.dll 后，QQ 的完整性校验触发退出"这一已知问题族吻合
（参见 NapCat issue #798 / #999："已触发QQ文件校验退出函数"）。

#### 当前判断（**未定论**）
NapCat 4.18.28 的 hook 与该 QQ 构建（9.9.35-52892）配合异常。证据：
- 已是官方最新版 NapCat，无更新可升；
- NapCat 发布页明确写"推荐使用 40768+ 版本"，下载链接指向 **QQ 9.9.26-44343**，
  即官方按该版本测试，而本机 QQ 已自动升级到 9.9.35-52892。

**下一步建议**：把 QQ 换成 NapCat 官方推荐的 9.9.26-44343 再试。

#### 本轮已修复（不依赖上述结论）
1. `services::napcat::launch_if_needed` 曾用错误参数启动（把 QQ 号当 QQ 路径传），
   与 `napcat_boot` 构成两个互相竞争的启动器 —— 已统一为一个；
2. `napcat_boot::start()` 成功路径从不计入失败，导致"连续失败 3 次就停"的保护**从未生效**，
   无限重启且每次多拉起 4 个 QQ 进程 —— 已补 `note_unready` 并接入链路监控；
3. **启动器的输出此前完全未捕获**，NapCat 的死因无法从应用侧看到 —— 已写入
   `<账号数据>/napcat_logs/boot-<时间戳>.log`；
4. `chain-status` 与链路监控是两份实现且已漂移，前者只报"网络错误" —— 已抽出
   `chain::explain_napcat_failure` 共用，现在会指出"首先卡在「X」"。

#### 解决方向：私有 QQ 环境（不再复用用户的 QQ）

**实测体积**：完整 QQ 环境 `A:/NTQQ` = **1177 MB**（`versions/` 1170 MB，其中 `QQNT.dll` 单文件 204.7 MB）；
NapCat 本体 `NapCat.Shell.zip` = 28.1 MB；官方 `OneKey.zip` 仅 1.0 MB（只含引导器，QQ 现场下载）。

**为什么不直接打包 QQ**：体积上可接受（~1.2 GB），但 QQ 是腾讯的软件，**重新分发有法律问题**。

**采用方案（与 NapCat 官方 OneKey 一致）**：
安装时从**腾讯官方 CDN** 拉取一个**指定版本**的 QQ，装到应用私有目录，不使用用户的 QQ。

**这样同时解决四个问题**（其中前两个是本 ISSUE 与 ISSUE-013 反复踩到的）：
1. 用户的 QQ 在运行 → 进程冲突；私有副本与用户的 QQ 互不相干；
2. 用户 QQ 自动升级 → NapCat 失效；私有副本**版本由我们锁定**，不会被动漂移；
3. 用户装过 LLQQNT 等插件污染 QQNT.dll → 私有副本干净；
4. 用户 QQ 装在非标准路径 / 无注册表登记 → 官方启动器靠 `reg query` 找 QQ
   （见 `launcher-user.bat`），私有副本由我们自己写路径，**不依赖注册表**。

**代码改动点**（接口已现成：`napcat_boot::configured_qq_path()` 本就从 `config/qq_path.txt` 读）：
1. 新增 `services/qq_provision.rs`：检测 → 下载指定版本 → 解压到 `<账号数据>/qq/<版本>/` → 校验；
2. 将 `qq_path.txt` 指向私有副本，而非 `A:/NTQQ/QQ.exe`；
3. 配置中记录**锁定的 QQ 版本号**，与 NapCat 版本一起做兼容性前置校验；
4. 首次运行引导完成该步骤；保留"使用我自己的 QQ"选项，但明确标注不稳定。

**代价**：安装时多下载约 1.2 GB；磁盘占用约 1.2 GB；NapCat 升级时需同步更新锁定的 QQ 版本。

#### 补充：回退到 4.9.81 无效，且"以前是好的"需要修正

**已执行**：把 `napcat/` 整体恢复为**版本一致的 4.9.81**（`NapCatWinBootMain.exe` 35328、
`NapCatWinBootHook.dll` 20992、`napcat.mjs` 3924813，三者与官方 v4.9.81 发布包逐字节吻合），
配置保留。**结果：仍然崩溃**（16:39:41，同样的 `0x80000003` in `QQNT.dll`）。

**"混装"假设因此被推翻**：混装（4.9.81 hook + 4.18.28 JS）确实是问题，但**不是唯一问题**，
换成一致的 4.9.81 并不能恢复。

**"以前是好的"也要修正。** 旧版 4.9.81 留下的日志（应用收集在账号目录 `napcat_logs/`）显示：

```
08:39:06 / 08:40:36 / 08:46:15 ×2 / 08:46:36 ×2 / 08:46:55 ×2 / 08:47:15 / 08:47:16 / 08:47:36 ×2
```

**8 分钟内被重启十余次**；08:47 那两条日志显示 WebUI 分别起在 **6099 和 6100**（多实例抢端口）。
即：**旧版也一直不稳定，只是它能走到"WebUI 就绪"；新版直接让 QQ 崩溃。**
最后一份正常草稿是 `09-22 00:31`，说明凌晨还能用，到早上 08:39 已在重启循环中。

**配置差异**（升级前备份 vs 现在）只有一处：`napcat.json` 的 `fileLog` 由 `false` 变 `true`。
这也解释了为什么旧版从不在 `napcat/logs/` 写文件日志——**旧版的日志一直是靠应用收集其控制台输出**。
配置不是崩溃原因。

**至此仍未定论。** 已排除项累积到 10 项（用户 QQ / Windhawk / 沙箱 / o3HookMode /
packetBackend / LLQQNT 残留 / 偏移表不匹配 / 版本不支持 / 混装 / 配置差异）。
崩溃形态一致（`QQNT.dll` 内 `0x80000003`），**高度符合"QQ 完整性校验拒绝注入"**这一族，
属于 NapCat 与 QQ 构建之间的兼容问题，**不在本应用代码可修范围内**。

---

### ISSUE-023: 协议重启后未自动触发快速登录，导致每次均需重复扫码
- **首次发现时间**: 2026-09-22
- **触发场景**: 用户已扫码登录过账号 `462564834`（凌），应用或协议进程重启后，登录页依旧重新展示二维码要求重新扫码。
- **技术根因剖析**:
  1. NapCat 底层提供 `/api/QQLogin/GetQuickLoginListNew` 与 `/api/QQLogin/SetQuickLogin` 接口，并已缓存了本机的 3 个已登录账号。
  2. 原后端 `get_protocol_status` 与前端仅在 CLI 中实现了快速登录指令，主链路在未登录时直接回落至请求静态二维码，从不自动复用历史凭据，也未在前端展示快速登录账号列表。
- **修复方案与措施**:
  1. 后端 `get_protocol_status` 读取 `bootstrap.json` 中的 `lastAccount`（如 `462564834`），若命中 NapCat 已记忆账号则自动发起 `set_quick_login` 快速静默握手；
  2. 新增 `quick_login` 及 `get_quick_login_accounts` Tauri 指令并在 `ProtocolStatusDto` 中透出记忆账号列表；
  3. 前端 `LoginView.tsx` 动态展示已记住的账号卡片与「一键免扫码登录」按钮，支持多账号一键切换。

---

### ISSUE-024: 登录页将协议端启动加载等待态误用红色「刷新失败」引起用户焦虑
- **首次发现时间**: 2026-09-22
- **触发场景**: 启动或刷新二维码时，协议端仍在拉起或未就绪，界面出现显眼的红色胶囊「刷新失败」以及红色异常文本。
- **技术根因剖析**:
  - 前端 `LoginView.tsx` 对所有 `error` 统一以 `text-rose-600`、`bg-rose-50` 和 `AlertCircle` 渲染为严重故障，但此时底层仅为「正在启动加载与准备本地环境」的正常中间过渡态，导致用户误认为系统损坏。
- **修复方案与措施**:
  1. 重构 `LoginView.tsx` 视觉反馈机制：将启动中的过渡状态重构为天蓝色系（`bg-sky-50`、`text-sky-700`、`Loader2` 旋转微动效），标注文案为「服务加载中」；
  2. 二维码加载盒内采用雷达呼吸与柔和旋转骨架反馈，彻底消除突兀的红色报警信息；
  3. 全局按钮与交互卡片注入统一的触感微反馈（`active:scale-95`、`cursor-pointer`、平滑阴影过渡）。

---

### ISSUE-025: OpenCode 官方免费供应商支持及模型动态拉取（拒绝硬编码）
- **首次发现时间**: 2026-09-22
- **触发场景**: 用户需要直接使用 OpenCode 官方免费供应商与模型，由于官方模型经常动态更新迭代，硬编码模型名称无法持久工作。
- **技术根因剖析**:
  1. OpenCode 官方 Zen 端点为 `https://opencode.ai/zen/v1`，兼容 OpenAI 规范；
  2. 本机已在 `A:\DevEnv\nvim-home\data\opencode\auth.json` 中配置官方凭证，但系统原先缺乏自动提取机制，迫使用户手动复制长密钥；
  3. 大模型供应商的模型列表随时变化，硬编码会导致新上线模型不可用、下线模型报错。
- **修复方案与措施**:
  1. 后端新增 `detect_opencode_auth_key()` 自动检测并读取本机 OpenCode 凭证；
  2. 后端新增 `fetch_provider_models` 指令动态请求端点 `/models` 获取实时模型列表；
  3. 前端设置页集成「从接口动态获取可用模型」与模型即时切换组件，自动标记官方 `-free` 免费模型，支持任意输入与动态点选。

---

### ISSUE-026: 群文件页面状态未记忆与缺少「查看摘要」闭环入口
- **首次发现时间**: 2026-09-22
- **触发场景**: 
  1. 页面重载或切换视图后，群文件下拉选框重置为空（显示「选择要同步的群聊…」与「暂无群文件」空白占位），必须用户手动重新选择；
  2. 文件生成摘要后，文件列表项依然只显示「AI 智能摘要」按钮，没有任何「查看摘要」按钮可二次调出结果。
- **技术根因剖析**:
  1. 前端 `App.tsx` 中的 `selectedGroupId` 初始状态硬编码为 `""`，未持久化记忆用户上次选中的群聊；
  2. 摘要结果仅作为临时状态保存在内存中，没有与具体 `fileId` 或本地缓存建立映射关联，导致已摘要状态无法显式反馈至列表项中。
- **修复方案与措施**:
  1. 在 `localStorage` 中持久化 `eazyqq_selected_group_id`，应用启动与组件加载时自动恢复上次选中的群聊并静默预加载文件；
  2. 在 `FilesView.tsx` 中建立 `eazyqq_file_summaries_cache` 持久化字典，已生成摘要的文件高亮呈现「查看摘要」紫色按钮与「重新生成」图标；
  3. 优化摘要侧边抽屉：增加复制全文、关闭弹窗、重新提炼等完整交互闭环。

---

### ISSUE-027: QQ 漫游历史消息未同步、多模型配置单槽覆盖与全链路缺少一键小白修复闭环
- **首次发现时间**: 2026-09-22
- **触发场景**:
  1. 打开联系人私聊或群聊会话时，历史聊天记录全为空白（仅能看见开机后接收的个别新消息）；
  2. 设置页切换大模型供应商时，用户此前微调的自定义模型名称、API Key 与 URL 被直接重置为硬编码默认预设；
  3. 链路自检中出现异常或断点时，缺乏一键自动安装、一键唤醒修复的操作入口。
- **技术根因剖析**:
  1. `get_messages` 原先仅从本地 SQLite `messages_log` 取数，未对接 OneBot 11 的漫游历史接口；
  2. `app_config.ai` 原仅维护单个平铺字段结构，切换即覆盖；
  3. `napcat_boot::restart` 未对前端暴露，各依赖项无一键处置导流。
- **修复方案与措施**:
  1. `OneBotClient` 扩充 `get_friend_msg_history` 与 `get_group_msg_history` 接口，在 `get_messages` 查询时无感从 NapCat 同步漫游消息并落库去重合并；
  2. `app_config.ai` 引入 `providers` 隔离映射字典，各供应商独立持久化模型、URL 与 Key；
  3. 后端增加 `restart_napcat` Tauri 指令，设置页为 NTQQ 官方下载、OpenCode 免费通道切换、NapCat 协议端一键唤醒提供完整 1-Click 闭环；
  4. 移除联系人与聊天抽屉中所有的「唯一指定测试联系人」特殊视觉标签，恢复统一干净外观。


