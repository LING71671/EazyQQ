# 0.5.3 中止重装后的二维码恢复

## 起始状态与证据

用户安装 0.5.2 后二维码一直等待。只读检查发现当前进程启动于 14:41:14，协议三个端口没有监听；旧 resources/napcat 的 Windows Job 查询成功且有 5 个活动进程。旧目录缺少 napcat.mjs、loadNapCat.cjs、qqnt.json 和 config/webui.json。旧引导日志最后为 08:53:07。账号登记不存在，bootstrap 仍有已选择账号，因此按兼容逻辑继续绑定旧存活 Job。中止旧安装留下不完整资源与存活进程；不能把存活等同服务就绪。

## 决策与边界

新增 runtime/recovery.rs 承载缺失资源检查与旧目录替换准备。普通状态读取只报告故障，禁止因超时重启或快速登录。显式恢复先校验新版载荷、准备私有副本并验证 QQ 路径，随后沿原有随机 Job 所有权定向停止旧树。资源准备失败保留旧树；外部管理的监听服务仍拒绝重启。旧资源迁移仅限当前安装根 resources/napcat，不移除用户目录或修改系统 QQ 安装。其他 QQ 会话保持。

旧目录迁移后重建桌面上下文，使 NapCat 客户端读取新目录的私有 token。显式自愈只为旧目录缺失必需文件且受管理的故障增加此恢复分支；健康、外部和普通等待扫码分支保持。CLI restart 复用同一恢复路径。

前端区分加载、失败与实际二维码；15 秒状态查询提示和 20 秒二维码等待提示不解除仍在执行的操作，也不并发重复请求。协议探测最多 3 秒，二维码和记忆账号查询并行各最多 5 秒；离线不查询记忆账号。领域错误和 Promise 拒绝均可见。

## 验证及未完成项

前端 25 项、Rust 112 项、类型和生成接口已通过初次回归。新增回归覆盖查询超时不重叠、领域错误恢复、显式恢复失败、20 秒等待结束、有效二维码展示、私有副本与配置保留、替换资源不完整时不触碰目标。真实浏览器渲染确认错误可见、一次显式恢复后二维码出现，pollingReadOnly=true、realQQTouched=false；截图 output/ui/login-unavailable.png 与 login-recovered.png 已查看。

本轮未在本机重复启动真实 QQ 测试，也未终止用户 QQ。真实安装包构建、CLI 与原生验收及正式发布待最终回执。模拟二维码不等同真实 QQ 登录成功。用户要求登录态与内存方案仅讨论，未实现。

## 真实 CLI 进程恢复回归

scripts/validation/protocol_recovery.py 在独立 .test-runtime 目录创建损坏旧协议 Job，并用不会执行 QQ 或加载钩子的 C++ 启动器模拟新进程。先验证替换载荷缺失时旧 Job 保持，再补齐载荷执行真实 CLI restart：旧 Job 退出、账号私有目录出现 CJS 与修补清单、登记路径更新，身份、端口和 auto_start 保留；随后 stop 仅停止新 Job，旁路进程仍存活。报告 damagedLiveJobRecovered=true、incompleteSourcePreservedOldJob=true、registrationAndPortsPreserved=true、unrelatedProcessPreserved=true、realQQTouched=false。该回归纳入正式发布门禁，不依赖二维码模拟来证明进程恢复。

## CI 失败核对

用户截图包含历史失败。已复查运行 37874193184：真实 QQ WebUI 已监听但二维码仍生成，旧脚本立即判失败；现脚本只对该生成状态执行有 45 秒上限的等待，后续 37889026746 成功。37804906313 为 pnpm/action-setup v4 不兼容 pnpm 11；37862735100 与 37812548639 为提升权限 runner 下 WebView2 调试入口不可用。历史已按官方机制修复，正式 0.5.1/0.5.2 完整流程分别 37889753582/37893262952 成功。0.5.2 的覆盖缺口是没有验证中止安装后仍存活且载荷缺失的旧 Job，本轮新增实际进程回归。
