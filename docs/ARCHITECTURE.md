# EazyQQ Architecture Specification

## 1. System Overview

EazyQQ is structured as a high-performance, local-first dual-mode application:
- **Desktop Application**: Powered by Tauri 2.0 (Rust) and React 19 (TypeScript, Vite, TailwindCSS v4).
- **Headless Agent Engine**: Built as a standalone binary (`eazyqq_cli`) exposing both standard subcommands and a JSON-RPC 2.0 stdio Model Context Protocol (MCP) server.

```text
+---------------------------------------------------------------------------------+
|                                External World                                   |
|   +--------------------------+   +------------------------------------------+   |
|   |  Desktop User (GUI)      |   |  External AI Agents (Cursor, Claude, AG) |   |
|   +------------+-------------+   +--------------------+---------------------+   |
+----------------|--------------------------------------|-------------------------+
                 | (Tauri IPC invoke/listen)            | (stdio JSON-RPC 2.0 / CLI)
+----------------v--------------------------------------v-------------------------+
|                                 Host Process                                    |
|   +------------------------------------+  +---------------------------------+   |
|   | Tauri Commands & Events            |  | eazyqq_cli & MCP Engine         |   |
|   | (system, protocol, chat, ai)       |  | (9 MCP Tools, 21 CLI Commands)  |   |
|   +-----------------+------------------+  +----------------+----------------+   |
|                     \                                     /                     |
|                      v                                   v                      |
|   +-------------------------------------------------------------------------+   |
|   |                        Domain Services Layer                            |   |
|   |  - protocol  : OneBot 11 WS client, NapCat supervisor, heartbeat        |   |
|   |  - security  : Default-deny routing engine, cooldown & filter checks    |   |
|   |  - ai        : Multi-provider client, SSE streaming parser              |   |
|   |  - workflows : Summarizer engine, file synchronization, doc extractors  |   |
|   |  - identity  : Machine ID derivation, multi-account isolation           |   |
|   |  - infra     : Tracing logger, redact filters, diagnostic packager      |   |
|   +--------------------+------------------------------+---------------------+   |
|                        |                              |                         |
|                        v                              v                         |
|   +-----------------------------+   +---------------------------------------+   |
|   |  Storage Layer (SQLite WAL) |   |  NapCat OneBot 11 Protocol Subprocess |   |
|   |  - rules, messages, drafts  |   |  - Native NTQQ injection hook         |   |
|   |  - summaries, settings      |   |  - WS :3001, HTTP :3000, WebUI :6099  |   |
|   +-----------------------------+   +---------------------------------------+   |
+---------------------------------------------------------------------------------+
```

---

## 2. Core Service Components

### 2.1 Protocol Layer (`src-tauri/src/services/protocol/`)
- **NapCat Supervisor**: Manages child process lifecycle of `NapCatWinBootMain.exe`, handles automated restarts, registry/path discovery for QQNT, and rate-limited relaunch protections.
- **WebSocket Event Bus**: Maintains persistent loopback WebSocket connection to `ws://127.0.0.1:3001` with exponential backoff reconnection.
- **Heartbeat & Health Checker**: Probes NapCat control plane, OneBot HTTP, WebSocket connection, and login status every 15 seconds.

### 2.2 Security & Routing Engine (`src-tauri/src/services/security/`)
- **Strict Default-Deny Policy**: Contacts and groups are ignored by default. Messages are only logged or passed to AI if explicitly whitelisted in `contact_rules`.
- **Mode Execution**:
  - `Auto-Reply`: Generates and dispatches response immediately when triggers match.
  - `Copilot`: Generates structured draft for human review with confidence score and reasoning trace.
  - `Summary-Only`: Records message streams silently for scheduled or on-demand summaries.
  - `Ignore`: Completely bypassed with zero storage or AI overhead.

### 2.3 Storage Layer (`src-tauri/src/services/storage/`)
- **Engine**: SQLite via `rusqlite` with strict performance PRAGMAs:
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA foreign_keys = ON;
  PRAGMA cache_size = -64000;
  PRAGMA busy_timeout = 5000;
  ```
- **Automated Schema Migrations**: Tables `contact_rules`, `messages`, `pending_drafts`, `group_files`, `group_summaries`, and `app_settings` are versioned and migrated automatically on boot.

### 2.4 Multi-Account & Machine Isolation (`src-tauri/src/services/identity/`)
Data is stored strictly in `EazyQQ_Data/` following per-machine and per-account scoping:
```text
EazyQQ_Data/
├── bootstrap.json                     # App-level state (last active account)
└── accounts/
    └── <machine_id>/                  # Machine hash derived from hardware identifiers
        ├── _unbound/                  # Temporary staging prior to QR code authentication
        └── <qq_uin>/                  # Concrete account data sandbox
            ├── eazyqq.db              # SQLite database for this account
            ├── logs/                  # Rotated tracing log files
            ├── files/                 # Downloaded group file assets
            └── diagnostics/           # Exported diagnostic zip archives
```

---

## 3. Headless CLI & Model Context Protocol (MCP)

### 3.1 CLI Architecture (`src-tauri/src/bin/eazyqq_cli/`)
- Provides 21 subcommands covering the full spectrum of application functionality: authentication, messaging, rules, summaries, files, configs, and diagnostics.
- Supports `--json` flag on all inspection commands for scriptability.
- Machine introspection via `eazyqq_cli schema --json` outputs complete JSON Schema declarations for all CLI commands and MCP tools.

### 3.2 MCP Server (`eazyqq_cli mcp`)
- Standard JSON-RPC 2.0 stdio server compliant with Anthropic Model Context Protocol specification.
- Exposes 9 core tools covering internal operations:
  - `send_message`: Dispatch direct or group messages.
  - `get_chat_history`: Fetch chronological context with limit filtering.
  - `list_contacts`: Inspect all friends, groups, and active routing modes.
  - `trigger_group_summary`: Trigger sliding-window summarization for any group.
  - `extract_document`: Extract raw text from local PDF, DOCX, TXT, or Markdown files.
  - `simulate_rule_match`: Test rule evaluations against arbitrary message payloads.
  - `update_target_rule`: Reconfigure routing modes, triggers, and prompt presets.
  - `get_system_health`: Survey dependency health and port availability.
  - `restart_napcat`: Restart protocol subprocess.
- **Stdout Isolation**: Tracing and diagnostic logging are redirected away from `stdout` during MCP mode, guaranteeing clean JSON-RPC frame delivery.
