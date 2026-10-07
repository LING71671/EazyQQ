use std::sync::atomic::{AtomicI64, Ordering};
use tauri::{command, State};
use crate::commands::AppState;
use crate::models::{ApiResponse, ProtocolStatusDto, QuickLoginAccountDto};

static LAST_AUTO_QUICK_LOGIN_TIME: AtomicI64 = AtomicI64::new(0);

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
        Some(current) => {
            tracing::warn!(
                "account switched from {} to {}; restarting so each account keeps its own \
                 data directory",
                current,
                observed
            );
            if let Err(e) = crate::services::accounts::adopt(observed) {
                tracing::error!("could not record the new account: {} - not restarting", e);
                return;
            }
            app.restart();
        }
        None => {
            // First time the account is known: a fresh install, or data written before
            // accounts were separated. Restarting now means the very first session already
            // writes into the right directory instead of the unbound one.
            tracing::info!(
                "adopting account {} so its data is isolated from other accounts",
                observed
            );
            if let Err(e) = crate::services::accounts::adopt(observed) {
                tracing::warn!("could not record the account: {}", e);
                return;
            }
            app.restart();
        }
    }
}

#[command]
pub async fn get_protocol_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ProtocolStatusDto>, String> {
    // 1. First probe OneBot HTTP for active session
    if let Ok(info) = state.onebot.get_login_info().await {
        if let Some(data) = info.get("data") {
            let uin = data.get("user_id").and_then(|v| v.as_i64()).map(|n| n.to_string());
            let nickname = data.get("nickname").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(observed) = &uin {
                reconcile_account(&app, observed);
            }
            if uin.is_some() {
                let avatar_url = qq_avatar(uin.as_ref());
                return Ok(ApiResponse::ok(ProtocolStatusDto {
                    is_connected: true,
                    login_status: "logged_in".to_string(),
                    qrcode_base64: None,
                    qrcode_error: None,
                    qq_number: uin,
                    nickname,
                    avatar_url,
                    quick_login_accounts: None,
                }));
            }
        }
    }

    // 2. Check NapCat WebUI login status
    let is_alive = state.napcat.is_alive().await;
    if !is_alive {
        return Ok(ApiResponse::ok(ProtocolStatusDto {
            is_connected: false,
            login_status: "unlogged".to_string(),
            qrcode_base64: None,
            qrcode_error: Some(
                "NapCat 协议端启动加载中，正在准备本地运行环境...".to_string(),
            ),
            qq_number: None,
            nickname: None,
            avatar_url: None,
            quick_login_accounts: None,
        }));
    }

    // 3. Probe available remembered quick-login accounts
    let mut quick_login_accounts: Vec<QuickLoginAccountDto> = Vec::new();
    if let Ok(res) = state.napcat.get_quick_login_list().await {
        if let Some(arr) = res.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(u) = item.get("uin").and_then(|v| v.as_str()) {
                    let nick = item.get("nickName").and_then(|v| v.as_str()).unwrap_or(u);
                    let face = item.get("faceUrl").and_then(|v| v.as_str()).map(|s| s.to_string())
                        .or_else(|| qq_avatar(Some(&u.to_string())));
                    quick_login_accounts.push(QuickLoginAccountDto {
                        uin: u.to_string(),
                        nickname: nick.to_string(),
                        face_url: face,
                    });
                }
            }
        }
    }

    if let Ok(status) = state.napcat.check_login().await {
        if let Some(data) = status.get("data") {
            let is_login = data.get("isLogin").and_then(|v| v.as_bool()).unwrap_or(false);
            let uin = data.get("uin").and_then(|v| v.as_str()).map(|s| s.to_string());
            if is_login {
                let avatar_url = qq_avatar(uin.as_ref());
                return Ok(ApiResponse::ok(ProtocolStatusDto {
                    is_connected: true,
                    login_status: "logged_in".to_string(),
                    qrcode_base64: None,
                    qrcode_error: None,
                    qq_number: uin,
                    nickname: None,
                    avatar_url,
                    quick_login_accounts: if quick_login_accounts.is_empty() { None } else { Some(quick_login_accounts) },
                }));
            }
        }
    }

    // 4. Auto-trigger quick login at most ONCE upon startup if remembered accounts exist
    if !quick_login_accounts.is_empty() {
        let last_attempt = LAST_AUTO_QUICK_LOGIN_TIME.load(Ordering::Relaxed);

        if last_attempt == 0 {
            let now_sec = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            LAST_AUTO_QUICK_LOGIN_TIME.store(now_sec, Ordering::Relaxed);

            let last_acc = crate::services::accounts::read_bootstrap().last_account;
            let candidate = last_acc
                .as_deref()
                .and_then(|target| quick_login_accounts.iter().find(|a| a.uin == target))
                .unwrap_or(&quick_login_accounts[0]);

            tracing::info!(
                "get_protocol_status: initial attempt quick login for {} ({})",
                candidate.nickname,
                candidate.uin
            );
            if let Ok(ql_res) = state.napcat.set_quick_login(&candidate.uin).await {
                if ql_res.get("code").and_then(|c| c.as_i64()) == Some(0) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    if let Ok(info) = state.onebot.get_login_info().await {
                        if let Some(data) = info.get("data") {
                            let uin = data.get("user_id").and_then(|v| v.as_i64()).map(|n| n.to_string());
                            let nickname = data.get("nickname").and_then(|v| v.as_str()).map(|s| s.to_string());
                            if let Some(observed) = &uin {
                                reconcile_account(&app, observed);
                            }
                            if uin.is_some() {
                                let avatar_url = qq_avatar(uin.as_ref());
                                return Ok(ApiResponse::ok(ProtocolStatusDto {
                                    is_connected: true,
                                    login_status: "logged_in".to_string(),
                                    qrcode_base64: None,
                                    qrcode_error: None,
                                    qq_number: uin,
                                    nickname,
                                    avatar_url,
                                    quick_login_accounts: Some(quick_login_accounts),
                                }));
                            }
                        }
                    }
                }
            }
            tracing::info!("initial quick login attempt did not immediately succeed; proceeding with QR code");
        }
    }

    let (qrcode_base64, qrcode_error) = match state.napcat.get_qrcode().await {
        Ok(qr) => (Some(qr), None),
        Err(e) => match state.napcat.refresh_qrcode().await {
            Ok(qr) => (Some(qr), None),
            Err(_) => (
                None,
                Some(format!("NapCat 协议端启动加载中，正在准备登录二维码与凭据... ({})", e)),
            ),
        },
    };

    Ok(ApiResponse::ok(ProtocolStatusDto {
        is_connected: true,
        login_status: "waiting_scan".to_string(),
        qrcode_base64,
        qrcode_error,
        qq_number: None,
        nickname: None,
        avatar_url: None,
        quick_login_accounts: if quick_login_accounts.is_empty() { None } else { Some(quick_login_accounts) },
    }))
}

#[command]
pub async fn quick_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    uin: String,
) -> Result<ApiResponse<()>, String> {
    tracing::info!("quick_login requested for {}", uin);
    let res = state.napcat.set_quick_login(&uin).await.map_err(|e| e.to_string())?;
    let code = res.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = res.get("message").and_then(|m| m.as_str()).unwrap_or("快速登录失败");
        return Ok(ApiResponse::err(1002, msg, Some("请确认手机 QQ 或该账号仍有效".to_string())));
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    reconcile_account(&app, &uin);
    Ok(ApiResponse::ok(()))
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
                    let face = item.get("faceUrl").and_then(|v| v.as_str()).map(|s| s.to_string())
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
pub async fn refresh_qrcode(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    // 1. Make sure the protocol side is up, and keep the reason if it will not start.
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
pub async fn logout() -> Result<ApiResponse<()>, String> {
    tracing::info!("logout requested: terminating QQ and resetting active account session");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut kill_cmd = std::process::Command::new("taskkill");
        kill_cmd.args(["/F", "/IM", "QQ.exe"]);
        kill_cmd.creation_flags(0x08000000);
        let _ = kill_cmd.output();
    }
    let _ = crate::services::accounts::clear_active();
    LAST_AUTO_QUICK_LOGIN_TIME.store(i64::MAX, Ordering::Relaxed);
    crate::services::chain::record_unknown(
        crate::services::chain::Link::QqLogin,
        "已主动退出登录，等待重新扫码或选择账号",
    );
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn restart_napcat() -> Result<ApiResponse<crate::services::napcat_boot::BootOutcome>, String> {
    let root = crate::services::logging::workspace_root();
    let napcat_dir = root.join("napcat");
    let outcome = crate::services::napcat_boot::restart(&napcat_dir);
    Ok(ApiResponse::ok(outcome))
}
