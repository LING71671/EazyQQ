# EazyQQ

面向 Windows 的 QQ 助手，提供桌面端、独立 CLI 和 stdio MCP。当前版本为 **0.5.0**，基于 Tauri、Rust、React、TypeScript、SQLite 和 NapCat / OneBot 11。

## 安装与使用

从 [GitHub Releases](https://github.com/LING71671/EazyQQ/releases) 下载桌面安装包，或下载 `eazyqq-cli-windows-x64.zip` 解压使用。需要本机安装官方 QQNT；使用 OpenCode 免费模型还需要安装原生 OpenCode。

桌面端的账号管理支持输入多个 QQ 号、批量启动或登录、独立扫码及账号切换。快速凭据失效时显示目标账号二维码，原有账号保持连接；目标身份确认后才提交切换。

CLI 压缩包目录如下：

```text
bin/eazyqq_cli.exe
bin/ezq.exe
napcat/
docs/
README.md
```

`ezq.exe` 是同一 CLI 的兼容文件名。桌面安装包也包含 `eazyqq_cli.exe`，与 GUI 共用安装目录的数据根路径。把可执行文件目录加入 PATH，或使用完整路径调用。

```powershell
eazyqq_cli accounts add --uin 10001 --json
eazyqq_cli accounts qr --uin 10001 --json
eazyqq_cli accounts select --uin 10001 --json
eazyqq_cli --account 10001 status --json
eazyqq_cli accounts start --uins 10001,10002 --dry-run --json
eazyqq_cli run --all
```

示例账号需要替换成自己的 QQ 号。`accounts login` 仅登录；`accounts select` 同时切换当前账号。`--account` 只改变该次 CLI 调用的上下文。批量结果逐账号返回，忘记账号保留数据。

## 功能与边界

- **多账号**：独立 HTTP、WebSocket、WebUI 端口，独立协议配置、数据库、AI 会话和浏览器缓存；工作租约避免重复消费消息。
- **消息与草稿**：默认拒绝未授权会话；支持自动回复、草稿待审、仅总结和忽略模式。发送、草稿审核和自动回复规则会影响真实 QQ 联系人。
- **简报与文件**：支持定时及手动简报、流式输出、群文件索引、下载和文档提取。实际支持的文件格式与参数以 CLI schema 为准。
- **模型与诊断**：OpenCode 免费模型通过原生运行时调用；自定义兼容服务通过 HTTP 调用。状态、登录和诊断共用身份校验，待扫码和未测试推理显示未知。
- **恢复与升级**：恢复按故障环节处理，保留已登录和外部管理的 QQ。只有 EazyQQ 自己创建的进程树可以定向停止；安装脚本不按 QQ 进程名清理。

```powershell
eazyqq_cli ai-models --json
eazyqq_cli ai-set --provider opencode --model big-pickle --json
eazyqq_cli ai-test --prompt "Reply only OK" --json
eazyqq_cli chain-status --json
eazyqq_cli repair --json
eazyqq_cli schema --json
eazyqq_cli mcp
```

模型目录不等于推理可用性，模型可能调整、限流或下线。周期监控不主动消耗模型推理额度；`ai-test` 和 `health --deep` 是显式推理检查。QQ 可能要求重新扫码认证。

## 从源码开发

环境：Windows x64、Node.js、pnpm、Rust、MSVC 构建工具和官方 QQNT。协议资源来自本地 `napcat/` 或 `src-tauri/resources/napcat/`；发布流程会准备官方 NapCat 资源。

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
pwsh -NoProfile -File scripts/cargo.ps1 build --bin eazyqq_cli
pnpm tauri:build
pnpm release:package
```

Windows 包装脚本自动定位 MSVC；`dev.cmd` 使用相同入口。发布包位于 `output/release/0.5.0/`，包含桌面安装包、CLI 压缩包、独立 CLI、校验和及更新清单。

## 验收与文档

```powershell
pnpm version:check
pnpm contracts:check
pnpm typecheck
pnpm test
pnpm test:rust
pnpm build
pnpm test:smoke
pnpm test:native
```

CLI 冒烟验收使用模拟 HTTP/WebSocket 和独立数据库。原生缓存验收使用真实 Tauri 桌面、两个模拟账号和独立 WebView 数据目录，不需要手机扫码。`EAZYQQ_ROOT` 可指定隔离的数据根目录。

- [产品规格](docs/PRODUCT.md)、[交互设计](docs/DESIGN.md)：用户能力与状态语义。
- [架构](docs/ARCHITECTURE.md)：目录、依赖与数据边界。
- [接口总览](docs/API_SPECIFICATION.md)、[桌面 IPC](docs/api/IPC.md)、[CLI](docs/api/CLI.md)、[覆盖矩阵](docs/api/COVERAGE.md)：契约与使用方法。
- [开发记录](docs/development/2026-10-08-reliability.md)、[0.5.0 发布记录](docs/development/2026-10-08-release-0.5.0.md)：问题、决策与验收证据。
- [贡献指南](CONTRIBUTING.md)、[变更日志](CHANGELOG.md)、[发布流程](docs/release/RELEASING.md)：后续维护入口。

协议数据默认保存在本机；选择云端模型时，被允许处理的内容会交给对应服务。密钥、个人协议配置和运行日志不进入发布包。许可证：[Apache 2.0](LICENSE)。
