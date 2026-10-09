# 0.5.0 云端发布验收记录

日期：2026-10-09（北京时间）。当前状态：修复发布流程后重新运行云端验收，公开资产仍由验收门禁控制。

## 已提交源代码

版本标签 `v0.5.0` 对应 `82c44904a7726fa916d25b58de645049ae6d9376`。本地已通过类型、接口、96 项 Rust、10 项前端、24 项 Release CLI 及原生账号/缓存/窗口/F1/版本验收，6 个分发文件与 SHA256 已生成。

## 第一轮云端故障

[运行 37804906313](https://github.com/LING71671/EazyQQ/actions/runs/37804906313) 在 pnpm 安装步骤失败，日志为 `onlyBuiltDependencies?.sort is not a function`，其他构建/发布步骤均未执行。

旧 `pnpm/action-setup@v4` 的内置安装器与 pnpm 11 及已有构建配置不兼容。根据 [官方 v6.1.0 文档](https://github.com/pnpm/action-setup/blob/v6.1.0/README.md) 采用支持 pnpm 11 的 `v6.1.0`，保留 packageManager 声明和冻结锁文件安装。

## 标签与流程的处理

不强制改写 `v0.5.0` 标签。CI 修复提交在 main，通过 main 的新版发布流程手动派发，传入同一标签；checkout 仍检出已经验收的标签源代码。流程改动只影响构建环境，不改变发布应用来源。

最终工作流链接、发布资产和下载校验在完成后追加记录，不能把第一轮失败写成发布成功。

## 第二轮云端故障

[运行 37805753322](https://github.com/LING71671/EazyQQ/actions/runs/37805753322) 已通过新版 pnpm 安装、Rust/Python 环境、版本和依赖准备，类型检查也通过，但生成契约比较失败。

Windows runner 默认 checkout 为 CRLF，生成器产物按 LF 比较，造成相同 JSON 被误报 `Contract drift`。发布流程在 checkout 前设定 canonical LF；main 增加 `.gitattributes`，生成器读取时规范 CRLF。不是跳过校验或重新生成掩盖接口变更，仍比较完整内容。

标签来源保持不变；新版流程以 canonical LF 检出同一标签。main 的兼容修复改善后续开发者 checkout，不改变本次发布应用行为。

## 第三轮云端验收

[运行 37806495896](https://github.com/LING71671/EazyQQ/actions/runs/37806495896) 已通过类型、契约、前端、Rust、完整安装包构建及 24 项 Release CLI 验收。原生驱动连接 WebView 调试端口超时，尚未建立公开 Release。

本机同版本原生验收已通过，需定位 runner 的环境差异。验证驱动改为明确绕过系统代理访问本机端口，使用软件渲染，并为冷启动提供更长上限；失败时输出应用退出码和隔离日志。不能仅凭超时断言某一具体根因已修复。

后续工作流从其自身修订读取验证驱动，应用仍构建同一标签。驱动变更不进入应用二进制；失败日志保存供定位，Rust 编译缓存也在失败时保存，避免无谓重复冷编译。

## 第四轮定位结果

[运行 37809691261](https://github.com/LING71671/EazyQQ/actions/runs/37809691261) 再次通过构建和 24 项 CLI，原生入口仍超时。驱动已明确绕过代理，日志报告系统未配置代理，因此不能把代理当成已确认原因。

应用进程当时仍在运行。中文日志输出被 runner 默认 cp1252 编码再次中断，上传动作默认忽略隐藏目录也导致日志未保存。现在将 Python 输出固定为 UTF-8，并允许仅指定路径的隔离日志上传；增加 WebView2 运行时版本信息。继续以实际日志定位，不降低原生验收门禁。

## 第五轮实际日志

[运行 37812548639](https://github.com/LING71671/EazyQQ/actions/runs/37812548639) 提供完整日志。应用成功创建窗口和托盘，前端调用联系人接口，确认模拟账号身份，WebSocket 与调度均运行。WebView2 153 已安装。问题限于驱动访问调试 HTTP 端口，不能继续称为“应用启动失败”。

为避免重复编译，增加只读取未公开候选二进制的短诊断流程；候选仍为已在本机通过原生验收的 0.5.0。记录浏览器进程的调试参数和端口监听，不输出环境凭据。候选 Release 仍是 draft，临时诊断二进制须在公开前移除。

短诊断日志中，浏览器进程未出现指定调试端口参数，也没有相应监听。采用 [Microsoft 文档支持的每应用注册表配置](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code#using-a-registry-value) 补充 runner 的调试入口：仅在 `GITHUB_ACTIONS=true` 时设置当前用户、当前测试 exe 的 AdditionalBrowserArguments，结束后恢复原值；本机验证和用户安装均不修改注册表。

## 已确认的调试配置限制

Microsoft 在 2026-09 更新的 WebView2 安全规范明确：提升权限进程忽略 WEBVIEW2_* 环境变量及 HKCU AdditionalBrowserArguments，而接受 HKLM 策略和程序内参数。参见 https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags 。这与 runner 中全部调试参数缺失、应用前端仍正常的现象一致。

临时托管 CI 验证改用 HKLM 的本应用配置，并在结束后恢复原值。本机仍使用普通进程环境，不设置机器注册表。先前 HKCU 尝试是诊断过程，当前实现不再使用它。

[运行 37864516213](https://github.com/LING71671/EazyQQ/actions/runs/37864516213) 已成功完成全部原生验证，证明问题为托管提升权限场景的调试配置机制；应用启动、版本、身份往返、缓存、窗口和 F1 均通过。仍须将用户随后追加的展示与动效修改纳入最终版本，不能公开旧候选二进制。
