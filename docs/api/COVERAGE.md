# 桌面接口与 CLI 覆盖矩阵

根据已注册的桌面处理函数和 CLI 路由生成。修改任一适配层后运行 `pnpm contracts:check`；有意变更接口时运行 `pnpm contracts:write` 更新本文。

| 桌面 IPC | CLI 对应操作 |
| --- | --- |
| `account_qrcode` | `accounts qr` |
| `app_close_window` | `window close` |
| `app_get_window_behavior` | `config` |
| `app_minimize_window` | `window minimize` |
| `app_show_window` | `window show` |
| `app_start_drag_window` | `window drag` |
| `app_toggle_maximize_window` | `window toggle-maximize` |
| `batch_accounts` | `accounts start|stop|login` |
| `batch_update_mode` | `batch-mode` |
| `check_app_update` | `updates check-app` |
| `check_dependencies` | `health` |
| `check_napcat_update` | `updates check-napcat` |
| `configure_account` | `accounts configure` |
| `delete_summary` | `summary-delete` |
| `dismiss_draft` | `draft-dismiss` |
| `download_file` | `file-download` |
| `export_diagnostics_bundle` | `export` |
| `fetch_provider_models` | `ai-models` |
| `forget_account` | `accounts forget` |
| `generate_summary` | `summarize` |
| `generate_summary_stream` | `summarize --stream` |
| `get_account_status` | `accounts status` |
| `get_chain_status` | `chain-status` |
| `get_config` | `config` |
| `get_contacts` | `contacts` |
| `get_group_files` | `files` |
| `get_messages` | `history` |
| `get_napcat_version` | `updates versions` |
| `get_pending_drafts` | `drafts` |
| `get_protocol_status` | `status` |
| `get_qq_path` | `qq-path` |
| `get_quick_login_accounts` | `quick-login-list` |
| `get_summary_history` | `summaries` |
| `list_accounts` | `accounts list` |
| `logout` | `logout` |
| `mark_read` | `mark-read` |
| `open_folder` | `folder` |
| `quick_login` | `quick-login` |
| `refresh_qrcode` | `qr --refresh` |
| `regenerate_draft` | `draft-regenerate` |
| `register_account` | `accounts add` |
| `repair_chain` | `repair` |
| `restart_napcat` | `restart` |
| `send_draft` | `draft-send` |
| `send_message` | `send` |
| `set_qq_path` | `qq-path --set` |
| `summarize_file` | `file-summarize` |
| `test_ai_connection` | `ai-test` |
| `trigger_ai_reply` | `ask` |
| `update_config` | `set-config` |
| `update_rule` | `rule` |
| `upgrade_app` | `updates install-app` |
| `upgrade_napcat` | `updates install-napcat` |
