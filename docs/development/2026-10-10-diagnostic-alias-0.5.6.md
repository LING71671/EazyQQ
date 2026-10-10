# 0.5.6 安装目录别名诊断修正

## 起始状态与证据

0.5.5 已公开发布并经旧 CLI 更新入口自动安装至 A:/EazyQQ。原生 Qt 协议模块、WebUI 和 OneBot 身份已恢复，但 `napcat-doctor` 的 entry_resolution 误报失败：配置的 C:/Program Files/Tencent/QQNT 实际指向 B 盘，入口按实际 B 盘 app 目录生成，检查器却拼接逻辑 C 盘目录。

源代码已修正为先解析 app 目录的最终物理路径，再拼接 package.main。模拟同盘深层目录别名也能稳定复现同一错误，不依赖本机盘符；新回归通过。既有加载链路不重启，不生成新尝试编号。

## CLI 语义

已登录时 `qr` 不应尝试生成或刷新二维码。命令现在复用只读身份证据，返回 `{loggedIn:true,uin,nickname}`；未登录流程保留。增加普通查询与 refresh 两项已登录冒烟，避免无意义的刷新；文本状态用当前 WebUI 与 OneBot 实际端点。

## 已验证结果

修复源码的完整 Rust 为 121 项库测试、2 项 CLI 测试，共 123 项；26 项 CLI 冒烟通过。用该构建对用户实际运行目录执行只读诊断，firstFailure=null，入口、加载器执行、模块导入、受管理 Job 和 WebUI 均 passed。身份仍由独立 status/login-info 确认，而不是将文件快照 ready 改成 true。

2026-10-10 09:08 的只读状态仍 loggedIn=true，OneBot 可达，WebUI coreReady=true、loginPhase=ready。协议 QQ 主进程 PID 29952、2026-10-09 22:53:02.683799 启动时间保持。当前外部 A:/NTQQ 主进程 PID 21948、08:54:39.529067 启动时间单独记录并保护；不将昨晚的陈旧 PID 用作终止目标。

证据为 `.test-runtime/diagnostic-alias-live-proof.json`、`diagnostic-alias-rust.log` 和 `diagnostic-alias-smoke.log`。没有登录请求、协议重启、消息发送或自动回复配置更改。

## 后续交付

当前安装仍为 0.5.5；0.5.6 清单、生成契约及中文文档已准备，完整构建、发布门禁、资产下载校验与安装结果仍待实际执行，不能提前记为完成。保持已公开标签和资产不变，构建继续放在 A 盘。

本地 0.5.6 安装包已生成，26703461 字节；六项分发文件打包完成，来源提交尚待版本准备提交后由云端生成。发布 CLI 26 项冒烟、真实 Tauri 模拟协议的账号往返/延迟响应/缓存/窗口验收、25 项前端测试及类型/契约检查通过。证据为 `.test-runtime/release-0.5.6-build.log`、`release-0.5.6-package.log`、`release-0.5.6-smoke.log`、`release-0.5.6-native.log` 和 `release-0.5.6-frontend.log`。
