# EazyQQ 视觉设计与交互规范书 (DESIGN.md)

<!-- impeccable:design-schema 1 -->

## 1. 视觉世界定调与铁律 (Operate 模式)

EazyQQ 属于典型的 **Operate 模式**（工具融入场景，高效完成任务）：
- **清爽功能性画布**：界面杜绝无意义的装饰色块或复杂动效，降低视觉干扰，专注高可读性与高处理效率。
- **零冗余文字铁律 ("非必要不添加文字")**：
  - **空状态**：保持纯净功能画布与单行简短状态提示。严禁在空状态下渲染欢迎导语、使用教程或营销说明段落。
  - **禁止装饰性提示词胶囊**：严禁注入示例 Prompt 卡片、预置问答气泡或非必要的引导按钮。
  - **精简界面标签**：仅保留关键功能标识、有效数据胶囊与操作按钮。
- **晨曦纯白为主调，高对比度暗黑自由切换**：默认以极浅灰白（`#f8fafc`、`#ffffff`）搭配天青蓝（`#0284c7`），支持右上角一键切换至现代化深色主题（`#020617`、`#0f172a`）。

---

## 2. 核心设计令牌 (Design Tokens)

### 2.1 颜色令牌
| 令牌标识 | 浅色模式 | 深色模式 | 语义定位 |
| :--- | :--- | :--- | :--- |
| `canvas-bg` | `#f8fafc` (slate-50) | `#020617` (slate-950) | 全局底色，柔和不刺眼 |
| `surface-card` | `#ffffff` (纯白) | `#0f172a` (slate-900) | 卡片容器、抽屉与弹窗底色 |
| `surface-subtle` | `#f1f5f9` (slate-100) | `#1e293b` (slate-800) | 输入框背景、侧边导航轨道 |
| `border-subtle` | `#e2e8f0` (slate-200) | `#334155` (slate-700) | 分割线与常规卡片边缘 |
| `text-primary` | `#0f172a` (slate-900) | `#f8fafc` (slate-50) | 标题、联系人姓名、正文文字 |
| `text-secondary` | `#475569` (slate-600) | `#94a3b8` (slate-400) | 时间戳、二级副标题 |
| `primary-accent`| `#0284c7` (sky-600) | `#38bdf8` (sky-400) | 品牌主色、主按钮、焦点高亮环 |

### 2.2 路由策略语义胶囊
| 策略模式 | 样式类名 | 视觉标记 |
| :--- | :--- | :--- |
| `自动秒回 (Auto-Reply)` | `bg-emerald-50 text-emerald-700 border-emerald-200` | 6px 翠绿微呼吸圆点 |
| `草稿待审 (Copilot)` | `bg-amber-50 text-amber-700 border-amber-200` | 6px 暖阳琥珀实心圆点 |
| `纯消息总结 (Summary)` | `bg-sky-50 text-sky-700 border-sky-200` | 6px 天青蓝实心圆点 |
| `直通静默 (Ignore)` | `bg-slate-50 text-slate-500 border-slate-200` | 6px 柔灰实心圆点 |

---

## 3. 动态流式与可观测性组件规范

### 3.1 实时推理打字机卡片 (`SummariesView.tsx`)
- **容器结构**：`border-sky-300 ring-1 ring-sky-200 bg-white dark:bg-slate-900 rounded-2xl p-5`。
- **可观测性指标胶囊**：
  - `TTFT`（首字耗时）：等宽字体展示毫秒数（`TTFT: 240ms`）。
  - `速率统计`：累计 Token 数与吞吐速率（`142 tok (38.2 t/s)`）。
- **打字机光标**：等宽内联呼吸脉冲游标块（`w-1.5 h-3.5 bg-sky-500 animate-pulse`）。

### 3.2 全链路健康指示器 (`TopHeader.tsx` & `ChainHealthDrawer.tsx`)
- 8 节点全链路状态灯：绿色（全链路就绪）、黄色（部分功能降级）、红色（核心服务断开）。

---

## 4. 排版与交互约束

- **字体体系**：现代无衬线字体栈（`Inter`, `system-ui`, `-apple-system`, `sans-serif`），时间戳、账号及可观测性指标强制使用等宽字体（`JetBrains Mono` / `ui-monospace`）。
- **微交互时长**：交互过渡严格控制在 `150ms ~ 200ms ease-out`，杜绝拖沓。
- **纯矢量图标**：全量采用 `@lucide/react` 矢量图标库，线条粗细统一为 `1.75px`。
