# 桌面 IPC 契约

适用于 0.5.3，传输方式为 Tauri `invoke(command, args)`。公共参数和 DTO 使用 `camelCase`，私有登记文件使用 `snake_case`。

## 返回与错误

大多数业务接口返回 `ApiResponse<T>`：

```json
{"success":true,"data":{},"error":null,"timestamp":1791452991000}
```

`timestamp` 为 Unix 毫秒时间。Promise 拒绝表示命令/运行时错误；`success:false` 表示已处理的领域错误。调用方必须检查两者。可选字段可能是 `null`。

```json
{"success":false,"data":null,"error":{"code":1002,"message":"目标账号身份未确认","suggestedAction":"打开该账号的二维码"},"timestamp":1791452991000}
```

`attempted:false` 表示未执行进程操作；`ok:false` 表示操作未成功；外层成功只代表请求得到结果。`quick_login` 中 code 1005 专门用于需要二维码认证，其他命令的错误码须结合命令上下文解释。

## 账号操作

| 命令 | 参数 | data |
| --- | --- | --- |
| `get_account_status` | `{uin}` | `AccountReport`，只读目标探测 |
| `list_accounts` | `{}` | `AccountReport[]` |
| `register_account` | `{uin, nickname?}` | `AccountInfoDto` |
| `quick_login` | `{uin}` | 空；身份确认后提交桌面选择 |
| `account_qrcode` | `{uin, refresh?}` | `{uin, qrcodeBase64?, loggedIn?}` |
| `batch_accounts` | `{operation, uins}` | `BatchOutcome[]` |
| `configure_account` | `{uin, autoStart}` | 空 |
| `forget_account` | `{uin}` | 空；保留账号数据 |

QQ 号为 5–20 位十进制数字，不以零开头。批量操作接受 1–100 个账号并去重，支持 `start`、`stop`、`login`，逐账号返回结果。登记不启动 QQ；停止关闭该账号的自启。当前账号及仍有受管理进程的账号不能忘记。

```json
{
  "instance":{"uin":"10001","nickname":"示例","httpPort":3000,"wsPort":3001,"webuiPort":6099,"autoStart":true,"processManaged":true},
  "login":{"loggedIn":true,"uin":"10001","nickname":"示例","source":"onebot"},
  "selected":true
}
```

`processManaged` 表示存在受验证的进程所有权，不代表已登录。认证来源为 `onebot`、`webui`、`identity_mismatch` 或无证据时的空字符串。探测超时不是退出登录的证据。账号列表最多并发探测八个账号，每个探测限制三秒。

桌面切换只重建 EazyQQ 上下文，协议会话保持。数据库、AI 和执行中的任务仍绑定旧账号直至进程退出。目标未确认则保留当前选择。状态轮询不触发快速登录、停止进程或刷新二维码。

## 登录、健康与恢复

`get_protocol_status` 包含 `isConnected`、`loginStatus`、`qqNumber`、`nickname`、`avatarUrl`、`qrcodeBase64`、`qrcodeError` 与记忆账号列表。有效状态包括 `unlogged`、`waiting_scan`、`scanned`、`logged_in`。OneBot 必须返回 `status=ok`、`retcode=0` 和有效身份；已确认身份优先于陈旧 WebUI 标志。

协议与登录探测并行，各限制三秒；记忆账号与二维码请求并行，各限制五秒。离线时 `qrcodeError` 返回缺失资源或服务不可用原因，不继续查询记忆账号。客户端应显示领域错误、Promise 拒绝与等待超时，不能因没有二维码就无限展示加载。

`restart_napcat` 是显式恢复操作。旧安装根 `resources/napcat` 会先准备并验证私有替换目录，再只停止原目录受验证的 Windows Job；外部监听会话拒绝重启。迁移成功后桌面自动重建上下文以读取新目录的凭据。`repair_chain` 仅在旧目录缺失必需资源且属于受管理故障时使用此路径，普通等待扫码、健康和外部会话保留。

`refresh_qrcode` 调用实际 `RefreshQRcode` API，失败不会返回旧磁盘图片冒充新码。`logout`、`restart_napcat` 拒绝结束不属于 EazyQQ 的运行会话。

`get_chain_status` 返回八个环节、`firstBreak`、`hasFailure`、`uptimeSecs`。状态为 `ok`、`unknown`、`failed`；链路时间字段保持现有 `last_ok_secs_ago`、`last_error_secs_ago`。等待扫码、未测试推理、只有 TCP 端口开放均不能代表完整正常。

`repair_chain` 返回 `{action, attempted, ok, detail}`。缺失服务可以启动；认证成功、已运行、外部管理、登录等待、模型或数据库故障不会触发统一重启。WebSocket 工作进程独立重连。周期监控不生成模型推理请求。

## 其他业务接口

| 领域 | 主要参数与返回 |
| --- | --- |
| 联系人 | `get_contacts({params?})` 返回 `{list,total}`；`params` 可含 `type`、`searchKeyword` |
| 规则 | `update_rule({rule})` 返回规则；`batch_update_mode({targetIds,mode})` 返回 `{affectedCount}`；`mark_read({targetId,at?})` 返回已读时间 |
| 消息 | `get_messages({targetId,limit,offset,targetType?})` 返回数组；`send_message({targetId,content})` 返回 `{messageId}`；`trigger_ai_reply({targetId,contextSnippet})` 返回候选文本 |
| 草稿 | `get_pending_drafts()` 返回数组；`send_draft({draftId,finalContent?})` 返回 `{sentMessageId}`；`dismiss_draft({draftId})` 返回空；`regenerate_draft({draftId,customInstruction?})` 返回新草稿 |
| 文件 | `get_group_files({groupId,folderId?})` 返回数组；`download_file({groupId,fileId,fileName})` 返回 `{taskId,localSavePath}`；`summarize_file({localFilePath})` 返回综述；`open_folder({targetPath})` 返回空 |
| 简报 | `generate_summary` / `generate_summary_stream` 接受 `{targetId,slidingWindowHours}`；`get_summary_history({targetId?})` 返回数组；`delete_summary({id})` 返回空 |
| 配置 | `get_config()`、`update_config({config})` 返回配置；修改接口接收完整 JSON 配置，不是服务端深度补丁合并 |
| 模型 | `fetch_provider_models({provider,baseUrl?,apiKey?})` 返回目录；`test_ai_connection({provider?,modelId?,baseUrl?,apiKey?,prompt?})` 返回测试结果 |
| 诊断 | `check_dependencies()` 返回依赖报告；`export_diagnostics_bundle()` 返回 `{zipFilePath}` |
| 更新 | `check_app_update()`、`check_napcat_update()` 返回更新信息；`get_napcat_version()` 返回版本；`upgrade_app` / `upgrade_napcat` 接受 `{downloadUrl?}` |

当前开发源码的诊断 ZIP 新增 `protocol/startup-trace.json`，包含实际私有目录、入口解析、资源散列、本次启动编号及加载器执行凭证。阶段状态 `passed`、`failed`、`unknown` 分别表示该检查成立、该检查失败或缺乏证据；端口开放不代表认证成立。凭证只记录阶段、PID、运行时版本、时间、错误类型/代码，不记录 token、消息或完整环境。新能力尚未随产品发布。
| QQ 路径 | `get_qq_path()` 返回路径；`set_qq_path({path})` 验证文件并更新引导 |

完整命令清单由 [覆盖矩阵](COVERAGE.md) 生成。所有 DTO 的精确字段以 `src/api/contracts.ts` 和 Rust 命令返回类型为准。

## 窗口返回例外

`app_minimize_window()` 返回布尔值，表示是否进入托盘；`app_toggle_maximize_window()` 返回最大化后的状态；`app_close_window()` 返回是否隐藏到托盘，退出时进程会结束。`app_start_drag_window()`、`app_show_window()` 返回空值，均不包 `ApiResponse`。`app_get_window_behavior()` 使用统一封装，data 为 `{closeToTray,minimizeToTray}`。

## 事件与维护

事件包括 `new-draft`、`new-chat-message`、`summary-chunk`、`summary-end`；对应 payload 在前端类型文件中定义。当前可见账号之外的后台工作进程不将账号数据推送给当前界面。

配置保存在账号 SQLite 中；长期工作进程在处理消息/定时任务前刷新 AI 配置。应用版本来自构建清单，协议版本来自实时接口或安装 metadata，不可用时为 `unknown`。协议升级要求已知会话停机，归档暂存检查路径及符号链接，并保留个人配置和回滚备份。

## 0.5.3 更新状态与事件

主程序信息新增 status、installerName、downloadSize、checksumSha256。状态为 available、up_to_date、newer_local、no_release、installer_missing；核心状态为 available、up_to_date、newer_local、version_unknown、asset_missing。可选值可能为 null。

app-update-progress、napcat-update-progress 的 payload 为 {phase,downloadedBytes,totalBytes}，阶段包含 checking、downloading、verifying、installing、ready。调用前订阅，成功、失败和卸载后释放。ready 只表示包就绪，不表示安装结束。

主程序升级只接受当前稳定版本的明确安装包及官方 SHA256；显式 downloadUrl 不能绕过选择。桌面退出后助手安装并重开，CLI 不退出另一个桌面。核心只接受官方 Shell 包，存活会话阻止覆盖，配置与备份保留；成功后按需启动账号。
