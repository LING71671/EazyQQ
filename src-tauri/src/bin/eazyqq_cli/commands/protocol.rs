use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json};

pub async fn cmd_status(svc: &Services, args: &Args) -> Result<(), String> {
    let napcat_alive = svc.napcat.is_alive().await;
    let login_info = svc.onebot.get_login_info().await;
    let webui_status = svc.napcat.check_login().await;

    let (logged_in, uin, nickname) = match &login_info {
        Ok(v) => {
            let data = v.get("data");
            let uin = data
                .and_then(|d| d.get("user_id"))
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string());
            let nick = data
                .and_then(|d| d.get("nickname"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (uin.is_some(), uin, nick)
        }
        Err(_) => (false, None, None),
    };

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
        println!("QQ 账号 : {}", data.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0));
        println!(
            "昵称    : {}",
            data.get("nickname").and_then(|v| v.as_str()).unwrap_or("<未知>")
        );
    }
    Ok(())
}

pub async fn cmd_qr(svc: &Services, args: &Args) -> Result<(), String> {
    let qr = svc.napcat.get_qrcode().await?;
    if let Some(path) = args.flag("save") {
        std::fs::write(path, &qr).map_err(|e| format!("写入 {} 失败: {}", path, e))?;
        println!("二维码已保存到 {}", path);
    }
    if args.json() {
        print_json(&serde_json::json!({ "qrcode": qr }));
    } else {
        hr();
        println!("真实登录二维码 (NapCat / NTQQ 签发):");
        hr();
        println!("{}", qr);
        hr();
        println!("请用手机 QQ 扫码登录。");
    }
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
    let uin = args
        .flag("uin")
        .ok_or_else(|| "缺少 --uin <QQ号>（可用 `eazyqq_cli quick-login-list` 查看）".to_string())?;

    tracing::info!("quick login requested for {}", uin);
    let res = svc.napcat.set_quick_login(uin).await?;

    let code = res.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        return Err(format!(
            "快速登录失败: {}",
            res.get("message").and_then(|m| m.as_str()).unwrap_or("未知错误")
        ));
    }

    if args.json() {
        print_json(&res);
        return Ok(());
    }
    println!("已触发账号 {} 的快速登录，请稍候在 `eazyqq_cli status` 中确认登录状态。", uin);
    Ok(())
}
