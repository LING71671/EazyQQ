# CLI 使用与契约

适用于 0.5.3。源码构建：`pwsh -NoProfile -File scripts/cargo.ps1 build --bin eazyqq_cli`。桌面安装包包含同名 CLI；独立压缩包还提供 `ezq.exe` 兼容名。

```powershell
.\src-tauri\target\debug\eazyqq_cli.exe --account 10001 status --json
.\src-tauri\target\debug\eazyqq_cli.exe schema --json
```

## 输出与账号上下文

`--account` 同时选择本次调用的数据库与协议端点，不修改桌面启动选择。未知账号直接拒绝，不回退到其他账号。`accounts select` 验证目标身份后保存选择；已运行桌面保持原上下文，直到由 GUI 切换或重启。

退出码 0 表示操作完成；连接、参数、领域和部分批量失败使用非零退出码。`--json` 不混入日志；`--stream --json` 输出逐行 JSON。`mcp` 的 stdout 专用于 JSON-RPC。`help --json` 输出命令契约。

## 账号与进程

```powershell
eazyqq_cli accounts add --uin 10001 --nick 示例 --json
eazyqq_cli accounts add --uin 10002 --json
eazyqq_cli accounts start --uins 10001,10002 --dry-run --json
eazyqq_cli accounts start --uins 10001,10002 --json
eazyqq_cli accounts login --all --json
eazyqq_cli accounts qr --uin 10002 --refresh --json
eazyqq_cli accounts status --uin 10002 --json
eazyqq_cli accounts select --uin 10002 --json
eazyqq_cli accounts stop --uins 10001,10002 --json
eazyqq_cli accounts configure --uin 10002 --auto-start true --json
eazyqq_cli accounts forget --uin 10001 --json
```

`instances` 是 `accounts` 的兼容别名。批量支持 `--uin`、`--uins`、`--all` 与 `--dry-run`，结果逐账号返回。登录不等于选中；进程已启动不等于身份已确认。忘记账号保留目录，拒绝当前账号及仍受管理的运行账号。

`start`、`stop`、`logout`、`restart` 针对当前上下文。Windows Job 仅包含 EazyQQ 创建的进程树；独立监督进程使后续 CLI 仍可管理。没有所有权收据的外部会话被保留，PID 和进程名不能作为终止依据。

`restart` 遇到旧安装根 `resources/napcat` 时，先验证当前版本载荷并准备私有运行目录，再定向停止旧受管理 Job。准备失败不会停止旧树，外部监听服务拒绝重启。操作成功表示进程启动请求已完成，仍需通过 `status` 确认服务及账号身份；不能据此宣称已登录。

`status`、`login-info`、`quick-login-list`、`quick-login --uin`、`qr [--refresh] [--save path] [--browser]` 提供认证操作。快速请求被接受不等于完成登录。二维码可能是 URL、data URI 或 base64；普通 `qr --save` 只有在接收到 PNG data URI 时写入 PNG，否则保存返回值文本。

## 常驻工作进程

```powershell
eazyqq_cli --account 10001 run
eazyqq_cli run --all
```

工作进程读取当前账号规则、处理消息并调度简报；默认拒绝未授权对象。系统租约拒绝同账号第二个 GUI/CLI 消费者。`run --all` 为登记账号启动独立 CLI 工作进程，控制器检查子进程存活；Ctrl+C 结束工作进程，协议会话不随之统一退出。

## 联系人、消息、规则、草稿和文件

| 命令 | 用途及常用参数 |
| --- | --- |
| `contacts`、`friends`、`groups` | 名单，支持相应的类型、搜索、白名单筛选 |
| `rule` | `--target`、`--mode`、`--cooldown-seconds`、`--enabled`、`--summary` 等 |
| `batch-mode` | `--targets id,id`、`--mode` |
| `send`、`ask`、`history`、`mark-read` | 按命令使用 `--target`、`--text`、`--type`、`--limit` 等 |
| `drafts`、`draft-send`、`draft-dismiss`、`draft-regenerate` | 使用 `--id`，可编辑文本或提供重新生成指令 |
| `files`、`file-download`、`file-summarize`、`folder` | 群文件、下载、综述和目录操作 |

精确参数以 `schema --json` 为准，不能把表中的概述当成所有命令都支持的参数。模式为 `auto_reply`、`copilot`、`summary_only`、`ignore`。发送和批准草稿影响真实 QQ 联系人；`ask` 生成候选而不发送。文件路径由绑定账号推导，下载过程中执行大小上限。

## 模型、简报与诊断

```powershell
eazyqq_cli ai-models --json
eazyqq_cli ai-set --provider opencode --model big-pickle --json
eazyqq_cli ai-test --model big-pickle --prompt "Reply only OK" --json
eazyqq_cli --account 10001 summarize --target 20001 --hours 6 --stream --json
eazyqq_cli chain-status --json
eazyqq_cli repair --json
```

`ai-config`、`ai-set`、`ai-detect`、`ai-models`、`ai-test` 管理模型。OpenCode 使用原生运行时和真实模型目录，私有会话目录不与其他账号混用；需要工具审批的操作不会自动放行。`EAZYQQ_OPENCODE_BIN` 可指定原生可执行文件。免费模型仍受服务当前规则和限流约束。

`summaries`、`summarize`、`summary-delete --id`、`whitelist-groups`、`scheduler-tick` 管理简报。流式 JSON 为 `{type:"chunk",content}`，结束为 `{type:"complete",summary}`；原生 OpenCode 可能按完整文本段输出。

`health [--deep]`、`chain-status`、`repair`、`napcat-doctor`、`export`、`selftest`、`simulate`、`log-path`、`log-tail` 提供诊断。`health --deep` 和显式 AI 命令会请求推理；普通周期检查不会。CLI 无法仅凭本进程状态断言另一进程的调度或前端健康。

## 配置、升级和窗口

`config` 查询；`set-config --key app_config --file settings.json` 保存完整 JSON 配置；`config-audit` 检查字段消费。不要把完整配置接口当作服务端深度补丁。`qq-path [--set path]` 查询/更新 QQ 路径。

`updates versions|check-app|check-napcat` 查询版本；`updates install-app|install-napcat --confirm` 启动安装。协议升级需要停机并保留回滚备份。`window status|show|hide|minimize|maximize|toggle-maximize|move|drag|close` 操作运行中的桌面；移动接受 `--x`、`--y`。

## MCP 与环境

运行 `eazyqq_cli [--account QQ] mcp`，使用 JSON-RPC `initialize`、`tools/list`、`tools/call`。通知不返回响应，实际工具 schema 由服务返回，版本来自当前构建。

`EAZYQQ_ROOT` 指定数据与资源根目录；默认根据安装/源码位置解析。普通云模型密钥可来自专用配置或通用兼容服务环境变量，免费 OpenCode 不自动采用这些密钥。Python 环境与包管理约束仍遵循 `AGENTS.md`。

[命令 schema](cli.schema.json) 和 [覆盖矩阵](COVERAGE.md) 自动生成；`pnpm contracts:check` 验证前端 invoke、桌面处理函数、CLI 路由与文档的一致性。

## 0.5.3 更新边界

updates install-app|install-napcat --confirm 只接受当前官方稳定资产，--url 不能绕过校验。主程序精确匹配 EazyQQ_<version>_x64-setup.exe，核心精确匹配 NapCat.Shell.zip；校验大小与 SHA256，更新跨进程互斥。CLI 安装助手等待本次命令退出，不强制关闭另一个桌面或 QQ。协议更新要求相关会话停机，保留配置和备份。JSON stdout 只输出最终结果。
