use crate::commands::AppState;
use crate::models::{ApiResponse, ProtocolStatusDto, QuickLoginAccountDto};
use tauri::{command, State};

/// QQ exposes avatars by account number; NapCat's login payload does not include one, so
/// derive it here to satisfy the Phase 1 "avatar + nickname echo" acceptance item.
pub fn qq_avatar(uin: Option<&String>) -> Option<String> {
    uin.filter(|s| !s.is_empty())
        .map(|s| format!("https://q1.qlogo.cn/g?b=qq&nk={}&s=100", s))
}

/// Keep this process's data directory aligned with the account actually signed in.
///
/// Every private path - database, log file, downloaded group files, diagnostics - is
/// computed once at startup from the recorded account. If the protocol side reports a
/// different one, carrying on would write this account's messages, drafts and logs into
/// the previous account's directory, which is exactly the cross-account leak we must not
/// have. So record the account and restart, which recomputes every path.
///
/// The restart happens once: after it, the recorded account matches the observed one.
pub fn reconcile_account(app: &tauri::AppHandle, observed: &str) {
    let observed = observed.trim();
    if observed.is_empty() {
        return;
    }

    match crate::services::accounts::active() {
        Some(current) if current == observed => {}
        _ => {
            tracing::info!(
                "adopting account {} so its data is isolated from other accounts (hot reloading webview)",
                observed
            );
            if let Err(e) = crate::services::accounts::adopt(observed) {
                tracing::warn!("could not record the account: {}", e);
                return;
            }
            // Restart the application context only after authentication is confirmed.
            // Immutable per-process database/AI/workflow handles prevent in-flight work
            // from crossing account boundaries during a selection change.
            std::env::set_var("EAZYQQ_ACCOUNT_RESTART", "1");
            app.restart();
        }
    }
}

#[command]
pub async fn get_protocol_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ProtocolStatusDto>, String> {
    let evidence = crate::services::protocol::session::probe(&state.napcat, &state.onebot).await;
    if evidence.logged_in {
        crate::services::chain::record_ok(
            crate::services::chain::Link::QqLogin,
            format!("Login confirmed by {}", evidence.source),
        );
        if let Some(uin) = &evidence.uin {
            let _ = crate::services::protocol::accounts::remember_observed(
                std::path::Path::new(state.napcat.napcat_dir()),
                uin,
                evidence.nickname.as_deref(),
            );
            reconcile_account(&app, uin);
        }
    }
    let connected = evidence.logged_in || state.napcat.is_alive().await;
    let mut quick_accounts = Vec::new();
    if let Ok(res) = state.napcat.get_quick_login_list().await {
        if let Some(arr) = res["data"].as_array() {
            for item in arr {
                if let Some(uin) = crate::services::protocol::session::account_id(&item["uin"]) {
                    quick_accounts.push(QuickLoginAccountDto {
                        nickname: item["nickName"].as_str().unwrap_or(&uin).into(),
                        face_url: qq_avatar(Some(&uin)),
                        uin,
                    });
                }
            }
        }
    }
    let (qr, error) = if !evidence.logged_in && connected {
        match state.napcat.get_qrcode().await {
            Ok(qr) => (Some(qr), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    Ok(ApiResponse::ok(ProtocolStatusDto {
        is_connected: connected,
        login_status: if evidence.logged_in {
            "logged_in"
        } else if connected {
            "waiting_scan"
        } else {
            "unlogged"
        }
        .into(),
        avatar_url: qq_avatar(evidence.uin.as_ref()),
        qq_number: evidence.uin,
        nickname: evidence.nickname,
        qrcode_base64: qr,
        qrcode_error: error,
        quick_login_accounts: Some(quick_accounts),
    }))
}

#[command]
pub async fn quick_login(app: tauri::AppHandle, uin: String) -> Result<ApiResponse<()>, String> {
    let source = crate::services::napcat_boot::locate_napcat_dir();
    match crate::services::protocol::accounts::login(&source, &uin).await {
        Ok(_) => {
            reconcile_account(&app, &uin);
            Ok(ApiResponse::ok(()))
        }
        Err(e) => {
            let code = if e.starts_with("QR_REQUIRED:") {
                1005
            } else {
                1002
            };
            Ok(ApiResponse::err(
                code,
                e.trim_start_matches("QR_REQUIRED: "),
                Some(
                    "Open the selected account's QR login; the current account is preserved".into(),
                ),
            ))
        }
    }
}

#[command]
pub async fn get_quick_login_accounts(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<QuickLoginAccountDto>>, String> {
    let mut list = Vec::new();
    if let Ok(res) = state.napcat.get_quick_login_list().await {
        if let Some(arr) = res.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(u) = item.get("uin").and_then(|v| v.as_str()) {
                    let nick = item.get("nickName").and_then(|v| v.as_str()).unwrap_or(u);
                    let face = item
                        .get("faceUrl")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| qq_avatar(Some(&u.to_string())));
                    list.push(QuickLoginAccountDto {
                        uin: u.to_string(),
                        nickname: nick.to_string(),
                        face_url: face,
                    });
                }
            }
        }
    }
    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn refresh_qrcode(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    // 1. Make sure the protocol side is up, and keep the reason if it will not start.
    crate::services::protocol::boot::set_auto_start(true);
    let mut launch_error: Option<String> = None;
    if !state.napcat.is_alive().await {
        if let Err(e) = state.napcat.launch_if_needed() {
            launch_error = Some(e);
        }
        // Wait up to 5 seconds for the WebUI to start listening.
        for _ in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if state.napcat.is_alive().await {
                break;
            }
        }
    }

    // 2. Ask NapCat for a genuinely new code.
    //
    // A failure is reported, not papered over. This used to fall back to `get_qrcode()`,
    // which returns whatever code NapCat last issued - so pressing "refresh" handed back the
    // same image while the UI announced "最新有效二维码已就绪" for a code that was already
    // stale, and the real reason was discarded with `Err(_)`. The caller keeps showing the
    // previous image; what it must not do is claim the code is fresh when it is not.
    match state.napcat.refresh_qrcode().await {
        Ok(base64_str) => Ok(ApiResponse::ok(serde_json::json!({
            "qrcodeBase64": base64_str,
            "expiresInSeconds": 120
        }))),
        Err(e) => {
            let mut reason = format!("无法获取全新二维码: {}", e);
            if let Some(note) = launch_error {
                reason.push_str(&format!("；协议端未能启动: {}", note));
            }
            Ok(ApiResponse::err(
                1001,
                reason,
                Some("请确认 NapCat 正在运行，且 QQ 安装路径正确".to_string()),
            ))
        }
    }
}

#[command]
pub async fn logout(state: State<'_, AppState>) -> Result<ApiResponse<()>, String> {
    tracing::info!("logout requested: stopping NapCat protocol session");
    crate::services::protocol::boot::set_auto_start(false);
    let outcome = crate::services::protocol::boot::stop();
    if !outcome.ok || (!outcome.attempted && state.napcat.is_alive().await) {
        return Ok(ApiResponse::err(
            1003,
            "Cannot stop an unowned protocol session; close it from QQ or NapCat",
            None,
        ));
    }
    crate::services::chain::record_unknown(
        crate::services::chain::Link::QqLogin,
        "已主动退出登录，等待重新扫码或选择账号",
    );
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn restart_napcat(
) -> Result<ApiResponse<crate::services::napcat_boot::BootOutcome>, String> {
    let napcat_dir = crate::services::napcat_boot::locate_napcat_dir();
    crate::services::protocol::boot::set_auto_start(true);
    let (dir, _, _, _) = crate::services::protocol::layout::selected(&napcat_dir);
    let outcome = tokio::task::spawn_blocking(move || crate::services::napcat_boot::restart(&dir))
        .await
        .map_err(|e| e.to_string())?;
    if !outcome.ok {
        return Ok(ApiResponse::err(1004, outcome.detail, None));
    }
    Ok(ApiResponse::ok(outcome))
}
