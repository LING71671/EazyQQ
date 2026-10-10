# 0.6.1 登录首屏与展示规范

## 起始状态和原因

用户反馈 0.6.0 登录页需要翻页才能判断状态，并明确禁止全项目廉价 emoji。一键登录实际使用 Lucide 星光图标，虽然不是字符 emoji，但属于同类星光装饰，按用户要求一并移除。旧结构把三行记忆账号、二维码、提示、操作垂直堆进窄卡片，放大留白造成首屏溢出。

## 决策与边界

保留 Slate/Sky 和现有侧栏；登录工作区按自身宽度使用双栏。左侧扫码/已确认会话，右侧记忆账号与展开式批量管理。列表局部滚动，不让账号数量推走扫码操作。删除无操作意义的顶端装饰和宣传脚注。目标扫码放在左侧，取消与身份验证不变；状态观察不快速登录，不修改等待扫码或外部 QQ 会话，不发送消息。

记忆账号列表抽到 components/accounts/QuickLoginAccounts.tsx；登录按钮只有文字，处理中加载提示保留。全项目自有聊天、文件、简报、设置中的 emoji/星光，健康状态中的字形标记、CLI/安装脚本装饰字符清理；噪声过滤测试用 Unicode 转义表达外来消息样本，不削弱过滤。第三方不可变协议载荷与用户消息内容不重写。

## 防回归和证据

scripts/validation/presentation.mjs 扫描自有文本源码、中文文档、CI 与脚本；构建和发布门禁执行 pnpm presentation:check。29 项前端测试通过，新增明确点击才登录、纯文字按钮和批量入口展开行为检查。真实原生验收使用模拟登录与三条记忆账号，通过 CDP 验证默认/最小 WebView 内容视口的边界、页面滚动和按钮图标；本轮不操作真实 QQ。完整 Rust、CLI、截图查看、分发与公开升级仍待追加，不能提前宣称完成。


## 本机验收中发现的限制

首轮本机原生程序被已安装 0.6.0 的全局单实例锁拒绝，进程退出码 0，没有取得 WebView；不是布局通过。验证路径后仅关闭 A:/EazyQQ/eazyqq.exe，再启动独立模拟程序，两个真实 QQ 主进程保留。第二轮默认布局截图确认核心任务首屏可见；最小窗口断言误计 closed details 的隐藏子按钮，因为它们仍能报告几何位置，改按 checkVisibility 筛选实际绘制操作，核心二维码与常驻入口的断言未削弱。


## 最终本地候选

128 项 Rust（126 库、2 CLI）、29 项前端、26 项 CLI 冒烟、54/59 IPC/CLI 覆盖、类型与展示规范检查通过。原生确认日志 `.test-runtime/release-0.6.1-native-confirm.log` 返回 loginFirstViewport、免费检测、账号往返、迟到状态选择保持、独立浏览器缓存与窗口回归全部为 true，realQQTouched=false。直接查看 `.test-runtime/native-profiles-a52zp4q4/login-layout-1100x740.png` 与 `login-layout-900x600.png`，几何结果 login-layout.json 的四项可见性检查成立；不继续开放式视觉修改。

安装包与六项分发资产已生成，资源698文件检查通过；本地候选来源为提交前 HEAD，不公开上传。公开门禁、下载散列与本机升级结果待追加；旧 0.6.0 标签和资产保留。两个真实 QQ 主进程 29952、21948 未结束；本轮只关闭已核对身份的 EazyQQ 窗口以解除本机测试单实例冲突。


## 云端失败溯源

作业 38022566059 的前端、Rust、构建、实际安装、26 CLI 与旧 Job 恢复通过，原生脚本初始化失败：ModuleNotFoundError: login_layout。CI 将 native_profiles.py 复制到 RUNNER_TEMP 再提升执行，但新子模块仍在项目目录；本机原位执行未覆盖这个边界。这是新增验证脚本的路径假设错误，不能称 CI 通过，也没有发布或本机升级。

修复从 EAZYQQ_TEST_PROJECT_ROOT 确定 scripts/validation 导入目录，在复制后的入口解析子模块。0.6.1 未公开标签保留，修复进入 0.6.2；本机增加与 CI 相同的临时复制 + 项目根变量验证，不扩大超时或删减断言。


## 0.6.2 临时入口补验

本机完整构建与打包通过；将修正后的 native_profiles.py 复制到 .test-runtime/native_profiles-relocated.py，显式 EAZYQQ_TEST_PROJECT_ROOT=B:/EazyQQ，以发布二进制执行，报告所有原生断言通过、realQQTouched=false。日志 .test-runtime/release-0.6.2-relocated-native.log，隔离目录 native-profiles-mevp5z2g。布局源码未再次修改，前端29/Rust128/CLI26的0.6.1行为证据保留，0.6.2完整CI仍需独立通过。没有把复制执行成功替代尚未完成的云端安装与公开分发。


## 0.6.2 云端调试地址溯源与修正

完整作业 38023862888 的类型、29前端、128Rust、构建、重装、26CLI、旧Job恢复、首屏与免费模型检查通过；账号10002建立后，CDP发现超时。下载失败日志和诊断包证明新账号独立 WebView 与前端IPC已运行，netstat显示 [::1]:59596 LISTENING（PID888），而127.0.0.1没有监听。验证器把IPv4视为唯一调试地址，错误地把调试连接失败报告为原生账号未就绪；不是身份或缓存隔离失败。

仅修正验证驱动：发现与关闭验证同时检查IPv4/IPv6，WebSocket Host保留括号authority；90秒总期限和账号/缓存断言不变。新增真实IPv6独占HTTP/WebSocket回归 webview_ipv6.py，验证fallback与Host实际交换，本机通过。完整临时入口原生回归 release-0.6.2-dual-stack-native.log 全部通过，realQQTouched=false。

应用二进制及公开标签80b6ecba7c0c9f265d1a51dd11363feb5b0335ab不变。既有工作流已经规定验证工具跟随工作流修订、应用源码保持标签；修正的主分支验证脚本经显式手动dispatch重跑v0.6.2，避免移动标签或给验证工具修订再次增加应用版本。发布门禁必须重新完整通过。
