# 0.6.0 免凭据模型检测与交付

## 用户定义与起始状态

用户明确要求免费指只下载安装 OpenCode、不配置账号、API Key 或付费即可使用，并指出模型会持续变更或下架。保留当前模型选择，不静默改为 Big Pickle；不实施 QQ 登录态/内存新策略。

0.5.5 已恢复本机真实 QQ 模块、WebUI 与 OneBot 登录证据；0.5.6 的目录别名诊断修正已在源码验证，但候选发布作业 38012888009 在原生切换检测发现 Windows 文件占用，未建立公开 Release。记录保留，相关修正进入 0.6.0，不覆盖公开或失败候选标签。

## 检测链路

领域服务位于 `services/ai/catalog/free_models.rs`。读取原生 `models opencode --refresh --verbose`，验证供应方和模型标识，显式输入/输出零定价只作为候选，不能从名称 `-free` 推断。报告记录原生版本、目录来源、观察时间、当前目录 IDs 和逐模型状态。

显式验证在账号固定 AI 目录中新建独立检测环境，使用 XDG data/state/config/cache、OPENCODE_TEST_HOME 与 OPENCODE_CONFIG_DIR 隔离；移除用户 Key 环境变量，阻止项目配置继承，不复制 auth 文件。保留原生运行时的匿名默认机制，不手写客户端请求头或伪造身份。工具仍需审批、外部目录与提问工具禁止，不加 --auto。

初版检测配置错误地强制空 Key，覆盖 SDK 的匿名默认，导致全部未确认；该轮不可计为免费通过。查阅 OpenCode v1.18.30 官方 provider/config 源码，移除该覆盖并隔离兼容用户配置后，完整实测 11 候选，4 个本次返回模型文本、7 个未确认。证据 `.test-runtime/free-models-verified-live.jsonl` 为逐行 progress 和 complete；不把超时或未知错误标成下架。

`available` 只表示本次无用户凭据请求成功；`retired` 来自明确停用元数据或明确错误；`removed_from_catalogue` 由前次记录与本次原生目录比较；`requires_conditions` 表示额外条件；`unconfirmed` 保留限流、网络、超时及不明错误。无普遍永久可用承诺。

## 桌面与 CLI

设置页沿用现有 Slate/Sky 风格，元数据读取不发起推理，用户点击“自动检测免费模型”才验证；进度包含完成数和当前模型。只有验证通过才可选用，旧模型不在新目录时提示并保留配置。事件按随机请求编号过滤迟到响应，未挂载组件不接收结果。

新增 IPC `detect_free_models({probe?,requestId?})` 和事件 `free-model-progress`，CLI 为 `ai-free-models [--probe] [--stream] --json`。不传 --probe 只读取目录；stream 是单行 JSON 进度与 complete。54 个桌面命令和 59 个 CLI 命令的生成覆盖已更新。

## CI 保存边界修复

候选 0.5.6 的失败日志为“保存账号选择 Access is denied”。原生测试的短暂 Python 文件读取未共享删除权限，Windows 原子替换会失败。JSON 写入在保留原文件的前提下，对 5/32/33 短暂错误进行最长 750ms 的有界重试；持续失败仍报原始错误。新增持有读句柄后释放的真实 Windows 回归。

`reconcile_account` 返回 Result，保存失败时 quick_login 返回准确领域错误，不把验证了身份等同于选择保存成功。旧进程身份仍保持至重建，迟到状态响应不改变选择。

## 验收状态

已经通过 28 项前端行为测试；原子读占用、模型元数据标识、零定价候选、条件/暂时失败分层及匿名环境回归已执行。最终版本完整 Rust、CLI、真实桌面渲染、完整发布、下载校验与本机更新证据仍需追加，不能提前宣称完成。

本机原有 QQ 与受管理健康协议不停止，不发送 QQ 消息，不启用自动回复。构建缓存仍使用 A 盘；此前被自动审批拒绝的临时 QQ 副本递归清理不再尝试。

## 最终本地候选

最终完整 Rust 为 126 项库测试和 2 项 CLI 测试，共 128 项通过；28 项前端测试、类型与54/59接口覆盖检查通过。真实 Tauri 使用模拟协议和原生 OpenCode 夹具验证检测通过/明确停用、配置不自动改变、账号往返、延迟旧响应、浏览器缓存、窗口与快捷键；未调用真实 QQ。截图已查看，修正了普通目录仍可点击停用模型的边界，确认截图为 `.test-runtime/native-profiles-nnt8y9la/free-model-discovery.png`，不继续进行开放式视觉循环。

CLI 26 项冒烟、版本校验和 698 文件资源检查通过。最终候选安装包 26766618 字节，六项分发文件已生成；候选在提交来源确定前生成，公开资产必须以完整 CI 再生成的 manifest 与 SHA256 为准。日志为 `.test-runtime/release-0.6.0-final-rust.log`、`release-0.6.0-final-frontend.log`、`release-0.6.0-confirm-native.log`、`release-0.6.0-final-smoke.log` 和 `release-0.6.0-final-package.log`。

本机仍为 0.5.5，发布门禁、公开下载验证和升级仍待完成。已经测试的行为不能替代尚未执行的发布与安装结果。
