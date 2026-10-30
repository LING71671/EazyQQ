# 桌面 IPC 契约

0.6.6 已发布；本文与本版本程序一致，不能据此宣称旧安装包已包含新功能。传输方式为 Tauri `invoke(command, args)`。公共参数和 DTO 使用 `camelCase`。

## 风格包与消息来源

`manage_style_packs` 接收 `{operation, packJson?, packId?, destination?}`。操作为 `list/import/select/enable/disable/remove/export`；返回 `{enabled, activeId, packs, exportedPath?}`。包格式、容量、当前账号绑定与隐私边界见 [风格包说明](../ai/STYLE_PACKS.md)。导出只写表达包本体，不输出账号安装索引；路径必须为不存在的 JSON 文件。

`trigger_ai_reply` 新增可选 `sourceMessageId/targetType`。桌面从最后一条实际来信提交来源编号，服务按编号读取并核验固定账号与会话，不能按相同文本或昵称猜作者；旧调用仅作为号主提供的上下文片段，原作者标记为未知。自动回复、草稿和重生成复用带来源的上下文链路。本轮来信按消息 ID 从历史中排除一次。

## 0.6.5 已发布范围

更新回执闭环、免费检测生命周期及 GUI/CLI/MCP 诊断语义统一已随 0.6.5 发布并保留在 0.6.6；具体实现和验收状态见 [本轮记录](../development/2026-10-28-reliability-and-lifecycle.md)。旧安装包不含 0.6.6 增量。

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

已绑定上下文的身份观察不提交账号选择。显式切换只保存启动目标，旧进程的身份和路径在退出前保持；首次未绑定身份确认采用原子检查，不能覆盖另一操作已经保存的目标。延迟状态响应不得导致切换回旧账号。

## 登录、健康与恢复

`get_protocol_status` 包含 `isConnected`、`loginStatus`、`qqNumber`、`nickname`、`avatarUrl`、`qrcodeBase64`、`qrcodeError` 与记忆账号列表。有效状态包括 `unlogged`、`waiting_scan`、`scanned`、`logged_in`。OneBot 必须返回 `status=ok`、`retcode=0` 和有效身份；已确认身份优先于陈旧 WebUI 标志。

协议与登录探测并行，各限制三秒；记忆账号与二维码请求并行，各限制五秒。离线时 `qrcodeError` 返回缺失资源或服务不可用原因，不继续查询记忆账号。客户端应显示领域错误、Promise 拒绝与等待超时，不能因没有二维码就无限展示加载。

`restart_napcat` 是显式恢复操作。旧安装根 `resources/napcat` 会先准备并验证私有替换目录，再只停止原目录受验证的 Windows Job；外部监听会话拒绝重启。迁移成功后桌面自动重建上下文以读取新目录的凭据。`repair_chain` 仅在旧目录缺失必需资源且属于受管理故障时使用此路径，普通等待扫码、健康和外部会话保留。

`refresh_qrcode` 调用实际 `RefreshQRcode` API，失败不会返回旧磁盘图片冒充新码。`logout`、`restart_napcat` 拒绝结束不属于 EazyQQ 的运行会话。

`get_chain_status` 返回八个环节、`firstBreak`、`hasFailure`、`uptimeSecs`。状态为 `ok`、`unknown`、`failed`；链路时间字段保持现有 `last_ok_secs_ago`、`last_error_secs_ago`。等待扫码、未测试推理、只有 TCP 端口开放均不能代表完整正常。

`repair_chain` 返回 `{action, attempted, ok, detail}`。缺失服务可以启动；认证成功、已运行、外部管理、登录等待、模型或数据库故障不会触发统一重启。WebSocket 工作进程独立重连。周期监控不生成模型推理请求。

## 其他业务接口

### OpenCode 免凭据检测

`detect_free_models({probe?,requestId?})` 返回统一 `ApiResponse<FreeModelsReport>`。`probe=false` 只刷新原生目录，不发起推理；`probe=true` 对显式零定价候选逐一发送隔离的中性请求。`requestId` 为空或未提供时由后端生成；非空时为 1–128 个 ASCII 字母、数字、连字符或下划线，用于过滤进度及定位本账号任务，不作为权限凭据。

报告字段为 `runtimeVersion`、`observedAtMs`、`catalogueIds`、`models`、`credentialsUsed` 和 `source`。每模型包含 `id`、`name`、`state`、`detail`、`observedAtMs`；状态为 `candidate`、`available`、`requires_conditions`、`retired`、`removed_from_catalogue`、`unconfirmed`。`available` 仅证明本次未借用用户账号/API Key 的请求成功，价格为零或名称含 free 不足以成立。目录移除只描述本次原生目录，不能从网络错误推出供应方永久下架。

0.6.5新增 `requestId`、`status`、`completed`、`total`、`catalogueRefreshed`、`error`、`cleanupError` 和 `history`。任务状态为 `completed`、`cancelled`、`failed`；部分完成结果保留在本次 `models`，历史报告位于 `history`，每项包含 `{requestId,status,observedAtMs,models}`，不能当作本次可用证据。目录刷新失败和临时目录清理失败分别呈现，不伪装为完整检测成功。

`cancel_free_model_detection({requestId})` 返回 `ApiResponse<boolean>`，仅定位固定账号运行目录下的同编号任务。`true` 表示找到正在执行的任务并请求取消；`false` 也可能表示取消先于检测注册而暂存，不能解释为任务已完成或子进程已退出。最终状态以原检测返回的报告为准。取消、控件卸载及窗口退出纳入本次子进程回收；只清理有本次所有权标记的检测目录，不删除账号凭据、用户缓存或其他 OpenCode 会话。关窗回收的统一验收仍在进行。

`free-model-progress` 事件字段为 `{requestId,completed,total,model}`。调用前订阅，结束后释放，丢弃不匹配编号；检测不自动保存模型选择、不改变 QQ 会话或规则。周期健康监控不调用显式推理检测。

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
| 更新 | `check_app_update()`、`check_napcat_update()` 返回更新信息并记录检查回执；`get_napcat_version()` 返回版本；`get_update_history()` 只读回执；`upgrade_app` / `upgrade_napcat` 接受 `{downloadUrl?}` |
| QQ 路径 | `get_qq_path()` 返回路径；`set_qq_path({path})` 验证文件并更新引导 |

0.6.0 的诊断 ZIP 新增 `protocol/startup-trace.json`，包含实际私有目录、入口解析、资源散列、本次启动编号及加载器执行凭证。阶段状态 `passed`、`failed`、`unknown` 分别表示该检查成立、该检查失败或缺乏证据；端口开放不代表认证成立。凭证只记录阶段、PID、运行时版本、时间、错误类型/代码，不记录 token、消息或完整环境。旧版 0.5.3 不包含该能力。

0.6.0 的加载入口和更新安装助手使用最终物理路径跨进程交接，现有参数与 DTO 不变；真实 QQ 的执行凭证仍是加载验收依据。详见 `docs/development/2026-10-09-physical-path-handoff.md`，旧版 0.5.3 安装包不包含该修复。

完整命令清单由 [覆盖矩阵](COVERAGE.md) 生成。所有 DTO 的精确字段以 `src/api/contracts.ts` 和 Rust 命令返回类型为准。

## 窗口返回例外

`app_minimize_window()` 返回布尔值，表示是否进入托盘；`app_toggle_maximize_window()` 返回最大化后的状态；`app_close_window()` 返回是否隐藏到托盘，退出时进程会结束。`app_start_drag_window()`、`app_show_window()` 返回空值，均不包 `ApiResponse`。`app_get_window_behavior()` 使用统一封装，data 为 `{closeToTray,minimizeToTray}`。

## 事件与维护

事件包括 `new-draft`、`new-chat-message`、`summary-chunk`、`summary-end`；对应 payload 在前端类型文件中定义。当前可见账号之外的后台工作进程不将账号数据推送给当前界面。

配置保存在账号 SQLite 中；长期工作进程在处理消息/定时任务前刷新 AI 配置。应用版本来自构建清单，协议版本来自实时接口或安装 metadata，不可用时为 `unknown`。协议升级要求已知会话停机，归档暂存检查路径及符号链接，并保留个人配置和回滚备份。

## 更新状态与事件

0.6.5包含尚未公开的可选诊断接口：`get_telemetry_status()`返回`{enabled,configured,component,release}`；`test_telemetry()`发送合成诊断，返回`{eventId,flushed,release}`。状态读取不发送测试，测试由用户显式操作。flushed只证明队列处理完成，服务器接收需按eventId核对。配置与隐私范围见[接入文档](../observability/SENTRY.md)。

0.6.5的 `get_update_history({})` 返回 `ApiResponse<UpdateReceipt[]>`，与 CLI `updates history` 读取同一暂存上下文的回执，按开始时间倒序最多 50 条。历史查询不创建目录、不运行安装、不改变 QQ。主程序及核心的检查、下载、校验、安装交接和部署分别留证；一次安装流程的再次检查、下载、校验和交接沿同一操作编号关联，重试创建新编号。

回执保留 `schemaVersion=1`，字段为 `operationId`、`component=app|napcat`、`operation=check|install`、`targetVersion?`、`phase`、`startedAtMs`、`updatedAtMs`、`installerExitCode?`、`error?`、`verifiedVersion?` 和 `events`。`events` 为最多 32 项 `{phase,atMs}` 阶段记录。旧回执缺少新增字段时仍可读取，旧 `installer_succeeded` 保持“待核验版本”的含义。

阶段包括 `checking`、`checked`、`downloading`、`verifying`、`ready`、`handoff`、`waiting_parent`、`installing`、`pending_verification`、`deployed`、`succeeded`、`failed`、`aborted`，以及兼容阶段 `installer_succeeded`。安装程序退出 0 只进入 `pending_verification`；真实 GUI 从交接记录的物理安装目录以目标版本启动后，才记为 `succeeded` 并填写 `verifiedVersion`。CLI 启动不能证明同目录 GUI 的版本；缺少核验目录或启动证据时保留待确认。`deployed` 只证明核心文件部署完成，不证明协议进程已加载该版本。

只有实际观察到的取消或助手明确放弃执行才记为 `aborted`；进程崩溃留下的未结束记录保持最后阶段，界面标明结果未确认，不凭陈旧 PID 或经过时间猜测失败。写入时最多保留 100 条操作回执，只清理已终态的自有证据和对应交接/核验文件；100 条均未决时拒绝新任务，保留未知证据。用户数据、协议配置和回滚备份不随此清理。损坏或身份不符回执报错，不伪装为成功。

主程序信息新增 status、installerName、downloadSize、checksumSha256。状态为 available、up_to_date、newer_local、no_release、installer_missing；核心状态为 available、up_to_date、newer_local、version_unknown、asset_missing。可选值可能为 null。

`app-update-progress`、`napcat-update-progress` 的 payload 为 `{operationId?,phase,downloadedBytes,totalBytes}`，阶段包含 `checking`、`downloading`、`verifying`、`installing`、`ready`。调用前订阅，按编号丢弃其他任务进度，成功、失败和卸载后释放。主程序的 `ready` 只表示包就绪，不表示安装结束；核心的 `ready` 表示文件部署完成，仍不表示协议运行版本已确认。

主程序升级只接受当前稳定版本的明确安装包及官方 SHA256；显式 downloadUrl 不能绕过选择。桌面退出后助手安装并重开，CLI 不退出另一个桌面。核心只接受官方 Shell 包，存活会话阻止覆盖，配置与备份保留；成功后按需启动账号。


## 0.6.3 推理验证状态

`test_ai_connection` 的成功数据新增 `matchesActiveConfig`，表示本次测试目标是否与完成时的生效供应方、模型、端点、Key一致。显式 `apiKey:""` 保持空值，不自动借用旧 Key。测试不保存配置；未保存预览不能替代生效模型的健康状态。保存与已验证预览一致的目标后复用该证据，不额外发送测试请求。

当前进程/账号的实际推理和显式测试记录按目标隔离的结果。普通轮询只读取/发布证据，不发起模型请求；成功不会被启动检查覆盖。更换目标或Key后没有匹配证据则为 unknown，最新失败为 failed。进程重建后重新验证，不把历史记录当作新推理成功。unknown 在界面显示“待验证”，不计为异常。

OpenCode 空Key使用账号私有 no-key 目录和匿名环境；不导入本机 auth.json 或Key环境变量。填写Key只在配置区管理，诊断不切换供应方。独立免费目录检测仍按其本次结果显示资格，普通目录刷新不宣称已验证全部模型。
