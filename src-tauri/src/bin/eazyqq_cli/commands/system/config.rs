use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json};

pub async fn cmd_config(svc: &Services, args: &Args) -> Result<(), String> {
    let raw = svc.db.get_setting("app_config").ok().flatten();
    let value: serde_json::Value = raw
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(eazyqq_lib::services::config::default_app_config);

    if args.json() {
        print_json(&value);
    } else {
        hr();
        println!("当前配置 (app_settings.app_config)");
        hr();
        print_json(&value);
        hr();
    }
    Ok(())
}

pub async fn cmd_set_config(svc: &Services, args: &Args) -> Result<(), String> {
    let key = args
        .flag("key")
        .ok_or_else(|| "缺少 --key <配置键>".to_string())?;

    // `--file` is the practical path for JSON blobs, since shell quoting of a whole
    // JSON object is fragile. `--value` stays for short scalar settings.
    let value = if let Some(path) = args.flag("file") {
        let raw =
            std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败: {}", path, e))?;
        // Validate JSON so a broken config can never be persisted.
        serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| format!("{} 不是合法 JSON: {}", path, e))?;
        raw
    } else {
        args.flag("value")
            .ok_or_else(|| "缺少 --value <配置值> 或 --file <json文件>".to_string())?
            .to_string()
    };

    svc.db
        .set_setting(key, &value)
        .map_err(|e| format!("写入配置失败: {}", e))?;

    tracing::info!("config written: {} ({} bytes)", key, value.len());
    if args.json() {
        print_json(&serde_json::json!({"key":key,"bytes":value.len()}));
    } else {
        println!("已写入 {} ({} 字节)", key, value.len());
    }
    Ok(())
}

pub fn cmd_config_audit(args: &Args) -> Result<(), String> {
    let rows = eazyqq_lib::services::config::audit();
    let unread: Vec<&String> = rows
        .iter()
        .filter(|(_, consumer)| consumer.is_none())
        .map(|(key, _)| key)
        .collect();

    if args.json() {
        print_json(&serde_json::json!({
            "total": rows.len(),
            "unread": unread,
            "fields": rows.iter().map(|(k, c)| serde_json::json!({
                "key": k,
                "consumedBy": c,
            })).collect::<Vec<_>>(),
        }));
        return Ok(());
    }

    hr();
    println!("配置项消费审计（{} 个字段）", rows.len());
    hr();
    for (key, consumer) in &rows {
        match consumer {
            Some(file) => println!("  [OK]   {:<32} <- {}", key, file),
            None => println!("  [DEAD] {:<32} <- 后端从不读取", key),
        }
    }
    hr();

    if unread.is_empty() {
        println!("所有配置项都有消费点，不存在「改了没用」的设置。");
    } else {
        println!("发现 {} 个只写不读的配置项：", unread.len());
        for key in &unread {
            println!("  - {}", key);
        }
        println!();
        println!("这些字段在界面上可改但不会生效，属于「假装成功的功能」，应实现或移除。");
    }
    hr();
    Ok(())
}
