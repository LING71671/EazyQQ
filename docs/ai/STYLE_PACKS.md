# 本地聊天风格包

在「系统设置 → 聊天风格包」导入 JSON 文件。导入成功后立即选中并启用，只影响当前账号的新回复；已有草稿需要重新生成。可以切换已导入包、启停、导出与移除当前包。导出选择尚不存在的 `.json` 文件，避免覆盖原文件。

## 格式与边界

风格包是有版本的表达规则，不是可执行提示词、模型配置或聊天记忆。只接受下列结构，未知字段、控制字符、越界容量、未知表情编号和含预定义指令关键词的短语会被拒绝；关键词校验不能识别所有指令语义。导入最大 64 KiB，每账号最多保存 32 个包。

```json
{
  "schemaVersion": 1,
  "id": "plain-short",
  "name": "简短日常",
  "version": "1.0.0",
  "rules": {
    "sentenceLength": "short",
    "punctuation": "minimal",
    "tone": "neutral",
    "lineBreaks": "natural",
    "preferredPhrases": [],
    "avoidPhrases": [],
    "qqFaceIds": ["76", "124"]
  }
}
```

句长可选 `short/balanced/detailed`；标点可选 `minimal/standard/expressive`；语气可选 `casual/neutral/warm/direct`；换行可选 `single/natural/frequent`。换行只控制本条回复的表达，不等于多次 QQ 发送。偏好短语最多 12 条、每条 12 字；避免短语最多 16 条、每条 24 字；QQ 表情最多 16 个，必须属于应用内置目录。短语作为语言片段处理，不能扩大权限、改变消息身份或要求模型编造事实。

V1 可选 `statistics` 保存 `messageCount/medianChars/p90Chars/questionRate/exclamationRate/faceRate/multiMessageRate` 聚合指标，不保存完整原始聊天、好友身份、源文件路径、账号或密钥。自定义短语仍应在分享前自行检查。

## V2：有证据的条件画像

V1 的总体统计和短语列表只能当基础配置，不能证明风格准确。V2 使用 `schemaVersion: 2` 并要求 `profile`；保留已有 `rules`。V1 不能携带 `profile`，本机账号安装索引继续使用版本 1，不影响旧包。

| 字段 | 内容与边界 |
| --- | --- |
| `methodology`、`sample` | 分析方法、训练样本/正文/回合/会话数、准确日期、独立留出回合数；训练和留出分开计数 |
| `metrics` | 正文长度、回合气泡分布、无句尾标点/纯问号/独立及混合表情比例、可解析原生编号的目录覆盖；均不是回复配额 |
| `patterns` | 最多 16 条条件模式，含情境、观察、证据数、跨会话数和 low/medium/high 证据强度 |
| `demonstrations` | 最多 24 组经选择的情境与本人短回复，每组最多 5 句、每句最多 100 字符；只允许已知原生 face，保留匿名训练来源引用 |
| `limitations` | 样本和方法局限，不能把词法分类、格式通过或历史分布稳定包装成拟人准确率 |

画像和示例按 JSON 数据装入固定账号的普通、流式及重新生成提示词，不作为系统指令执行。当前来话、身份、原始材料及已知事实优先；示例里的经历、时间、关系或承诺不是当前本人事实。V2 示例可能包含经过选择的私人原句，分享包前仍须检查；开源仓库只放通用实现和合成夹具。

正文长度和问号比例以正文消息为分母；表情比例以有正文或原生表情的消息为分母，纯材料占位不参与。多气泡与单条换行分开测量，不能由多气泡比例直接推导频繁换行。`holdoutTurnCount` 是独立留出集合的表达回合数，不要求小于训练回合数。

## 存储与开源

包保存在固定账号私有目录的 `style-packs/<id>.json`。`style-pack.json` 是本机安装索引，包含绑定账号、选中包与启用状态，不能作为共享风格包导出。导出仅写风格包本体。

开源代码提供通用机制、合成夹具及示例，不内置个人风格。私人包建议保存在 `output/private-style-packs/` 或工作区外；该目录已被 Git 忽略，发布资源门禁同时拒绝风格包私有目录与安装索引。不要把原始聊天放进 `docs/`、`src/` 或协议资源。

## 从自己的本地记录提炼

项目根目录运行以下命令，替换账号和路径；脚本只读本地 QCE modern chunked HTML，不运行导出 HTML 中的脚本，不联网、不调用模型。

```powershell
node scripts/ai/import-chat-style.mjs --input "D:/MyChatExports" --owner 10001 --output "output/private-style-packs/my-style.qqstyle.json" --id my-style --name "我的聊天风格"
```

脚本按导出元数据中的明确 UIN/UID 映射提取作者，不能依赖昵称或 `isSelf`。整条排除含引用、转发、系统内容和明确 AI 标记的记录；只将跨多个会话反复出现的保守短反应作为可选片段，不导出聊天正文、联系人或事实。需要保存审计文件时添加 `--audit "output/private-style-packs/my-style.audit.json"`，将审计与包一同留在私有输出目录。

导出格式无法证明未标记的旧文本一定由人手写，统计也不能证明关系亲疏。生成结果是待本人检查的表达配置，不承诺完全复现本人或无法被识别。

## 可复核分析与编包

需要有证据的画像时，先分析语料，再逐条审查训练示例与条件模式，最后编译。以下均为虚构路径与账号，不联网、不调用模型、不发送 QQ 消息。

```powershell
node scripts/ai/analyze-chat-style.mjs --input "D:/MyChatExports" --owner 10001 --output "output/private-style-packs/analysis.json"
node scripts/ai/compile-style-profile.mjs --analysis "output/private-style-packs/analysis.json" --decisions "output/private-style-packs/reviewed-decisions.json" --output "output/private-style-packs/reviewed.qqstyle.json" --audit "output/private-style-packs/reviewed.audit.json"
```

分析器核验作者，隔离冲突重放，分开 text-content 与原生表情，按 120 秒本人连续发言和 30 分钟会话段重建回合。报告含会话集中度、按会话等权与按消息加权结果、确定种子的聚类 bootstrap、时期变化及逐会话尾段留出。去内容重合的留出偏向少见表达，因此同时保留原始尾段对照；不能据此声称全局时间预测或真人盲评通过。

决策文件须包含版本/编号/名称、训练准确日期、条件模式、训练回合引用、示例选择和可选短语。`demonstrations` 使用 `turnId/situation/context/messageIndexes`；情境说明可经语义复核后泛化，回复正文从训练原文读取，不能手造“原始例子”。材料、缺失表情、未知来源、留出回合或 Unicode 图符不能悄悄变成示例。编译器只创建新输出，审计保存分析与决策 SHA256、逐例消息来源、取舍索引与变换边界。

原生目录由 `node scripts/ai/sync-native-faces.mjs --check` 离线核对，合并受 SHA256 固定的本地经典快照与受 commit/SHA256 固定的公开 QFace 原生索引；快照不依赖随版本移动的协议资源目录。只接纳有证据的原生类型与编号，预览使用真实 APNG/GIF 资源；发送保持 `face{id}`。未映射编号显示明确名称回退，不能凭缺失编号猜一个 Unicode 表情。

## CLI 与 MCP

```powershell
eazyqq_cli --account 10001 style-packs --action import --file "output/private-style-packs/my-style.qqstyle.json" --json
eazyqq_cli --account 10001 style-packs --action select --id my-style --json
eazyqq_cli --account 10001 style-packs --action disable --json
eazyqq_cli --account 10001 style-packs --action export --id my-style --output "D:/Exports/my-style.qqstyle.json" --json
```

省略 `--action` 为只读列表；其余操作为 `enable/remove`。MCP 的 `manage_style_packs` 工具复用相同领域服务。安装、切换及提示词加载均使用服务创建时绑定的账号目录，不依据运行过程中全局选择变化重新绑定。
