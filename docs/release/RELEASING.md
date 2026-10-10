# EazyQQ 发布流程

当前源码版本 0.6.1。公开发布须对应完整验收的 Git 标签；本地安装包生成不等于已经发布。

0.5.3 的已知 QQ 加载入口故障见 `docs/development/2026-10-09-startup-provenance.md`。0.6.0 源码修复已通过隔离真实 QQ 的跨盘模块导入与 WebUI 验收；打包、安装和分发仍须通过本流程。不能将证据收集、假引导器回归或文件存在替代真实 QQ 加载及身份确认。

后续物理路径源码修复见 `docs/development/2026-10-09-physical-path-handoff.md`。发布门禁还须证明真实 QQ 执行新入口、导入协议模块并使 WebUI 可达；独立 Node 接收路径测试不能代替 QQ 验收。公开 0.5.3 标签和资产不覆盖。

设置 `CARGO_TARGET_DIR` 时，`release:package` 与 Cargo 使用同一目标目录；本机建议放在 A 盘。测试 CLI/GUI 用 `EAZYQQ_TEST_CLI`、`EAZYQQ_TEST_GUI` 指定该目录的 release 二进制，避免误测旧版本。

## 版本与文档

统一更新 package.json、Cargo.toml、Cargo.lock 和 tauri.conf.json；运行 `pnpm contracts:write` 更新生成 schema。`pnpm version:check` 校验清单，`pnpm contracts:check` 校验接口覆盖。

同步 README、CHANGELOG、产品、交互、架构、IPC、CLI、智能体指南及中文开发记录。二进制版本由清单提供；不要在界面或更新失败分支添加旧版本兜底。

## 必须通过的检查

```powershell
pnpm install --frozen-lockfile
pnpm presentation:check
pnpm typecheck
pnpm test
pnpm test:rust
pnpm contracts:check
pnpm tauri:build
$env:EAZYQQ_TEST_CLI = (Resolve-Path src-tauri/target/release/eazyqq_cli.exe).Path
pnpm test:smoke
pnpm test:native
pnpm release:package
git diff --check
```

原生验收使用真实桌面和模拟协议，验证账号往返、浏览器缓存及窗口还原；不触碰真实 QQ，不需要手机登录。首次构建在本机或 CI 可能需要下载依赖。

## 发布资产

`output/release/<version>/` 包含：

| 资产 | 用途 |
| --- | --- |
| `EazyQQ_<version>_x64-setup.exe` | 桌面安装包，包含 CLI |
| `eazyqq-cli-windows-x64.zip` | CLI、兼容别名、协议资源及中文文档 |
| `eazyqq_cli.exe`、`ezq.exe` | 独立可执行文件，协议资源需另行准备 |
| `SHA256SUMS.txt` | 各分发文件的 SHA256 |
| `latest.json` | 版本、来源修订、资产 URL、大小及散列 |

`latest.json` 是本项目的分发清单；应用现有版本检查读取 GitHub Releases API。协议资源检查拒绝个人配置、账号、凭据、日志、缓存、数据库与符号链接。不得把本机 napcat 的个人登录文件复制进 ZIP。

## GitHub 自动发布

发布工作流由 `v*` 标签或带标签参数的手动触发启动。它确认标签与清单版本相同，准备官方协议资源，执行类型/单元/CLI/原生测试，构建及打包后才建立并发布 Release。

```powershell
git tag -a v0.6.1 -m "Release 0.6.1"
git push origin main
git push origin v0.6.1
gh run list --workflow release.yml
```

使用已审查的提交，不强制覆盖公开版本标签。未公开候选仅在用户明确授权后允许更新，使用精确 force-with-lease 并记录旧、新来源；发布后的标签与资产保持不变。失败时定位真实日志，修复原因并更新开发记录，不将失败的构建当成已发布。发布说明来自 `docs/release/v<version>.md`，上传内容来自版本目录。

GitHub Windows runner 的提升权限 WebView2 忽略环境变量和 HKCU 调试参数。原生验证驱动仅在托管 CI 中临时设置本应用 HKLM 调试策略，结束后恢复；普通本机验证不修改注册表。原生检查仍是发布门禁，不能以跳过检查解决调试入口故障。

## 发布后验证

核对 Release 为目标标签、资产齐全、公开下载可用；下载 CLI ZIP 到独立目录核验校验和与 `version --json`。检查该标签的工作流结果和来源修订，记录发布链接及证据。
