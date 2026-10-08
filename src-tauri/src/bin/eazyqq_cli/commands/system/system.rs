use crate::{args::Args, context::Services, format::print_json};
use eazyqq_lib::services;

pub async fn cmd_repair(svc: &Services, _args: &Args) -> Result<(), String> {
    let report = services::protocol::health::repair(&svc.napcat, &svc.onebot).await;
    print_json(&serde_json::to_value(&report).map_err(|e| e.to_string())?);
    if report.ok {
        Ok(())
    } else {
        Err(report.detail)
    }
}

pub async fn cmd_models(svc: &Services, _args: &Args) -> Result<(), String> {
    let config = svc.ai.current();
    let models = services::ai::fetch_models_with_metadata(
        &config.provider,
        &config.base_url,
        Some(&config.api_key),
    )
    .await?;
    print_json(&serde_json::to_value(models).map_err(|e| e.to_string())?);
    Ok(())
}

pub async fn cmd_updates(svc: &Services, args: &Args) -> Result<(), String> {
    use eazyqq_lib::commands::system::updater;
    let sub = args
        .positional
        .first()
        .map(String::as_str)
        .unwrap_or("versions");
    let value = match sub {
        "versions" => {
            serde_json::json!({"appVersion":env!("CARGO_PKG_VERSION"), "napcatVersion":updater::current_napcat_version(&svc.onebot).await})
        }
        "check-app" => {
            serde_json::to_value(updater::check_app_update().await?).map_err(|e| e.to_string())?
        }
        "check-napcat" => {
            serde_json::to_value(updater::check_napcat_update_for(&svc.onebot).await?)
                .map_err(|e| e.to_string())?
        }
        "install-app" | "install-napcat" if !args.has("confirm") => {
            return Err(
                "Installation requires --confirm; inspect updates check-app/check-napcat first"
                    .into(),
            )
        }
        "install-app" => {
            serde_json::to_value(updater::upgrade_app(args.flag("url").map(str::to_string)).await?)
                .map_err(|e| e.to_string())?
        }
        "install-napcat" => serde_json::to_value(
            updater::upgrade_napcat(args.flag("url").map(str::to_string)).await?,
        )
        .map_err(|e| e.to_string())?,
        _ => {
            return Err(
                "Use updates versions|check-app|check-napcat|install-app|install-napcat".into(),
            )
        }
    };
    print_json(&value);
    Ok(())
}

pub fn cmd_delete_summary(svc: &Services, args: &Args) -> Result<(), String> {
    let id = args.flag("id").ok_or("Missing --id")?;
    svc.db.delete_summary(id).map_err(|e| e.to_string())?;
    print_json(&serde_json::json!({"deleted":id}));
    Ok(())
}

pub fn cmd_batch_mode(svc: &Services, args: &Args) -> Result<(), String> {
    let ids: Vec<String> = args
        .flag("targets")
        .ok_or("Missing --targets id,id")?
        .split(',')
        .map(str::to_string)
        .collect();
    let mode = args.flag("mode").ok_or("Missing --mode")?;
    if !["auto_reply", "copilot", "summary_only", "ignore"].contains(&mode) {
        return Err("Invalid rule mode".into());
    }
    let changed = svc
        .db
        .batch_update_mode(&ids, mode)
        .map_err(|e| e.to_string())?;
    print_json(&serde_json::json!({"affectedCount":changed}));
    Ok(())
}

pub fn cmd_qq_path(args: &Args) -> Result<(), String> {
    let source = services::napcat_boot::locate_napcat_dir();
    if let Some(path) = args.flag("set") {
        let path = std::path::PathBuf::from(path);
        if !path.is_file()
            || path
                .file_name()
                .is_none_or(|name| !name.to_string_lossy().eq_ignore_ascii_case("qq.exe"))
        {
            return Err("Select an existing QQ.exe".into());
        }
        std::fs::create_dir_all(source.join("config")).map_err(|e| e.to_string())?;
        std::fs::write(
            source.join("config/qq_path.txt"),
            path.to_string_lossy().as_bytes(),
        )
        .map_err(|e| e.to_string())?;
    }
    print_json(
        &serde_json::json!({"path":services::protocol::patch::configured_qq_path(&source)?.to_string_lossy()}),
    );
    Ok(())
}
