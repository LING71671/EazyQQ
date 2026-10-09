# 跨进程物理路径修复

## 起始状态与原因

从 main 的 e39c1fd 开始，公开 0.5.3 仍存在加载器找不到的问题。此前同机真实 QQ 对照已确认：Codex 后代进程打开逻辑 AppData 时，文件实际位于 LocalCache；WMI 脱离启动的 QQ 不继承这一目录视图。文件存在、junction 指向正确、调用者 require.resolve 成功，都不能证明 QQ 能读取入口。

本轮继续修复源码。用户在此前诊断后报告黑屏，系统事件只确认 20:59:44 异常关机和 21:00:48 重启，没有足够证据确定原因；转储目录读取被 Windows 拒绝。为避免扩大变量，本轮没有启动真实 QQ，没有注入 DLL、修改驱动或注册表。黑屏未判定为已修复。

## 决策与实现

新增 `services/infra/system/filesystem.rs`，使用 Rust `canonicalize` 取得存在文件/目录的最终物理路径，再将 Windows verbatim 盘符和 UNC 前缀转为接收程序可用的路径。拒绝设备命名空间与非绝对路径；未创建的输出文件先解析实际父目录。不能通过当前包身份猜测路径，也不能硬编码 Codex 包名。

Rust 官方说明 Windows `canonicalize` 对应 `CreateFile` 与 `GetFinalPathNameByHandle`：[标准库文档](https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html)。目录联接的父目录必须先解析；若直接解析完整桥链接会跳到另一盘的账号目录，无法生成 Electron 要求的同盘相对入口。

`runtime/entry.rs` 解析实际 QQ app 目录与账号加载器。同盘直接生成相对入口；跨盘候选桥根先创建并解析物理父目录，再检查盘符、加锁和创建 junction。优先候选不可写或最终盘符不合适时尝试后续候选；既有桥只能复用相同目标。盘符比较兼容 verbatim 前缀。

启动完成私有准备后解析实际协议目录，主模块 URL、加载器凭证、NAPCAT 环境路径和注入参数来自这个目录。QQ 可执行文件、监督进程副本、监督请求目录与输出日志父目录也按实际路径交接。更新安装助手的安装包、助手副本、交接 JSON、工作目录和显式数据根覆盖采用实际路径；SHA256、大小校验、退出等待及所有权边界保留。

诊断桥根使用共用函数，避免简单删除字符串前缀破坏 UNC。IPC 与 CLI 字段未改变；认证证据仍只读，端口或加载凭证不能冒充身份确认。

## 验证证据

回归覆盖：verbatim 盘符、UNC、设备路径拒绝、不存在输出文件的实际父目录、逻辑加载器别名、桥父目录先解析、独立 WMI 接收进程内 Node `require.resolve` 与内容读取。所有单元测试数据仍位于每进程 `.test-runtime`。更新助手回归启动的是测试安装程序；CLI 冒烟使用模拟协议，`realQQTouched=false`。

本机编译一个只包含正式 `filesystem.rs` 的小型解析程序，读取既有 AppData 目录；再通过 WMI 启动普通 Node，仅执行文件读取与解析，不导入协议或 QQ 业务模块。`.test-runtime/namespace-receiver-proof.json` 显示逻辑桥路径返回 `MODULE_NOT_FOUND`，物理桥读取 1004 字节并 resolve 到保留的私有测试加载器。这确认共用解析函数在当前实际重定向环境有效；不是完整 QQ 模块加载验收。

最终完整 Rust 复验通过：119 项库测试、2 项 CLI 测试，共 121 项；25 项前端测试、类型检查、生成契约检查及 24 项 CLI 冒烟通过。证据为 `.test-runtime/physical-path-verified-rust.log` 与 `physical-path-cli-smoke.log`。构建目录使用 `A:/DevEnv/Caches/cargo/eazyqq-diagnostics`，B 盘可用约 47.6 GiB，没有恢复 B 盘 debug 大缓存。

## 兼容边界与未完成项

未改写 QQ 安装文件，未操作其他 QQ 的会话，没有发送消息或实施登录态/内存策略。旧入口和账号数据保留；新入口在下次显式启动准备时重写。源码仍使用开发基线版本 0.5.3，不能用版本号相同推断当前安装已包含修复。

真实 QQ 加载新生成入口、模块导入凭证和 WebUI 就绪仍须验收。此次独立 Node 验证不能代替这三项；尚未生成、安装或公开发布新的修复包。后续发布须统一新版本、完整打包与安装验收，保持公开标签不可变。
