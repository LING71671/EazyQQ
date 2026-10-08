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
