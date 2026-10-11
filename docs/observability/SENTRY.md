# Sentry 接入与运维

## 项目与覆盖范围

组织为`openreverselab`，前端项目[eazyqq-ui](https://openreverselab.sentry.io/settings/projects/eazyqq-ui/)，Rust/CLI/更新助手项目[eazyqq-core](https://openreverselab.sentry.io/settings/projects/eazyqq-core/)。前端项目关联已有GitHub集成的LING71671/EazyQQ，前后端按组件标签区分。

覆盖React错误边界和未处理异常、桌面IPC耗时与领域错误、Rust panic和错误日志、CLI失败、AI普通/流式调用、协议健康与恢复、简报生成与定时调度、安装助手失败与更新编号。前端发送IPC时长指标；应用和CLI共用发布版本标签。IPC的前端与后端各自建span，当前没有声明已实现跨IPC分布式trace上下文传播。

错误和性能采样均为1.0，不因为额度主动缩减采样；队列仍使用SDK的有限缓存，网络故障不能阻塞正常调用。日志仅保留结构化技术信息，不把本地完整日志上传。没有启用会话回放、原始AI输入/输出收集或QQ进程内部注入。

## 启用与测试

默认不启用上报。Rust使用`EAZYQQ_TELEMETRY=1`显式启用，DSN由`EAZYQQ_SENTRY_DSN`或构建参数提供。前端从`VITE_EAZYQQ_SENTRY_DSN`获取公共摄取地址；桌面内以Rust启用状态为准，在后台初始化，不阻塞首屏；纯浏览器开发预览才使用`VITE_EAZYQQ_TELEMETRY=1`。启动早期错误最多保留8条、每条栈最多64KiB，未启用时丢弃。

`telemetry status --json`只读取状态；`telemetry test --confirm --json`发送合成测试并返回`eventId`、`flushed`和`release`。设置页的“发送诊断测试”分别测试前端与桌面。`flushed=true`只表示SDK发送队列已处理，实际接收须通过Sentry查询同一事件ID确认，不能据此冒充服务器已接收。

发布工作流已使用仓库变量`EAZYQQ_SENTRY_UI_DSN`与`EAZYQQ_SENTRY_CORE_DSN`注入公共DSN；没有把Sentry管理令牌写入代码或发布包。普通CI/单元测试不启用网络上报，Rust单元测试显式禁用SDK初始化；真实投递验收使用`.test-runtime`隔离目录与合成数据。私有开发配置在`.env.local`，不进入Git。

## 数据边界与诊断证据

发送前采用字段白名单：版本、组件、操作、错误码、更新编号、时间与必要源码位置。QQ身份、会话正文、配置内容、请求正文/头、凭据、本地目录和变量值被剔除。Sentry两个项目启用默认数据清理与IP清理；云平台仍可能有网络来源等服务元数据，不把SDK脱敏等同于平台完全不处理网络信息。异常栈保留文件名/函数/行号，源码映射与原生调试文件自动上传未在本轮配置，不宣称具备完整符号还原。

安装助手的公共DSN只在调用者已启用时通过私有请求交接，助手拥有独立组件标签。旧请求缺少该字段仍兼容；开启诊断不会重启QQ或修改账号选择。

SDK采用`@sentry/react 11.6.0`及`sentry/sentry-tracing 0.49.3`。JavaScript 11使用`dataCollection`，错误回调为`ErrorEvent`，性能回调为`StreamedSpanJSON`；不能恢复旧`sendDefaultPii`参数或对新span使用旧字段。

参考：[Sentry Rust SDK配置](https://docs.rs/sentry/0.49.3/sentry/struct.ClientOptions.html)、[Sentry React文档](https://docs.sentry.io/platforms/javascript/guides/react/)、[项目API](https://docs.sentry.io/api/projects/update-a-project/)。具体投递、隐私检查和代码验收见开发记录。
