# EazyQQ：UI/UX 设计规范与原子化组件规格书 (DESIGN.md)

<!-- impeccable:design-schema 1 -->

## 1. 视觉世界定位与设计哲学（Visual World & Philosophy）

### 1.1 模式定调：Operate 模式（以任务与高效为核心）
根据 Impeccable 规范，EazyQQ 属于典型的 **Operate 模式**（产品界面服务于用户任务）：
- **工具融入场景**：界面不喧宾夺主，杜绝无意义的繁复装饰、花哨动画或大面积饱和色块堆叠。让用户在浏览消息、审核草稿、拉取文件时获得“本该如此”的熟悉感与流畅感。
- **白色清新为主调（Light Mode Default）**：采用珍珠云白、极简纯白容器与微磨砂玻璃质感，营造出如 macOS 原生应用或现代 Notion/Linear 般的通透与雅致；
- **深色主题随时切换（Dark Mode Theme）**：完整保留现代高对比度深色模式（`#090d16` 底色），在右上角提供一键平滑切换。

---

## 2. 核心设计令牌系统（Design Tokens）

### 2.1 颜色系统（Color Palette）

#### 基础材质层级（Light Mode 基准）
| 令牌名称 | 颜色数值 | 语义定位与用途 |
| :--- | :--- | :--- |
| `--canvas-bg` | `#f8fafc` (Slate-50) | 全局底层背景，温润不刺眼，告别惨白感 |
| `--surface-card` | `#ffffff` (Pure White) | 浮动卡片、弹窗、列表项底色，配合 `shadow-sm` |
| `--surface-subtle` | `#f1f5f9` (Slate-100) | 侧边栏轨道底色、输入框浅灰底色、不可用背景 |
| `--border-subtle` | `#e2e8f0` (Slate-200) | 分割线、常规卡片边缘边框 |
| `--border-hover` | `#cbd5e1` (Slate-300) | 悬停态边框增强 |
| `--border-focus` | `#0284c7` (Sky-600) | 键盘焦点与选中态高亮边框 |

#### 文字层级（Typography Scale）
| 令牌名称 | 颜色数值 | 适用场景 |
| :--- | :--- | :--- |
| `--text-primary` | `#0f172a` (Slate-900) | 主标题、联系人名称、对话正文（高对比度阅读） |
| `--text-secondary` | `#475569` (Slate-600) | 消息摘要、时间戳、表单说明文案 |
| `--text-muted` | `#94a3b8` (Slate-400) | 占位符、未激活状态、快捷键提示文本 |

#### 业务语义强调色（Semantic Accents）
| 语义定位 | 颜色数值 | 搭配浅色背景 | 适用场景 |
| :--- | :--- | :--- | :--- |
| **品牌主色 (Sky)** | `#0284c7` / `#0ea5e9` | `#f0f9ff` (Sky-50) | 主操作按钮、选中指示器、草稿高亮卡片 |
| **自动秒回 (Mint)** | `#059669` (Emerald-600) | `#ecfdf5` (Emerald-50) | `[自动秒回]` 策略胶囊与状态灯 |
| **草稿待审 (Amber)** | `#d97706` (Amber-600) | `#fffbeb` (Amber-50) | `[草稿待审]` 策略胶囊与未处理待办计数 |
| **纯消息总结 (Iris)** | `#7c3aed` (Violet-600) | `#f5f3ff` (Violet-50) | `[纯消息总结]` 策略胶囊与简报图标 |
| **直通静默 (Slate)** | `#64748b` (Slate-500) | `#f8fafc` (Slate-50) | `[直通静默]` 策略胶囊与未启用项 |
| **危险阻断 (Rose)** | `#e11d48` (Rose-600) | `#fff1f2` (Rose-50) | 驳回、删除、重置等危险操作 |

---

## 3. 原子化组件库详细规格（Atomic Component Library）

系统严格执行 Impeccable 组件七大标准状态约束：**Default (默认)、Hover (悬停)、Focus-Visible (聚焦)、Active (按压)、Disabled (禁用)、Loading (加载骨架)、Error (错误)**。

### 3.1 基础原子组件（Atoms）

#### 1. Button（按钮）
- **尺寸档位**：
  - `sm`: 高度 28px，文本 12px，内边距 `px-2.5 py-1`，圆角 `rounded-md` (6px)；
  - `md`: 高度 36px，文本 14px，内边距 `px-4 py-2`，圆角 `rounded-lg` (8px)；
  - `lg`: 高度 44px，文本 15px，内边距 `px-6 py-2.5`，圆角 `rounded-xl` (10px)；
- **变体分类**：
  - `Primary`: `bg-sky-600 text-white hover:bg-sky-700 active:scale-[0.98]`，聚焦时 `ring-2 ring-sky-500/50`；
  - `Secondary`: `bg-white border border-slate-200 text-slate-700 hover:bg-slate-50 active:bg-slate-100`；
  - `Ghost`: 无背景与边框，`text-slate-600 hover:bg-slate-100 hover:text-slate-900`；
  - `Danger`: `bg-rose-50 text-rose-600 border border-rose-200 hover:bg-rose-100`；
- **防呆与反馈**：点击时自动触发 1000ms 防抖节流，Loading 态展示行内 14px 细线条旋转 Spinner，文案自动变更为“处理中...”。

#### 2. Badge / Tag（策略与状态胶囊）
- **规范**：高度 22px，字体 11px，字重 Medium，左右内边距 `px-2`，完全圆角 `rounded-full`；
- **结构**：`[6px 发光圆形指示点] + [策略文字]`；
- **示例**：
  - `<Badge variant="auto" dot>自动秒回</Badge>`
  - `<Badge variant="draft" dot>草稿待审 (3)</Badge>`
  - `<Badge variant="summary" dot>纯消息总结</Badge>`
  - `<Badge variant="ignore" dot>直通静默</Badge>`

#### 3. Input & Textarea（输入框与文本域）
- **外观**：纯白底色（`bg-white`），轻柔灰色边框（`border-slate-200`），文字主深炭色（`text-slate-900`）；
- **焦点状态**：`focus:border-sky-500 focus:ring-2 focus:ring-sky-500/20`，丝滑过渡；
- **防呆校验**：支持附带清空按钮（一键清空粘贴长文本），错误态边框变红（`border-rose-400`）并在底部平滑展开红色微说明。

#### 4. Switch（现代化滑动开关）
- **尺寸**：宽 38px，高 22px，轨道内圆球直径 18px；
- **动效**：`transition-transform duration-200 ease-out` 弹性位移；
- **色彩**：未激活时冷灰（`bg-slate-200`），激活时天青蓝（`bg-sky-600`）。

#### 5. Tooltip（微提示气泡）
- **定位**：基于 Portal 浮于顶层（逃逸容器溢出限制，绝不被 `overflow-hidden` 截断）；
- **样式**：深黑背景（`bg-slate-900/95`），纯白 12px 细文本，圆角 `rounded-md`，带微弱指向小三角；
- **时序**：鼠标悬停停留 300ms 触发，移出立即消失。

#### 6. Skeleton（骨架加载屏）
- **拒绝界面中心傻转的大 Spinner**。联系人列表、消息流与群文件树加载时，采用浅灰平滑脉冲色块（`bg-slate-100 animate-pulse rounded-md`）模拟真实组件轮廓。

---

### 3.2 复合分子组件（Molecules）

#### 1. ContactListItem（联系人/群列表项）
- **布局**：左右结构，高度 60px，外侧圆角 8px，内边距 `px-3 py-2`；
- **左侧**：40px 圆形头像（群聊为圆角方形头像）+ 状态角标；
- **中间**：第一行显示联系人/群备注名称（加粗），第二行显示最新一条消息剪影（超长单行省略 `truncate`）；
- **右侧**：顶部展示时间戳（如 `14:23` 或 `昨天`），底部并排展示对应策略胶囊药丸（如 `[自动]`、`[草稿]`）与红色未读圆点；
- **状态**：激活项带有 `bg-sky-50/80 border-l-4 border-sky-600` 明确高亮。

#### 2. DraftActionCard（草稿待办审核卡）
- **容器**：纯白底色带微天青浅边框（`border-sky-200 shadow-sm rounded-xl p-3.5`）；
- **头部**：展示原发送者名字、接收时间及标签「AI 拟回复建议」；
- **正文**：浅蓝高亮文本区域，排版清晰；
- **底部操作栏**：
  - 绿色按钮「一键发送 (Ctrl+Enter)」；
  - 灰色按钮「微调修改」；
  - 红色幽灵按钮「丢弃 (Esc)」。

#### 3. FileItemRow（文件条目行）
- **呈现**：文件类型彩色专属图标（PDF 砖红、DOCX 湛蓝、代码文件翠绿、压缩包金黄）；
- **信息**：文件名、格式后缀、体积大小（如 `14.2 MB`）、上传者昵称及上传时间；
- **操作区**：悬停出现浮动按钮组：「下载到本地」与「AI 智能提取与总结」。

#### 4. ModelProviderCard（大模型供应源选择卡）
- **外观**：扁平圆角卡片，左侧品牌 Logo（OpenCode、DeepSeek、OpenAI、Claude、Ollama）；
- **指标**：网络延迟测试指示灯（绿点 `28ms`）；
- **状态**：单选 Radio 结合卡片整体选中边框高亮。

---

### 3.3 业务有机体（Organisms）

#### 1. SidebarRail（左侧导航轨道 - 64px）
- 紧凑竖向单列，纯浅灰底色，居中排列核心功能图标；
- 悬停图标呈现微悬浮底色，激活图标呈现天青实心填充与左侧小指示条；
- 底部常置：应用版本号、深/浅主题快速切换按钮与「使用说明书 (F1)」入口。

#### 2. ContactListPanel（中间列表与检索面板 - 300px）
- 顶部集成全局即时搜索框（支持拼音、QQ号、群名即时过滤）；
- 顶部二级筛选胶囊：`全部` / `群聊` / `好友` / `待审核草稿 (3)`；
- 底部支持一键批量操作入口。

#### 3. GroupFileExplorer（群文件可视化中心）
- 顶部带有群选择器与文件夹面包屑导航条（如 `项目资料群 > 2026年开发文档 > ...`）；
- 支持列表模式与网格大图标模式自由切换；
- 顶部醒目主操作按钮：「一键全量同步该群文件」与「汇总全群文件生成知识库速览」。

#### 4. BuiltInManualDrawer（内置说明书原生侧滑抽屉）
- 用户在任意界面按下 `F1` 或点击导航栏说明书图标时，从右侧平滑滑出 480px 宽度的帮助手册抽屉；
- 抽屉内置全局搜索框，左侧带小目录树，右侧展示图文排版清晰的实战手册，随查随关，完全不打乱当前正在进行的任务。

---

## 4. 动效与交互节奏规范（Motion Tokens）

- **时长（Duration）**：交互状态过渡一律控制在 `150ms ~ 200ms`，禁止长于 300ms 的拖沓过渡；
- **缓动曲线（Easing）**：采用现代标准贝塞尔曲线 `cubic-bezier(0.16, 1, 0.3, 1)`（迅速启动、柔和回落）；
- **微交互场景**：
  - 卡片 Hover：轻微上浮 1px，投影由 `shadow-sm` 平滑加深为 `shadow-md`；
  - 扫码页面：二维码外围容器环绕低饱和度的晨曦蓝平缓呼吸光晕；
  - 抽屉与模态框：伴随 `150ms` 的 20% 灰度半透明遮罩淡入。

---

## 5. 纯矢量图标与视觉资产设计体系（Iconography & Visual Assets）

系统严禁在界面使用任何参差不齐、操作系统显示各异的 Unicode Emoji，全部统一采用极具现代专业质感的纯矢量 SVG 图标库（基于 `@lucide/react`），统一线条粗细为 `1.75px`，严格对齐像素网格：

### 5.1 核心功能与业务图标映射表（Lucide React Mapping）

| 业务分类 | 功能语义 | 对应 Lucide 图标名称 | 尺寸规范 | 配色规范（白色基调） |
| :--- | :--- | :--- | :--- | :--- |
| **主导航** | 会话与策略面板 | `<MessageSquare />` | 20px | 默认中灰，激活天青蓝 |
| **主导航** | 待审草稿箱 | `<Inbox />` | 20px | 默认中灰，有待办时带琥珀小圆点 |
| **主导航** | 群文件智能中心 | `<FolderGit2 />` | 20px | 默认中灰，激活天青蓝 |
| **主导航** | 智能简报与纪要 | `<FileText />` | 20px | 默认中灰，激活鸢尾紫 |
| **主导航** | 参数设置中心 | `<Settings />` | 20px | 默认中灰，激活深炭黑 |
| **主导航** | 图形化使用说明书 | `<BookOpen />` | 20px | 默认中灰，悬停加亮 |
| **策略状态** | 自动秒回模式 | `<Zap />` | 14px | 薄荷青绿 `#059669` |
| **策略状态** | 草稿待审模式 | `<Clock />` | 14px | 暖阳琥珀 `#d97706` |
| **策略状态** | 纯消息总结模式 | `<Sparkles />` | 14px | 鸢尾紫晶 `#7c3aed` |
| **策略状态** | 直通静默模式 | `<EyeOff />` | 14px | 柔灰 `#94a3b8` |
| **文件类型** | PDF 文档 | `<FileText className="text-red-500" />` | 18px | 红色专属标识 |
| **文件类型** | Word / 文档 | `<FileText className="text-blue-500" />` | 18px | 蓝色专属标识 |
| **文件类型** | 代码源文件 | `<FileCode className="text-emerald-500" />` | 18px | 翠绿专属标识 |
| **文件类型** | 压缩包归档 | `<Archive className="text-amber-500" />` | 18px | 金黄专属标识 |
| **文件类型** | 图片附件 | `<Image className="text-indigo-500" />` | 18px | 靛青专属标识 |
| **通用操作** | 发送 / 放行草稿 | `<Send />` | 14px | 纯白（按钮内部） |
| **通用操作** | 刷新二维码 | `<RefreshCw />` | 16px | 晨曦蓝 `#0284c7` |
| **通用操作** | 下载文件 | `<Download />` | 16px | 中灰，悬停深炭 |
| **通用操作** | 全局帮助提示 | `<HelpCircle />` | 14px | 柔灰 `#94a3b8` |

### 5.2 品牌专属矢量 Logo 设计（Brand Emblem SVG）
EazyQQ 拥有独立的现代化几何矢量 Logo（位于 `src/renderer/assets/logo.svg`），无需任何位图位姿：
- **几何结构**：由两条优雅的流线对聊气泡轮廓交叠构成，交汇核心点嵌入一颗微光脉冲四角星（象征 AI 与即时通讯的无缝融合）；
- **色彩渐变**：天青蓝（`#0284c7`）向电青蓝（`#38bdf8`）的 135 度线性渐变，在纯白背景与深色背景下均具备极致的辨识度。

### 5.3 极简无杂质空状态微插画（Empty States）
当草稿箱无内容、群内暂无文件或未选中联系人时，不使用呆板的一行文字，而是采用纯 CSS/SVG 绘制的极简线条插画：
- **无待办草稿**：由纯线条绘制的信封闭合图标，配标题“草稿箱已全部处理完成”，副标题“AI 生成的新拟回复将在此处呈现待审”；
- **群文件暂空**：由线条文件夹与向下的微光箭头构成，配标题“该群暂无可拉取文件”，配按钮“点击立即从 QQ 同步”。

