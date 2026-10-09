# 接口规范总览

适用于 EazyQQ 0.5.2。接口入口分为桌面 IPC、CLI 和 stdio MCP；账号、登录、协议运行时、模型和存储逻辑由共享服务提供。

## 契约入口

| 文档或源码 | 用途 |
| --- | --- |
| [桌面 IPC](api/IPC.md) | 参数、返回封装、账号与健康语义 |
| [CLI](api/CLI.md) | 命令、输出、退出码、常驻任务和环境 |
| [覆盖矩阵](api/COVERAGE.md) | 已注册桌面操作的 CLI 对应关系，自动生成 |
| [CLI schema](api/cli.schema.json) | 精确命令名与参数类型，自动生成 |
| `src/api/contracts.ts`、`src/api/client.ts` | 前端类型及实际 invoke 参数 |
| `src-tauri/src/commands/` | Rust 参数、返回类型和适配层 |

运行 `eazyqq_cli schema --json` 获取当前构建的命令契约；MCP 的真实工具定义通过 `tools/list` 获取。文档不是复制出来的第二套餐单，名称和字段不得脱离实现独立修改。

## 通用规则

公共参数及新 DTO 使用 `camelCase`；磁盘登记文件保留历史 `snake_case`。可选字段可序列化为 `null`。调用方同时处理 Promise 拒绝与 `success:false`；生命周期操作另有 `attempted` 和 `ok`。

窗口控制返回原始布尔值或空值，不使用 `ApiResponse`；其行为设置查询使用统一封装。该差异已在前端类型中明确，详见 IPC 文档。

身份确认优先使用成功 OneBot 返回，必须是有效非零账号，并匹配该客户端的预期身份。只有目标身份确认后才提交选择。`quick_login` 的 1005 表示需要目标二维码；1002 表示其他登录失败。失败保持当前上下文，不统一重启 QQ。

CLI 的 `--json` 输出不混入日志；流式输出为逐行 JSON。MCP stdout 只包含 JSON-RPC，通知不返回响应。版本信息来自构建/运行时 metadata，版本升级按 SemVer 排序。

## 接口演进

新增或修改操作时：更新共享服务与适配层，校对前端类型，补充行为测试，运行 `pnpm contracts:write` 和 `pnpm contracts:check`，更新对应中文文档及开发记录。兼容别名必须有实际路由，不能只在文档中声称支持。

涉及账号、磁盘格式、错误语义和输出结构的变更，记录迁移及兼容边界。新功能的参数和结果应包含足够的身份信息，不允许依赖一个会在运行中改变的全局账号写入路径。
