# 主程序安装交接回执

## 起始状态

8916e0c的日常CI前端通过，Windows仍在Rust步骤，任务38101562453已重新确认存活，未重启。后续路线第2项的源码检查显示：主程序更新准备/交接返回字符串，助手只在失败时写单独error.log；GUI退出与CLI命令结束后无法通过公共接口查询安装最终结果。交接请求用PID命名，同一进程重复请求可能覆盖。

## 本轮实现与边界

新增infra/updates/receipts.rs和只读查询。随机32位十六进制编号关联请求和回执，准备好的目标版本传给回执；助手在等待调用者、复查校验、安装、退出结果时记录状态。旧助手请求缺少编号仍兼容运行，旧error.log保留。API升级返回继续为字符串并包含编号；新增get_update_history及updates history，客户端和生成覆盖一致。JSON原子写、系统锁、目标身份/文件大小检查与末态保护避免迟到回盖；结果最多50条，读取不创建目录。

成功阶段名为installer_succeeded，必须有安装阶段与退出0；它不证明目标版本已安装，也不保证重新打开的程序就绪。未结束记录不按时间猜失败，当前尚未实现进程退出后的中止归类。此次只交付安装交接回执，下载、核心更新、进度编号、查询界面、验证新版本与存储留存策略后续继续完成；路线第2项保持进行中。没有升级本机、操作真实QQ或实施登录/内存策略。

## 验证计划

领域回归覆盖跨进程持久读取、末态保护、失败与独立重试、身份/路径拒绝、只读空查询与50条返回上限；实际Windows助手使用隔离C夹具，分别退出0及9。类型、前端、55/59契约、Rust、CLI和云端结果待执行后追加，不以旧0.6.4安装回执替代新实现证明。

## 功能与验收回执

用户进一步明确以新功能、体验和代码健壮性为重点，本轮新增设置页主程序更新记录：显示阶段、版本、时间、退出码和失败原因，编号可展开；查询串行、错误可重试，损坏的字段/日期/成功证据拒绝渲染。沿用Slate/Sky及功能SVG，不增加装饰或回放。37前端、140库+2CLI Rust、55/59契约、类型/展示门禁通过；27CLI模拟冒烟通过。实际助手的隔离安装夹具退出0和9均记录终态，跨进程caller退出后仍可读取，未运行用户安装包。

首次尝试不存在的custom-protocol项目feature被Cargo拒绝；直接cargo release构建虽编译成功，仍使用开发地址，原生驱动未找到tauri.localhost。检查本机Tauri crate明确custom-protocol=[tauri-macros/custom-protocol]，改用scripts/cargo.ps1 build --release --bin eazyqq --features tauri/custom-protocol进入真实生产入口。没有扩大原生90秒等待期限或跳过断言；失败记录update-history-native.log保留。

native-profiles-8ssvqu58真实WebView验收通过updateHistoryEvidence、GUI/CLI回执一致、1100×740/900×600无横向溢出、已成功执行安装程序仍待核验版本、失败退出码可见；原有账号往返、缓存、模型健康、窗口及F1均通过，realQQTouched=false。两个update-history截图均已直接查看，状态与记录对应，未进行额外视觉微调。日志update-history-native-confirm.log、update-receipts-rust-confirm.log，生产构建update-history-production-build.log。本轮新功能仍为源码开发状态，没有覆盖公开0.6.4或升级用户安装。
