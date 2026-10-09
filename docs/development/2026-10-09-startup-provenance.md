# QQ 加载入口故障溯源与诊断链路

## 本轮范围

用户要求先定位问题并做好溯源及诊断链路。停止重复启动，保留外部 QQ；不发布产品版本，不实施登录态或内存策略。源码新增诊断能力尚未安装到用户桌面。

## 已确认事实

| 层次 | 证据 | 结论 |
| --- | --- | --- |
| 安装与私有载荷 | 三个不可变资源逐文件 SHA256 一致 | 排除私有复制缺失或内容不一致 |
| 私有加载器 | loadNapCat.cjs 存在，大小 118 字节 | 文件存在不能代表 QQ 已执行 |
| 生成入口 | package.main 为 116 字节相对路径，无字面省略号 | 截图省略号不能据此判定路径被写坏 |
| 加载桥 | C 盘用户目录 junction 指向 A 盘私有目录，规范路径相同 | 普通文件系统层能通过桥读取 |
| 普通 Node | Node 22.22.1 require.resolve 与 realpath 成功 | 不能代替 QQ 内部运行时解析 |
| Windows API | GetFileInformationByName 的 0/3 类，普通与长路径形式均成功 | 未观察到普通文件 API 的失败 |
| QQ 实际启动 | 用户截图为 Cannot find module；引导日志仅到 Process resumed | 真实模块加载失败，启动接受不是就绪 |
| 服务 | 3000、3001、6099 未监听 | 协议链路没有完成初始化 |

A 盘为健康的固定 NTFS 卷，没有证据支持“映射盘失效”。当前没有完整 QQ 内部文件访问结果或进程缓解策略快照，不能将权限、RedirectionGuard、QQ 自身限制或注入钩子行为写成确定根因。

本机异常后关闭当前 EazyQQ 以阻止 GUI 自动拉起；再调用绑定目录的 CLI stop，由受验证 Job 控制失败树。只读检查确认原 QQ PID 7464、启动时间 07:59:07.64301 及 A:/NTQQ/QQ.exe 路径保持不变。本轮未再次启动本机 QQ 做实验。

## 诊断覆盖缺口

旧 napcat-doctor 硬编码根目录/napcat，可能错过真实私有目录；已改为复用 source 定位和 selected 布局。历史诊断只检查资源存在、全机 QQ 数量和引导日志写入，不证明 QQ 执行入口。旧假引导器回归只能证明 Job、文件与登记迁移，不包含 QQ 模块解析。

先前真实 QQ 云端验收使用用户目录 QQ；本机 QQ 在 Program Files。两者入口的相对路径与路径上下文不同，不能直接迁移结论。本轮云端对照仍待结果：相同不可变安装夹具分别放在用户目录与 Program Files，保存真实弹窗文字、加载器凭证、端口和暂停调试器结果；不扫码，不发送消息。作业 37905077835 为证据采集作业，不是发布验收。

## 新证据链

health/startup_trace.rs 生成结构化快照：private_payload、entry_resolution、private_loader、launch_request、loader_execution、qq_module_load、owned_process、webui_transport、authenticated_session。状态为 passed/failed/unknown，分别记录首个失败与首个未确认。快照中的 ready 不能因文件或 TCP 探测成功而设为真。

每次实际启动写 eazyqq-startup-attempt.json，生成加载器包含该 attemptId；加载器执行写 eazyqq-loader-witness.json，区分 loader_entered、module_imported、module_failed，仅记录 PID、版本、时间和错误类型/代码，不记录凭据、消息或整个环境。写凭证失败不阻断协议代码。监督进程接受请求另写 eazyqq-startup-launch.json；陈旧凭证不得用于新尝试。旧版本只有 launch plan/result 时使用 legacy 请求编号并明确 attemptOrigin；缺少新加载器凭证保持 unknown。

napcat-doctor --json 增加 startupTrace；诊断 ZIP 增加 protocol/startup-trace.json。捕获操作只读，不生成新的启动尝试、快速登录或重启，也不主动进行 AI 推理。身份证据仍由账号只读探测提供，不能从窗口、Job 或端口推断登录成功。

## 验证及未完成项

已新增真实 Node 导入成功/失败凭证、陈旧凭证拒绝、仅 TCP 不能证明认证就绪、凭证不包含错误文本中的秘密、旧启动收据只读关联等回归。Rust、CLI 及生成契约的最终结果在执行后补充。

当前根因仅收窄到 QQ 内部加载入口层；最终致因仍待云端对照和真实运行时证据。不会用诊断收集作业的成功代替产品登录成功，也不会因一个猜测继续发版本。

## 首轮云端取证准备失败

作业 37905077835 未执行 QQ：Prepare immutable fixtures 返回 release not found。临时夹具确实存在且为未公开草稿，但工作流只授予 contents:read。GitHub 官方文档明确只有 push 权限可列出草稿，历史可用真实 QQ 取证工作流也使用 contents:write。本轮将单一取证作业的临时 GITHUB_TOKEN 改为 contents:write 以读取草稿；工作流没有发布步骤。该失败不能写成 QQ 加载失败，也不能写成取证通过。

参考：https://docs.github.com/en/rest/releases/releases#list-releases 。临时夹具只含不可变 QQ 安装代码，上传至未公开草稿，取证结束后删除附件，不进入任何公开产品版本。
