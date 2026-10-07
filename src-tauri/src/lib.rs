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

#[cfg(target_os = "windows")]
mod single_instance {
    use std::ffi::c_void;

    extern "system" {
        fn CreateMutexW(lpMutexAttributes: *mut c_void, bInitialOwner: i32, lpName: *const u16) -> *mut c_void;
        fn GetLastError() -> u32;
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
        fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
        fn SetForegroundWindow(hWnd: *mut c_void) -> i32;
    }

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const SW_RESTORE: i32 = 9;

    pub fn check_or_exit() {
        unsafe {
            let mutex_name: Vec<u16> = "Global\\EazyQQ_App_SingleInstance_Mutex\0".encode_utf16().collect();
            let handle = CreateMutexW(std::ptr::null_mut(), 0, mutex_name.as_ptr());
            if !handle.is_null() && GetLastError() == ERROR_ALREADY_EXISTS {
                let win_title: Vec<u16> = "EazyQQ - 个人专属智能助手\0".encode_utf16().collect();
                let hwnd = FindWindowW(std::ptr::null(), win_title.as_ptr());
                if !hwnd.is_null() {
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                }
                std::process::exit(0);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    services::infra::logging::load_dotenv();

    #[cfg(target_os = "windows")]
    single_instance::check_or_exit();

    // Resolve which account's data this process serves BEFORE anything touches the disk.
    //
    // Every private artefact is derived from that choice - the SQLite database, the log
    // file (which contains message text verbatim), downloaded group files, diagnostics -
    // so getting it wrong first would mean writing one account's data into another's
    // directory. This is the earliest possible point, and it needs no logging.
    let bootstrap = services::accounts::read_bootstrap();
    services::accounts::set_active(bootstrap.last_account.clone());

    // Logging next, so anything below leaves a durable trace - now inside the account's own
    // directory instead of a shared one.
    services::logging::init(true);
    services::logging::log_paths();

    let root_dir = services::logging::workspace_root();
    let data_dir = services::logging::data_dir();
    services::logging::ensure_dir(&data_dir);

    // Locate the NapCat installation: prioritize workspace, bundled resources, or dynamic sibling paths.
    let mut napcat_candidates = vec![
        root_dir.join("napcat"),
        root_dir.join("resources").join("napcat"),
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            napcat_candidates.push(exe_dir.join("resources").join("napcat"));
            napcat_candidates.push(exe_dir.join("napcat"));
        }
    }

    if let Some(parent) = root_dir.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains("napcat") {
                        napcat_candidates.push(p);
                    }
                }
            }
        }
    }

    let napcat_dir = napcat_candidates
        .iter()
        .find(|p| p.join("NapCatWinBootMain.exe").exists() || p.join("napcat.mjs").exists())
        .cloned()
        .unwrap_or_else(|| root_dir.join("napcat"));
    let napcat_dir_str = napcat_dir.to_string_lossy().to_string();
    if napcat_dir.exists() {
        tracing::info!("NapCat directory resolved to: {}", napcat_dir.display());
    } else {
        tracing::warn!(
            "NapCat directory not found (probed: {:?}) - protocol control will be unavailable until NapCat is ready",
            napcat_candidates
        );
    }

    match services::accounts::active() {
        Some(uin) => {
            tracing::info!("serving account {} (data in {})", uin, data_dir.display());
            // One-time relocation of data written before accounts were separated.
            if let Err(e) = services::accounts::migrate_legacy_if_needed(&uin) {
                tracing::warn!("account data migration failed: {}", e);
            }
            // Data written before the account was known would otherwise be stranded in
            // the unbound directory forever.
            services::accounts::migrate_unbound_if_needed(&uin);
            // NapCat keeps its own logs and cache in the shared `napcat/` directory, and
            // those logs contain message text. Sweep them into this account's directory.
            let swept = services::accounts::sweep_napcat_artifacts(&napcat_dir, &uin);
            if !swept.is_empty() {
                tracing::info!("swept shared NapCat artefacts: {}", swept.join(", "));
            }
        }
        None => tracing::warn!(
            "no QQ account recorded yet; using {} until the protocol side reports one",
            data_dir.display()
        ),
    }
    if let Err(e) = services::accounts::ensure_layout(services::accounts::active().as_deref()) {
        tracing::error!("could not prepare the account data layout: {}", e);
    }

    let db_path = data_dir.join("eazyqq.db");
    let db = Arc::new(Database::init(&db_path).unwrap_or_else(|e| {
        tracing::error!("cannot open SQLite database at {}: {}", db_path.display(), e);
        panic!("Failed to init SQLite database: {}", e)
    }));
    tracing::info!("SQLite ready at {}", db_path.display());

    // Rules written by older versions carry the old hardcoded interval, which would shadow
    // the global cadence setting forever on an upgraded install.
    services::scheduler::normalize_legacy_intervals(&db);

    // A brand-new account database inherits the settings that belong to the installation
    // (AI provider and key, tray behaviour, summary defaults) so a second QQ account does
    // not have to be configured from scratch.
    if let Some(uin) = services::accounts::active() {
        if let Some(note) = services::accounts::seed_settings_if_missing(&db, &uin) {
            tracing::info!("{}", note);
        }
    }

    // --- WebView2 compatibility mode ---------------------------------------------
    //
    // On some machines Chromium's renderer cannot start under its own sandbox: the
    // frontend never executes, no crash dump is produced, and the user sees an empty
    // window. Passing `--no-sandbox` works around it, but it disables the renderer
    // sandbox - a real security property - so it must NOT be baked in for everyone just
    // because one machine needs it.
    //
    // Instead the app always starts sandboxed, and the startup watchdog enables this
    // mode only after a startup failure has actually been observed, then restarts once.
    // A healthy machine never pays the cost; an affected one recovers by itself.
    // The flag lives in bootstrap.json rather than the database: it has to be read before
    // the window exists, and the database is per account while this setting is app-wide.
    let webview_compat_mode = bootstrap.webview_compat_mode;

    if webview_compat_mode {
        tracing::warn!(
            "WebView compatibility mode is ON: adding --no-sandbox to the WebView2 command \
             line because the renderer previously failed to start under its sandbox. This \
             reduces security; once the underlying cause is fixed, clear the \
             `webviewCompatMode` field in {} to restore the sandbox.",
            services::accounts::bootstrap_path().display()
        );
        // WebView2 reads this when its environment is created, which happens when the
        // Tauri builder below creates the window - so it has to be set before that.
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--no-sandbox");
    }

    // All runtime components are self-contained in the current workspace

    // The WebUI token is read from `napcat/config/webui.json` by the service. No built-in
    // fallback is supplied on purpose: the token is generated per installation, so a
    // hardcoded one would both fail on any other machine and ship a credential in the
    // source tree.
    let napcat = Arc::new(NapCatService::new(
        "http://127.0.0.1:6099".to_string(),
        String::new(),
        napcat_dir_str,
    ));
    let onebot = Arc::new(OneBotClient::new("http://127.0.0.1:3000".to_string()));
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .or_else(|_| std::env::var("AI_API_KEY"))
        .ok()
        .or_else(|| db.get_setting("ai_api_key").ok().flatten())
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

    // The OneBot WebSocket port comes from `napcat.wsPort`; 3001 is NapCat's default. Read
    // from config rather than hardcoded so a non-default setup works without a rebuild.
    let onebot_ws_port = {
        let raw = db.get_setting("app_config").ok().flatten();
        raw.as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| {
                v.get("napcat")
                    .and_then(|n| n.get("wsPort"))
                    .and_then(|p| p.as_u64())
            })
            .filter(|p| *p > 0 && *p < 65536)
            .unwrap_or(3001)
    };
    tracing::info!("OneBot WebSocket port -> {}", onebot_ws_port);

    let app_state = AppState {
        db: db.clone(),
        // Cloned rather than moved so the chain monitor can keep probing it.
        napcat: napcat.clone(),
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
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                tracing::info!("main window shown and focused during setup");
            } else {
                tracing::error!("main window handle not found during setup");
            }

            // Safety net with a self-heal attempt.
            //
            // The failure mode this guards against is subtle: when the WebView cannot
            // execute JS, the frontend never mounts, `app_show_window` is never called,
            // and - crucially - no crash dump is produced. The process looks perfectly
            // healthy from the outside while the user stares at nothing.
            //
            // Escalation, in order. The first step is what keeps the workaround off
            // healthy machines: compatibility mode is never shipped enabled, it is only
            // ever turned on by an observed failure on the machine that has the problem.
            {
                let handle = app.handle().clone();
                let compat_already_on = webview_compat_mode;

                tauri::async_runtime::spawn(async move {
                    let frontend_mounted = |h: &tauri::AppHandle| {
                        h.get_webview_window("main")
                            .map(|w| w.is_visible().unwrap_or(false))
                            .unwrap_or(false)
                    };

                    tokio::time::sleep(std::time::Duration::from_secs(9)).await;

                    if frontend_mounted(&handle) {
                        return;
                    }

                    // Step 1: decide whether the WebView is actually to blame before changing
                    // anything that persists.
                    //
                    // "The frontend never mounted" has two very different causes, and the
                    // remedy for one of them permanently weakens the process. In a packaged
                    // build the assets are embedded, so failing to mount really does mean the
                    // WebView could not run them. In a development build the page is fetched
                    // over the network first - when the dev server was bound to IPv6 only and
                    // WebView2 dialled IPv4, the page never arrived at all, and blaming the
                    // WebView would have turned the sandbox off for good to fix a problem that
                    // had nothing to do with it.
                    let webview_is_culprit = !tauri::is_dev();

                    if !compat_already_on && webview_is_culprit {
                        tracing::warn!(
                            "frontend did not mount within 9s and the frontend assets are \
                             embedded, so the WebView could not run them. Enabling \
                             compatibility mode (adds --no-sandbox to the WebView2 command \
                             line) and restarting once. This failure produces no crash dump \
                             - it means the WebView could not execute JS, not that a process \
                             died."
                        );
                        if let Err(e) = services::accounts::set_webview_compat(true) {
                            tracing::error!(
                                "could not persist the compatibility flag: {} - the restart \
                                 will not help",
                                e
                            );
                        }
                        // Does not return: the process is replaced.
                        handle.restart();
                    }

                    if !compat_already_on {
                        // Development build: the page should have arrived from a server, so
                        // report where from instead of touching the WebView configuration. The
                        // window is shown in step 3 so the failure is visible rather than
                        // silent.
                        let dev_url = handle
                            .config()
                            .build
                            .dev_url
                            .as_ref()
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| "(devUrl is not set)".to_string());
                        tracing::error!(
                            "frontend did not mount within 9s. This is a development build, so \
                             the page is fetched from {} - check that address first; the WebView \
                             is probably fine. Compatibility mode is deliberately NOT enabled, \
                             because disabling the sandbox would not make an unreachable dev \
                             server reachable.",
                            dev_url
                        );
                    } else {
                        // Step 2: compatibility mode is already on, so reload once in case the
                        // failure was transient.
                        tracing::warn!(
                            "compatibility mode is already enabled but the frontend still has \
                             not mounted; reloading the webview once"
                        );
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.eval("window.location.reload()");
                        }

                        tokio::time::sleep(std::time::Duration::from_secs(9)).await;
                    }

                    // Step 3: never leave the app invisible.
                    if let Some(window) = handle.get_webview_window("main") {
                        if !window.is_visible().unwrap_or(false) {
                            tracing::error!(
                                "frontend still has not mounted after a reload; showing the \
                                 window anyway. Likely causes: the WebView runtime cannot run \
                                 its renderer on this machine, or the frontend could not be \
                                 fetched."
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
                format!("ws://127.0.0.1:{}", onebot_ws_port),
                db.clone(),
                // Cloned: the chain monitor also needs it.
                onebot.clone(),
                ai.clone(),
            );
            tracing::info!(
                "OneBot WebSocket listener task spawned (ws://127.0.0.1:{})",
                onebot_ws_port
            );

            // --- End-to-end chain monitor ---------------------------------------------
            // Polls the links that can be probed from outside (NapCat WebUI, QQ login,
            // OneBot HTTP, database, AI provider) and records them alongside the links that
            // report themselves (WebSocket, scheduler, frontend). Its purpose is to make a
            // break *visible and located* rather than merely absent - several failures
            // during development produced no error anywhere a human would look.
            services::chain::spawn_monitor(
                db.clone(),
                napcat.clone(),
                onebot.clone(),
                ai.clone(),
                napcat_dir.clone(),
            );
            tracing::info!("chain monitor started (link status is reported end to end)");

            // Background summarizer: honours the summary whitelist, per-group interval,
            // sliding window and custom prompt straight from SQLite.
            services::scheduler::start_summary_scheduler(handle, db, ai, onebot.clone());
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
            quick_login,
            get_quick_login_accounts,
            logout,
            get_contacts,
            get_chain_status,
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
            generate_summary_stream,
            get_summary_history,
            delete_summary,
            get_config,
            update_config,
            test_ai_connection,
            fetch_provider_models,
            check_dependencies,
            restart_napcat,
            export_diagnostics_bundle,
            app_minimize_window,
            app_toggle_maximize_window,
            app_close_window,
            app_start_drag_window,
            app_show_window,
            app_get_window_behavior,
            check_app_update,
            get_qq_path,
            set_qq_path,
            get_napcat_version,
            check_napcat_update,
            upgrade_napcat,
            upgrade_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EazyQQ application");
}
