# 贡献指南

面向 EazyQQ 中国开发者和参与维护的 AI。当前开发基线为 0.5.0。

## 开始前

阅读 [代理指引](AGENTS.md)、[架构](docs/ARCHITECTURE.md)、[产品](docs/PRODUCT.md)、[接口](docs/API_SPECIFICATION.md) 和最新开发记录。先检查 `git status`，保留其他开发者已有修改，不对整仓库执行重置或清理。

Windows 开发需要 Node.js、pnpm、Rust、MSVC 和 QQNT。协议资源来自 `napcat/` 或已准备的资源目录。项目文档用中文；代码与注释用英文。提交信息采用 Conventional Commits。

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
```

## 目录与依赖

业务逻辑放入对应领域服务，桌面/CLI 只负责适配。新增实现先选择 auth/runtime/transport/messaging 等职责目录，避免扩大平铺目录。账号身份层不负责杀进程，协议运行时不负责写聊天规则。

状态观察不可隐式变更登录；数据库与任务句柄不得随全局账号变化重新绑定。跨账号资源边界和进程所有权是必须保持的行为。

## 接口变更

同时更新 Rust 适配、前端类型和 CLI 对应能力。使用 `pnpm contracts:write` 再执行 `pnpm contracts:check`；不要手改生成的覆盖矩阵或假造 CLI 支持。添加行为回归，更新中文规范及开发记录，说明兼容性和迁移。

## 验收

```powershell
pnpm typecheck
pnpm test
pnpm test:rust
pnpm contracts:check
pnpm build
pwsh -NoProfile -File scripts/cargo.ps1 build --bin eazyqq_cli
pnpm test:smoke
pnpm test:native
```

CLI/原生验收使用独立根目录和模拟协议。真实 QQ 验收需要保持用户现有会话，不启用真实自动回复或发送测试消息，除非用户明确要求。

`pnpm test:native` 默认使用 release GUI 和 CLI；可用 `EAZYQQ_TEST_GUI`、`EAZYQQ_TEST_CLI` 指向匹配的构建产物。它测试真实 WebView、身份往返切换、缓存隔离和窗口还原，不要求手机登录。

## 提交与发布

提交前运行 `git diff --check` 并复核敏感配置没有进入 Git 或资源包。更新开发记录中的起始状态、原因、决策、证据、未完成项。发布流程见 [RELEASING.md](docs/release/RELEASING.md)。
