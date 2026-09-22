pub mod models;
pub mod commands;
pub mod services;

use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use commands::*;
use services::db::Database;
use services::napcat::NapCatService;
use services::onebot::OneBotClient;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging must come first so that everything below - including a failed SQLite
    // open or a WebView2 startup problem - leaves a durable trace on disk.
    services::logging::init(true);
    services::logging::log_paths();

    let root_dir = services::logging::workspace_root();
    let data_dir = services::logging::data_dir();
    services::logging::ensure_dir(&data_dir);

    let db_path = data_dir.join("eazyqq.db");
    let db = Arc::new(Database::init(&db_path).unwrap_or_else(|e| {
        tracing::error!("cannot open SQLite database at {}: {}", db_path.display(), e);
        panic!("Failed to init SQLite database: {}", e)
    }));
    tracing::info!("SQLite ready at {}", db_path.display());

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
    let onebot = Arc::new(OneBotClient::new("http://127.0.0.1:3000".to_string()));
    let api_key = std::env::var("TOKENRHYTHM_API_KEY")
        .or_else(|_| std::env::var("AI_API_KEY"))
        .ok()
        .or_else(|| db.get_setting("tokenrhythm_api_key").ok().flatten())
        .unwrap_or_default();

    // The AI provider comes from the persisted config, so the settings UI actually
    // controls where requests go (local OpenCode / Ollama, DeepSeek, OpenAI, ...).
    let ai_config = {
        let raw = db.get_setting("app_config").ok().flatten();
        let parsed = raw
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
        services::ai::AiRuntimeConfig::from_app_config(
            parsed.as_ref().and_then(|v| v.get("ai")),
            &api_key,
        )
    };
    tracing::info!("active AI provider -> {}", ai_config.describe());
    let ai = Arc::new(services::ai::AiService::new(ai_config));

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
            tracing::info!("Tauri setup: creating main window and system tray");

            // The window is created hidden (see tauri.conf.json) so the user never sees
            // Chromium's startup sequence: a blank surface, then a black frame while the
            // GPU process dies and is respawned, then finally the rendered UI. Instead
            // the frontend calls `frontend_ready` once React has mounted, and only then
            // does the window appear - already painted.
            if let Some(window) = app.get_webview_window("main") {
                tracing::info!(
                    "main window created hidden; waiting for the frontend to report ready"
                );
                let _ = window;
            } else {
                tracing::error!("main window handle not found during setup");
            }

            // Safety net with a self-heal attempt.
            //
            // The failure mode this guards against is subtle: when the WebView cannot
            // execute JS, the frontend never mounts, `app_show_window` is never called,
            // and - crucially - no crash dump is produced. The process looks perfectly
            // healthy from the outside while the user stares at nothing. So instead of
            // merely revealing an empty window, reload once first, then report clearly.
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(9)).await;

                    let still_hidden = match handle.get_webview_window("main") {
                        Some(w) => !w.is_visible().unwrap_or(false),
                        None => false,
                    };

                    if still_hidden {
                        tracing::warn!(
                            "frontend has not reported ready after 9s; reloading the webview \
                             once. Note: no crash dump accompanies this failure mode - it \
                             usually means the WebView cannot execute JS, not that a \
                             process died."
                        );
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.eval("window.location.reload()");
                        }

                        tokio::time::sleep(std::time::Duration::from_secs(9)).await;
                    }

                    if let Some(window) = handle.get_webview_window("main") {
                        if !window.is_visible().unwrap_or(false) {
                            tracing::error!(
                                "frontend still has not reported ready after a reload; \
                                 showing the window anyway. Likely causes, in order: the \
                                 WebView runtime cannot run its renderer on this machine \
                                 (check `additionalBrowserArgs` in tauri.conf.json), or the \
                                 dev server is unreachable."
                            );
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });
            }

            // --- System tray: keep EazyQQ resident so the OneBot listener and the
            // --- scheduled group summarizer keep running while the window is hidden.
            let show_item = MenuItem::with_id(app, "tray_show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "tray_quit", "退出 EazyQQ", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let mut tray_builder = TrayIconBuilder::with_id("eazyqq-tray")
                .tooltip("EazyQQ - 个人专属智能助手")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "tray_show" => {
                        tracing::info!("tray: show main window requested");
                        show_main_window(app);
                    }
                    "tray_quit" => {
                        tracing::info!("tray: quit requested, exiting process");
                        app.exit(0);
                    }
                    other => tracing::debug!("tray: unhandled menu id {}", other),
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        tracing::info!("tray: left click, restoring main window");
                        show_main_window(tray.app_handle());
                    }
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            } else {
                tracing::warn!("default window icon missing; tray icon will be blank");
            }

            match tray_builder.build(app) {
                Ok(_) => tracing::info!("system tray icon created (id=eazyqq-tray)"),
                Err(e) => {
                    tracing::error!("failed to create system tray icon: {}", e);
                    return Err(e.into());
                }
            }

            let handle = app.handle().clone();
            services::ws_listener::start_onebot_ws_listener(
                handle.clone(),
                "ws://127.0.0.1:3001".to_string(),
                db.clone(),
                onebot,
                ai.clone(),
            );
            tracing::info!("OneBot WebSocket listener task spawned (ws://127.0.0.1:3001)");

            // Background summarizer: honours the summary whitelist, per-group interval,
            // sliding window and custom prompt straight from SQLite.
            services::scheduler::start_summary_scheduler(handle, db, ai);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .try_state::<AppState>()
                    .map(|state| read_window_behavior(&state.db).1)
                    .unwrap_or(true);
                if close_to_tray {
                    tracing::info!("close requested: close_to_tray=true, hiding window");
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    tracing::info!("close requested: close_to_tray=false, letting the app exit");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_protocol_status,
            refresh_qrcode,
            logout,
            get_contacts,
            mark_read,
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
            app_minimize_window,
            app_toggle_maximize_window,
            app_close_window,
            app_start_drag_window,
            app_show_window,
            app_get_window_behavior,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EazyQQ application");
}
