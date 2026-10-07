# EazyQQ Product Specification

<!-- impeccable:product-schema 1 -->

## Platform
Windows Desktop (Tauri 2.0 + WebView2) & Headless Command-Line Interface (`eazyqq_cli`).

## Tech Stack
Tauri 2.0 (Rust) + Vite 6 + React 19 + TypeScript + TailwindCSS v4 + rusqlite (WAL mode) + OneBot 11 (NapCat NTQQ Core) + Model Context Protocol (JSON-RPC 2.0 stdio).

## Target Users
- **Everyday Users & Community Operators**: Need an automated, secure, local-first assistant for QQ groups and friends without dealing with command lines, Python environments, or sign servers.
- **AI Developers & External Agents**: Need programmatic, standard MCP/CLI control over all internal QQ messaging, group history, summaries, file extraction, and diagnostics.

## Product Purpose
Deliver an out-of-the-box, zero-barrier, premium personal QQ intelligent assistant. EazyQQ seamlessly bridges personal QQ interactions with modern LLMs (DeepSeek, OpenAI, Claude, Ollama, LM Studio, vLLM) with granular rule controls, human-in-the-loop draft reviews, streaming summaries, and external agent orchestration.

## Positioning & Core Modes
EazyQQ operates in dual mode:
1. **Interactive Desktop GUI**: Modern, light/dark premium interface for scanning QR codes, managing routing rules, reviewing AI drafts, exploring group files, and reading streaming summaries.
2. **Headless Agent Engine (MCP & CLI)**: Standard JSON-RPC 2.0 stdio server (`eazyqq_cli mcp`) and CLI pipeline (`eazyqq_cli`) exposing complete superset access to all backend operations for external agents (Cursor, Claude Desktop, Antigravity).

## Operating Context
- **Runtime**: Windows 10/11 x64, silent background tray capability, multi-account and per-machine data isolation.
- **Authentication**: Native NTQQ QR code scanning with automated session persistence and quick-login renewal.
- **Security & Privacy**: Strict default-deny whitelist. All message logs, routing rules, drafts, and summaries reside solely in local SQLite databases (`PRAGMA journal_mode = WAL`).

## Capabilities & Constraints
- **Zero-Barrier QR Authentication**: Real-time Base64 QR code streaming, auto-refresh on expiration, and automatic credential persistence.
- **Four-Quadrant Routing Matrix**:
  - `Auto-Reply`: AI replies autonomously based on triggers (`@me`, keywords, or all messages).
  - `Copilot / Draft`: AI drafts proposed responses for human review, editing, and one-click dispatch.
  - `Summary-Only`: Silent background collection for selected whitelisted groups to generate periodic summaries.
  - `Ignore`: Full bypass without recording or AI intervention.
- **Streaming Summaries & Observability**:
  - Sliding time-window summarization with real-time SSE chunk streaming.
  - Telemetry observability: Time to First Token (TTFT in ms) and token generation velocity (tokens/sec).
- **Group File Knowledge Center**: Hierarchical tree explorer, background download manager, and local document extraction (PDF, DOCX, TXT, Markdown, code).
- **External AI Superpower & Introspection**:
  - `llms.txt` and `llms-full.txt` standard specification files.
  - `eazyqq_cli schema --json` dynamic CLI and MCP tool JSON Schema introspection.
  - `eazyqq_cli mcp` stdio server with clean stdout isolation for zero-corruption tool calls.

## Design Philosophy & Principles
1. **Zero-Fluff Minimalist Invariant ("非必要不添加文字")**:
   - Clean, functional canvas without marketing welcome text or decorative prompt pills.
   - UI surfaces only actionable controls, functional identifiers, and concise live statuses.
2. **Predictable Autonomy**: Users hold full sovereignty over AI actions via explicit dual whitelists and draft buffers.
3. **Local-First Reliability**: High-concurrency WAL SQLite storage, automated crash recovery, and self-healing watchdog.
