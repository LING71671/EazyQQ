pub fn cmd_help() {
    println!(
        r#"EazyQQ CLI - 无界面控制通道 (v{version})

用法: eazyqq-cli <命令> [选项]

协议与登录
  status                          协议进程、登录态、二维码可用性一览
  login-info                      当前登录 QQ 号与昵称
  quick-login-list                列出可免扫码快速登录的账号
  quick-login --uin <QQ号>        对指定账号执行快速登录（免扫码）
  qr [--save <path>]              获取真实登录二维码（未登录时有效）

联系人与规则（默认全部拒绝）
  contacts [--type friend|group]  同步真实好友/群列表并列出规则
           [--search <关键词>] [--whitelist] [--summary-only]
  groups                          原始群列表（直连 OneBot）
  friends                         原始好友列表（直连 OneBot）
  rule --target <id>              修改规则
       [--mode auto_reply|copilot|ignore]
       [--summary on|off] [--interval-hours <N>]
       [--trigger all|at_me|keyword] [--keywords a,b,c]

消息与 AI
  send --target <id> --text <文本>  真实发送消息（群号自动识别为群聊）
  ask  --target <id> --text <提示>  AI 生成一条候选回复（不发送）
  drafts                          列出待审核草稿
  draft-send --id <草稿ID>        放行草稿并真实发送 [--text <覆盖内容>]
  draft-dismiss --id <草稿ID>     丢弃草稿
  draft-regenerate --id <草稿ID> --instruction <微调指令>  按指令重新生成
  history --target <id> [--limit N] 查看本地消息流水
  mark-read --target <id> [--at <毫秒时间戳>]  清除该会话未读标记

简报
  summarize --target <群号> [--hours N] [--prompt <提示词>]   立即生成真实 AI 简报
  scheduler-tick                  强制执行一次定时总结调度（与后台循环同一逻辑）
  summaries [--target <id>]       列出已生成的群简报
  whitelist-groups                列出已加入简报白名单的群

群文件
  files --target <群号>           同步并列出群文件（含本地下载状态）
  file-download --target <群号> --file-id <文件ID>   下载到工作区
  file-summarize --path <本地路径>  提取文档文本并生成结构化综述

配置与诊断
  config [--json]                 打印当前配置
  set-config --key <k> --value <v> 写入 app_settings
  ai-config                       查看实际生效的大模型供应商配置
  ai-detect                       探测本机可用的 OpenAI 兼容推理端点
  ai-set --provider <p>           切换供应商（省 token 可指向本地端点）
         [--base-url <u>] [--model <m>] [--key <k>]
         [--temperature <t>] [--max-context <n>]
  ai-test                         对当前供应商发起真实连通性测试
  napcat-doctor                   协议端启动路径诊断：逐步骤走查，指出第一个卡住的地方
  selftest                        关键不变量自检（安全边界、触发、冷却、脱敏）
  config-audit                    审计配置项：找出「界面上能改但后端不读」的设置
  chain-status                    全链路状态：逐环节体检并定位第一个断点
  health [--deep]                 依赖与链路自检（--deep 会真实调用大模型）
  export                          导出脱敏诊断包 zip
  simulate --target <id> --text <消息>  调试后门：注入模拟消息走完整处理链路
           [--type group|friend] [--sender-name <名>] [--sender-id <号>]
  mcp                             启动 Model Context Protocol (stdio) 供外部 AI 接入
  schema                          导出机器可读的全量命令与工具 Schema 契约
  log-path                        打印日志文件路径
  log-tail [--lines N]            查看日志尾部
  version                         版本信息

通用选项
  --json                          以 JSON 输出，便于脚本消费
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
