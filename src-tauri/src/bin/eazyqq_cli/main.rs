//! `eazyqq-cli` - headless control channel for EazyQQ.
//!
//! Everything the GUI can do is reachable from here, so the backend can be driven and
//! regression-tested on a real QQ account without touching the UI (the GUI itself can
//! be blocked by machine-level WebView2 problems, see ISSUE-013).
//!
//! Usage: `cargo run --bin eazyqq-cli -- <command> [options]`
//! Run without arguments to print the full command list.

mod args;
mod commands;
mod context;
mod format;

use std::process::ExitCode;
use std::sync::Arc;

use args::Args;
use context::Services;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();

    if args.command.is_empty() || args.command == "help" || args.has("help") {
        commands::help::cmd_help();
        return ExitCode::SUCCESS;
    }

    if args.command == "version" {
        println!("eazyqq-cli {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Resolve the account before initialising logging: the log file is account-scoped
    // (it contains message text), so initialising first would write into the wrong
    // account's directory - or the unbound one.
    {
        let bootstrap = eazyqq_lib::services::accounts::read_bootstrap();
        eazyqq_lib::services::accounts::set_active(bootstrap.last_account.clone());
    }

    if args.command == "schema" {
        return match commands::schema::cmd_schema(&args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("错误: {}", e);
                ExitCode::FAILURE
            }
        };
    }

    let is_mcp = args.command == "mcp";
    eazyqq_lib::services::logging::init(!is_mcp);

    let result = match args.command.as_str() {
        "mcp" => {
            let svc = match Services::build() {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    eprintln!("初始化失败: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            commands::mcp::run_mcp_server(svc).await
        }
        "log-path" => commands::diagnostics::cmd_log_path(),
        "log-tail" => commands::diagnostics::cmd_log_tail(&args),
        "stop" => commands::lifecycle::cmd_stop(&args),
        _ => {
            let svc = match Services::build() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("初始化失败: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            match args.command.as_str() {
                "start" => commands::lifecycle::cmd_start(&svc, &args).await,
                "stop" => commands::lifecycle::cmd_stop(&args),
                "restart" => commands::lifecycle::cmd_restart(&svc, &args).await,
                "instances" => commands::instances::cmd_instances(&svc, &args).await,
                "status" => commands::protocol::cmd_status(&svc, &args).await,
                "login-info" => commands::protocol::cmd_login_info(&svc, &args).await,
                "quick-login-list" => commands::protocol::cmd_quick_login_list(&svc, &args).await,
                "quick-login" => commands::protocol::cmd_quick_login(&svc, &args).await,
                "qr" => commands::protocol::cmd_qr(&svc, &args).await,
                "contacts" => commands::contacts::cmd_contacts(&svc, &args).await,
                "groups" => commands::contacts::cmd_raw_roster(&svc, &args, true).await,
                "friends" => commands::contacts::cmd_raw_roster(&svc, &args, false).await,
                "rule" => commands::contacts::cmd_rule(&svc, &args).await,
                "send" => commands::chat::cmd_send(&svc, &args).await,
                "ask" => commands::chat::cmd_ask(&svc, &args).await,
                "drafts" => commands::chat::cmd_drafts(&svc, &args).await,
                "draft-send" => commands::chat::cmd_draft_send(&svc, &args).await,
                "draft-dismiss" => commands::chat::cmd_draft_dismiss(&svc, &args),
                "draft-regenerate" => commands::chat::cmd_draft_regenerate(&svc, &args).await,
                "history" => commands::chat::cmd_history(&svc, &args).await,
                "mark-read" => commands::chat::cmd_mark_read(&svc, &args),
                "files" => commands::files::cmd_files(&svc, &args).await,
                "file-download" => commands::files::cmd_file_download(&svc, &args).await,
                "file-summarize" => commands::files::cmd_file_summarize(&svc, &args).await,
                "summaries" => commands::summary::cmd_summaries(&svc, &args).await,
                "summarize" => commands::summary::cmd_summarize(&svc, &args).await,
                "scheduler-tick" => commands::summary::cmd_scheduler_tick(&svc, &args).await,
                "whitelist-groups" => commands::summary::cmd_whitelist_groups(&svc, &args).await,
                "config" => commands::config::cmd_config(&svc, &args).await,
                "set-config" => commands::config::cmd_set_config(&svc, &args).await,
                "health" => commands::diagnostics::cmd_health(&svc, &args).await,
                "chain-status" => commands::diagnostics::cmd_chain_status(&svc, &args).await,
                "config-audit" => commands::config::cmd_config_audit(&args),
                "selftest" => commands::diagnostics::cmd_selftest(&svc, &args).await,
                "napcat-doctor" => commands::diagnostics::cmd_napcat_doctor(&args),
                "ai-config" => commands::ai::cmd_ai_config(&svc, &args),
                "ai-set" => commands::ai::cmd_ai_set(&svc, &args),
                "ai-test" => commands::ai::cmd_ai_test(&svc, &args).await,
                "ai-detect" => commands::ai::cmd_ai_detect(&svc, &args).await,
                "export" => commands::diagnostics::cmd_export(&svc, &args).await,
                "simulate" => commands::diagnostics::cmd_simulate(&svc, &args).await,
                other => {
                    eprintln!("未知命令: {}\n", other);
                    commands::help::cmd_help();
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误: {}", e);
            ExitCode::FAILURE
        }
    }
}
