use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json};

pub async fn cmd_status(svc: &Services, args: &Args) -> Result<(), String> {
    let napcat_alive = svc.napcat.is_alive().await;
    let login_info = svc.onebot.get_login_info().await;
    let webui_status = svc.napcat.check_login().await;

    let evidence = eazyqq_lib::services::protocol::session::probe(&svc.napcat, &svc.onebot).await;
    let (logged_in, uin, nickname) = (evidence.logged_in, evidence.uin, evidence.nickname);

    if args.json() {
        print_json(&serde_json::json!({
            "napcatWebUiAlive": napcat_alive,
            "onebotReachable": login_info.is_ok(),
            "loggedIn": logged_in,
            "qqNumber": uin,
            "nickname": nickname,
            "webuiLoginStatus": webui_status.ok(),
        }));
        return Ok(());
    }

    hr();
    println!("EazyQQ 运行状态");
    hr();
    println!(
        "NapCat WebUI (6099) : {}",
        if napcat_alive { "在线" } else { "离线" }
    );
    println!(
        "OneBot HTTP (3000)  : {}",
        match &login_info {
            Ok(_) => "可达".to_string(),
            Err(e) => format!("不可达 ({})", e),
        }
    );
    println!(
        "登录状态            : {}",
        if logged_in { "已登录" } else { "未登录" }
    );
    if logged_in {
        println!("QQ 账号             : {}", uin.unwrap_or_default());
        println!("昵称                : {}", nickname.unwrap_or_default());
    }
    if !logged_in {
        println!();
        println!("提示: 未登录时可用 `eazyqq-cli qr` 获取二维码后手机 QQ 扫码。");
    }
    hr();
    Ok(())
}

pub async fn cmd_login_info(svc: &Services, args: &Args) -> Result<(), String> {
    let info = svc
        .onebot
        .get_login_info()
        .await
        .map_err(|e| format!("查询登录信息失败: {}", e))?;
    if args.json() {
        print_json(&info);
    } else {
        let data = info.get("data").cloned().unwrap_or(serde_json::json!({}));
        println!(
            "QQ 账号 : {}",
            data.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0)
        );
        println!(
            "昵称    : {}",
            data.get("nickname")
                .and_then(|v| v.as_str())
                .unwrap_or("<未知>")
        );
    }
    Ok(())
}

pub async fn cmd_qr(svc: &Services, args: &Args) -> Result<(), String> {
    let (qr, maybe_url) = if args.has("refresh") {
        (svc.napcat.refresh_qrcode().await?, None)
    } else {
        svc.napcat.get_qrcode_info().await?
    };

    if let Some(path) = args.flag("save") {
        if qr.starts_with("data:image/png;base64,") {
            let b64 = qr.trim_start_matches("data:image/png;base64,");
            if let Ok(bytes) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            {
                std::fs::write(path, &bytes).map_err(|e| format!("写入 {} 失败: {}", path, e))?;
                if !args.json() {
                    println!("二维码 PNG 图片已保存到 {}", path);
                }
            } else {
                std::fs::write(path, &qr).map_err(|e| format!("写入 {} 失败: {}", path, e))?;
                if !args.json() {
                    println!("二维码已保存到 {}", path);
                }
            }
        } else {
            std::fs::write(path, &qr).map_err(|e| format!("写入 {} 失败: {}", path, e))?;
            if !args.json() {
                println!("二维码已保存到 {}", path);
            }
        }
    }

    if args.json() {
        print_json(&serde_json::json!({
            "qrcode": qr,
            "url": maybe_url
        }));
        return Ok(());
    }

    if args.has("browser") {
        let temp_html = std::env::temp_dir().join("eazyqq_login_qr.html");
        let html_content = format!(
            r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <title>EazyQQ 扫码登录</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f8fafc; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; margin: 0; }}
    .card {{ background: white; padding: 36px; border-radius: 20px; box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.05); text-align: center; border: 1px solid #e2e8f0; width: 320px; }}
    h2 {{ margin: 0 0 8px; color: #0f172a; font-size: 18px; font-weight: 600; }}
    p {{ color: #64748b; font-size: 13px; margin: 0 0 24px; }}
    img {{ width: 220px; height: 220px; border-radius: 12px; border: 1px solid #e2e8f0; display: block; margin: 0 auto; }}
    .badge {{ display: inline-flex; align-items: center; margin-top: 20px; padding: 6px 14px; background: #f0f9ff; color: #0284c7; border-radius: 9999px; font-size: 12px; font-weight: 500; border: 1px solid #e0f2fe; }}
  </style>
</head>
<body>
  <div class="card">
    <h2>EazyQQ 手机 QQ 扫码登录</h2>
    <p>请使用手机 QQ 扫描下方二维码授权登录</p>
    <img src="{}" alt="QQ Login QR Code" />
    <div class="badge">扫码后终端将自动检测并同步状态</div>
  </div>
</body>
</html>"#,
            qr
        );
        std::fs::write(&temp_html, html_content).map_err(|e| format!("写入临时网页失败: {}", e))?;

        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", &temp_html.to_string_lossy()])
            .spawn();

        hr();
        println!("已在系统默认浏览器中打开登录页面：");
        println!("{}", temp_html.display());
        println!("请在浏览器中用手机 QQ 扫描二维码完成登录。");
        hr();
        return Ok(());
    }

    hr();
    println!("真实登录二维码 (NapCat / NTQQ 签发):");
    hr();

    let render_target =
        maybe_url
            .as_deref()
            .unwrap_or(if !qr.starts_with("data:") { &qr } else { "" });

    let mut printed_matrix = false;
    if !render_target.is_empty() {
        if let Ok(code) = qrcode::QrCode::new(render_target.as_bytes()) {
            let rendered = code
                .render::<qrcode::render::unicode::Dense1x2>()
                .dark_color(qrcode::render::unicode::Dense1x2::Light)
                .light_color(qrcode::render::unicode::Dense1x2::Dark)
                .build();
            println!("{}", rendered);
            printed_matrix = true;
        }
    }

    if !printed_matrix {
        // Fallback for raw data URI if no direct text URL
        println!(
            "(已获取 Base64 图片，使用 `eazyqq_cli qr --browser` 可直接在浏览器中弹窗大图扫码)"
        );
        println!("{}", qr);
    }

    hr();
    println!(
        "请用手机 QQ 扫码登录。若控制台显示不便，可运行 `eazyqq_cli qr --browser` 在浏览器中扫码。"
    );
    Ok(())
}

/// List accounts available for password-free quick login (Phase 1).
pub async fn cmd_quick_login_list(svc: &Services, args: &Args) -> Result<(), String> {
    let res = svc.napcat.get_quick_login_list().await?;

    if args.json() {
        print_json(&res);
        return Ok(());
    }

    let accounts = res
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    hr();
    println!("可快速登录的账号 ({} 个)", accounts.len());
    hr();
    if accounts.is_empty() {
        println!("(无) 说明 NapCat 未记住任何账号，需要扫码登录一次。");
    }
    for acc in accounts {
        println!(
            "{}  {:<16} {}",
            acc.get("uin").and_then(|v| v.as_str()).unwrap_or("-"),
            acc.get("nickName").and_then(|v| v.as_str()).unwrap_or("-"),
            acc.get("faceUrl").and_then(|v| v.as_str()).unwrap_or("")
        );
    }
    hr();
    Ok(())
}

/// Perform a password-free quick login.
pub async fn cmd_quick_login(svc: &Services, args: &Args) -> Result<(), String> {
    let uin = args.flag("uin").ok_or_else(|| {
        "缺少 --uin <QQ号>（可用 `eazyqq_cli quick-login-list` 查看）".to_string()
    })?;

    let source = eazyqq_lib::services::napcat_boot::locate_napcat_dir();
    let evidence = eazyqq_lib::services::protocol::accounts::login(&source, uin).await?;
    if args.json() {
        print_json(&serde_json::to_value(evidence).map_err(|e| e.to_string())?);
    } else {
        println!("Account {} login confirmed", uin);
    }
    let _ = svc;
    Ok(())
}
