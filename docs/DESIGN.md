# EazyQQ UI/UX Design Specification

<!-- impeccable:design-schema 1 -->

## 1. Visual Philosophy & Invariants

EazyQQ follows the **Operate Mode** design standard:
- **Clean Functional Canvas**: Interface elements prioritize actionable information, low visual noise, and high readability.
- **Zero-Fluff Invariant ("非必要不添加文字")**:
  - **Empty States**: Strictly clean, functional canvas with minimal icon and single-line status. Forbidden to render explanatory welcome paragraphs or marketing introductory guides.
  - **No Decorative Starter Pills**: Forbidden to inject sample prompt pills, suggestion chips, or decorative starter cards unless explicitly requested.
  - **No Fluff Labels**: Keep UI to pure functional IDs, active target chips, and actionable controls only.
- **Light Default with High-Contrast Dark Mode**: Polished neutral light surfaces (`#f8fafc`, pure `#ffffff`) with seamless toggling to modern dark mode (`#0f172a`, `#1e293b`).

---

## 2. Design Tokens

### 2.1 Color Tokens
| Token | Light Mode | Dark Mode | Usage |
| :--- | :--- | :--- | :--- |
| `canvas-bg` | `#f8fafc` (slate-50) | `#020617` (slate-950) | Main background |
| `surface-card` | `#ffffff` | `#0f172a` (slate-900) | Content cards, drawers, dialogs |
| `surface-subtle` | `#f1f5f9` (slate-100) | `#1e293b` (slate-800) | Input backgrounds, sidebar rail |
| `border-subtle` | `#e2e8f0` (slate-200) | `#334155` (slate-700) | Borders, list separators |
| `text-primary` | `#0f172a` (slate-900) | `#f8fafc` (slate-50) | Headings, active contact names |
| `text-secondary` | `#475569` (slate-600) | `#94a3b8` (slate-400) | Timestamps, subtitles |
| `primary-accent`| `#0284c7` (sky-600) | `#38bdf8` (sky-400) | Brand primary buttons, focus rings |

### 2.2 Semantic Mode Badges
| Mode | Badge Class | Visual Indicator |
| :--- | :--- | :--- |
| `Auto-Reply` | `bg-emerald-50 text-emerald-700 border-emerald-200` | Emerald 6px pulse dot |
| `Copilot / Draft` | `bg-amber-50 text-amber-700 border-amber-200` | Amber 6px solid dot |
| `Summary-Only` | `bg-sky-50 text-sky-700 border-sky-200` | Sky 6px solid dot |
| `Ignore / Silent` | `bg-slate-50 text-slate-500 border-slate-200` | Slate 6px dot |

---

## 3. Streaming & Telemetry Component Specs

### 3.1 Streaming Typewriter Card (`SummariesView.tsx`)
- **Card Shell**: `border-sky-300 ring-1 ring-sky-200 bg-white dark:bg-slate-900 rounded-2xl p-5`.
- **Telemetry Pill**:
  - `TTFT` (Time To First Token): Monospace badge displaying elapsed milliseconds (`TTFT: 240ms`).
  - `Velocity`: Token counter and throughput speed (`142 tok (38.2 t/s)`).
- **Typewriter Cursor**: Monospace inline cursor block (`w-1.5 h-3.5 bg-sky-500 animate-pulse`).

### 3.2 Health Indicator Badge (`TopHeader.tsx` & `ChainHealthDrawer.tsx`)
- Status dot reflecting live 8-node chain health (Green = All Ready, Amber = Partially Degraded, Red = Disconnected).

---

## 4. Typography & Layout Constraints

- **Font Stack**: Modern geometric sans-serif (`Inter`, `system-ui`, `-apple-system`, `sans-serif`), with `JetBrains Mono` / `ui-monospace` for timestamps, UINs, and telemetry.
- **Micro-Interactions**: Hover transitions capped at `150ms ~ 200ms ease-out`.
- **Vector Icons**: Standardized on `lucide-react` with `1.75px` stroke width.
