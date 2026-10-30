# 独立任务、表达偏好与多气泡回复

本文描述已公开发布的 0.7.0；旧版 0.6.6 不包含这些接口。公开产物核验见[发布记录](../development/2026-10-30-release-0.7.0.md)。

## 固定账号与任务

桌面通过 `manage_tasks({request})` 调用，CLI 通过 `eazyqq_cli --account QQ tasks` 调用，MCP 工具为 `manage_tasks`，参数同样包在 `request` 中。桌面返回 `ApiResponse`，其中数据为 `{tasks,task,deletedId?,draftId?,cleanupError?}`；CLI 直接输出该数据对象，MCP 将同一数据编码在标准工具结果的 JSON 文本中。

| `operation` | 请求字段 | 结果与副作用 |
| --- | --- | --- |
| `list` / `get` | `get` 需要 `taskId` | 读取当前账号的任务；过期运行租约恢复为已中断，不重启 QQ |
| `create` | `targetId,targetType,title?,instruction,persona?,allowTodos?,selectedFileIds?` | 创建本地任务，不请求模型、不发送 QQ；目标固定为 `private` 或 `group` |
| `run` | `taskId,instruction?,expectedRevision?` | 执行或继续本任务，保存独立历史和运行记录；耗时期间可从其他调用取消 |
| `cancel` / `delete` | `taskId` | 取消只影响当前任务；执行中不可删除；删除仅清理此任务自有目录 |
| `draft` | `taskId,expectedRevision?` | 将已完成任务的候选转入草稿，同一次运行重复导出返回同一份；不直接发送 |

Task 包含 `id,accountUin,targetId,targetType,title,instruction,persona,allowTodos,selectedFileIds,revision,state,session,messages,steps,todos,result,candidateReply,error,activeRun,createdAt,updatedAt`。`state` 为 `ready,running,completed,failed,cancelled,interrupted`。TaskSession 保存 `id,generation,provider,model`；认证指纹只在账号数据库内使用，接口不返回 Key。TaskRun 保存运行编号、顺序、输入版本、表达偏好版本、来源快照、实际调用数和预算。

同账号、同目标的不同任务仍有独立会话；换供应方、端点、模型或认证后，新运行轮换会话代次。运行期间发生新消息、人设变化、认证变化、取消或版本更新时，旧结果不能覆盖新状态。SQLite 事务与租约拒绝 GUI/CLI 并发认领同一任务。进程硬退出后，超过运行时限的租约显示为已中断；不会猜测完成或自动重做外部动作。

首版工具仅有：当前会话已验证缓存消息检索、当前群缓存目录、明确选定且已经下载文件的正文提取，以及明确允许的任务本地待办。文件需位于固定账号的当前群目录，拒绝链接与路径逃逸；最多选择 5 个文件，每个读取不超过 8 MiB、正文返回不超过 20000 字符。未下载、类型不支持或读取失败均明确返回错误；不会扩大到任意 URL、目录、联系人或群。

每次运行最多 6 次模型调用、90 秒；每账号最多保留 100 个任务，每任务记录最多 512 KiB，历史和步骤也有数量上限。任务会话目录为 `ai-runtime/tasks/<taskId>/sessions/<sessionId>`，模型或认证变化后使用新的会话目录；普通回复、简报、文档等临时推理使用独立 `ai-runtime/requests/<requestId>`，完成后清理自有请求目录。单次可接受缓存不超过 128 MiB；临时目录最多保留 32 个，异常遗留达到上限时明确报错，不删除其他运行中的目录。

```powershell
eazyqq_cli --account 10001 tasks --action create --type group --target 20001 --title "整理安排" --instruction "根据本群缓存整理待办，先检索来源" --allow-todos --json
eazyqq_cli --account 10001 tasks --action run --id task_ID --revision 1 --json
eazyqq_cli --account 10001 tasks --action draft --id task_ID --json
eazyqq_cli --account 10001 tasks --action cancel --id task_ID --json
```

CLI 的 `--selected-file-ids` 接受文件 ID 的 JSON 数组。任务不会自行下载文件；可先在群文件页明确下载，再选择任务允许读取的文件。

## 表达偏好继承

`manage_personas({request})` 的操作为 `get,set,clear`，字段为 `targetType?,targetId?,memberUin?,text?`。不指定目标即为当前账号；指定目标为联系人或群；`memberUin` 仅允许与具体群一起使用。群成员按号码绑定，不按显示名合并。

返回 `{text,inheritedText,revision,overrides}`。`text` 是当前层明确保存的覆盖，`inheritedText` 是上层表达偏好；清除后恢复继承。顺序为账号、会话、群成员、任务，具体层在后。表达偏好只改变说话方式，不授予工具、文件、QQ 发送权限，也不提供真实人物经历。每项最多 4096 字、每账号最多 256 项。

```powershell
eazyqq_cli --account 10001 personas --action set --text "简短直接，先回应对方的问题" --json
eazyqq_cli --account 10001 personas --action set --type group --target 20001 --member 30001 --text "熟人语气，少用解释句" --json
eazyqq_cli --account 10001 personas --action clear --type group --target 20001 --member 30001 --json
```

## 回复计划与逐条回执

`PendingDraftDto` 新增可选 `replyPlan`。旧草稿正文按一个气泡兼容，不根据换行、标点或段落自动拆消息。计划只包含内容，账号、目标和授权由应用绑定：

```json
{"schemaVersion":1,"bubbles":[{"id":"b1","segments":[{"type":"text","text":"我看到了"}]},{"id":"b2","segments":[{"type":"face","faceKey":"427"}]}]}
```

模型候选中的气泡编号由程序重建；只允许 `text` 与目录内基础 `face`。纯文字、纯表情、同气泡混排与多个气泡均支持，独立表情不强加文字。首版最多 6 个气泡，每气泡最多 32 段、2000 字，总计划最多 32 KiB；超限、未知表情、Unicode emoji 或越权字段明确拒绝，不静默截断。

`send_draft({draftId,finalContent?,finalBubbles?})` 返回 `ApiResponse<DeliveryReport>`。`finalBubbles` 是显式气泡文本数组，允许 CQ 原生 face 码；未传时采用保存的计划。仅修改 `finalContent` 时兼容为一条文字消息。CLI 使用 `draft-send --id ID --bubbles-json '["第一条","第二条"]'`；`ask --json` 新增 `replyPlan`，原有 `reply` 预览字段保留。桌面聊天窗口的构思入口保留文本预览兼容，不直接发送。

DeliveryReport 为 `{batchId,status,receipts,messages}`；每条回执为 `{bubbleId,status,messageId,error}`。气泡状态为 `pending,sending,sent,failed,unknown,cancelled,superseded`；批次还可显示 `partial`。每气泡对应一次协议写入，混排段不会被拆散。会话租约避免批次交错，账号级节流按 250 毫秒间隔预留发送时隙；每次写入前检查账号、规则、输入快照、表达偏好、取消及修订状态。

账号数据库最多保留 500 条完整批次报告，写入时优先保留最近 200 条终态报告。归档仅处理无活跃租约且所有气泡均为已发送、已取消或已过期的记录，并保留不含正文的不可变编号与散列凭据，阻止旧编号重复发送；未知送达、明确失败与未完成记录不会自动清除。归档凭据最多 50000 条，容量不足时拒绝新批次，已有未完成批次仍可核对和恢复；读取列表不执行保留清理。

先持久化计划，再写入 `sending`，仅有有效平台消息编号才记 `sent`。部分成功保留草稿；重试跳过已发送前缀。明确拒绝可重试失败项；超时、连接断开、缺少编号和发送中断记 `unknown`。存在未知项时，此会话的新批次也被拦截，避免用新编号隐藏不确定结果。新消息或撤权使剩余气泡标为 `superseded`，不会重新授权旧计划。

`manage_reply_batches({request})` 支持 `list,get,reconcile`。`get` 返回报告，`list` 返回含固定作用域、原计划、输入快照与报告的批次记录，未知项优先；`reconcile` 需要 `batchId,bubbleId`，并提供核对到的 `deliveredMessageId` 或明确的 `confirmedNotDelivered:true`。已送达项不能通过核对入口重置。人工确认只表示用户核对结果，程序不冒充已自动查证 QQ 客户端。

```powershell
eazyqq_cli --account 10001 reply-batches --json
eazyqq_cli --account 10001 reply-batches --action get --id DRAFT_ID --json
eazyqq_cli --account 10001 reply-batches --action reconcile --id DRAFT_ID --bubble b2 --not-delivered --json
```

MCP 对应 `manage_tasks,manage_personas,manage_reply_batches`，均使用 `request`；`approve_draft({draftId,finalBubbles?})` 仅用于用户明确批准的具体草稿。智能体内部工具白名单不包含这些发送或设置工具，材料和模型不能自行扩权。

`get_group_files` 新增 `cachedOnly?:boolean`；为 `true` 时仅读数据库，普通群文件页仍按既有行为同步目录。CLI 对应 `files --target GROUP --cached-only`。
