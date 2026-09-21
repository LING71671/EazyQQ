pub mod models;
pub mod commands;
pub mod services;

use std::sync::Arc;
use commands::*;
use services::db::Database;
use services::napcat::NapCatService;
use services::onebot::OneBotClient;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let root_dir = if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd.clone()
    };

    let data_dir = root_dir.join("EazyQQ_Data");
    let _ = std::fs::create_dir_all(&data_dir);
    let db_path = data_dir.join("eazyqq.db");
    let db = Arc::new(Database::init(&db_path).expect("Failed to init SQLite database"));

    // All runtime components are self-contained in the current workspace
    let napcat_dir = if root_dir.join("napcat").exists() {
        root_dir.join("napcat")
    } else {
        std::path::PathBuf::from("B:\\EazyQQ\\napcat")
    };
    let napcat_dir_str = napcat_dir.to_string_lossy().to_string();

    let napcat = Arc::new(NapCatService::new(
        "http://127.0.0.1:6099".to_string(),
        "f7376db3d59d".to_string(),
        napcat_dir_str,
    ));
    let api_key = std::env::var("TOKENRHYTHM_API_KEY")
        .or_else(|_| std::env::var("AI_API_KEY"))
        .ok()
        .or_else(|| db.get_setting("tokenrhythm_api_key").ok().flatten())
        .unwrap_or_default();

    let ai = Arc::new(services::ai::AiService::new(
        "https://tokenrhythm.studio/v1".to_string(),
        api_key,
        "qwen3.8-flash".to_string(),
    ));

    let app_state = AppState {
        db: db.clone(),
        napcat,
        onebot: onebot.clone(),
        ai: ai.clone(),
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            services::ws_listener::start_onebot_ws_listener(
                handle,
                "ws://127.0.0.1:3001".to_string(),
                db,
                onebot,
                ai,
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_protocol_status,
            refresh_qrcode,
            logout,
            get_contacts,
            update_rule,
            batch_update_mode,
            get_messages,
            send_message,
            trigger_ai_reply,
            get_pending_drafts,
            send_draft,
            dismiss_draft,
            regenerate_draft,
            get_group_files,
            download_file,
            summarize_file,
            open_folder,
            generate_summary,
            get_summary_history,
            delete_summary,
            get_config,
            update_config,
            test_ai_connection,
            check_dependencies,
            export_diagnostics_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EazyQQ application");
}
