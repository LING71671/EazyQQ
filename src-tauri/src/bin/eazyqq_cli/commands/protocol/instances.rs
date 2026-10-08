use crate::{args::Args, context::Services, format::print_json};
use eazyqq_lib::services::{self, protocol::accounts};

pub async fn cmd_instances(_svc: &Services, args: &Args) -> Result<(), String> {
    let source = services::napcat_boot::locate_napcat_dir();
    let sub = args
        .positional
        .first()
        .map(String::as_str)
        .unwrap_or("list");
    match sub {
        "status" => {
            let uin = args.flag("uin").ok_or("Missing --uin")?;
            print_json(
                &serde_json::to_value(accounts::status(uin).await?).map_err(|e| e.to_string())?,
            );
            Ok(())
        }
        "list" => {
            print_json(&serde_json::to_value(accounts::list().await).map_err(|e| e.to_string())?);
            Ok(())
        }
        "add" => {
            let uin = args.flag("uin").ok_or("Missing --uin")?;
            let instance = services::instances::get_or_register_instance(uin, args.flag("nick"))?;
            print_json(
                &serde_json::to_value(accounts::AccountInfoDto::from(&instance))
                    .map_err(|e| e.to_string())?,
            );
            Ok(())
        }
        "select" => {
            let uin = args.flag("uin").ok_or("Missing --uin")?;
            let evidence = accounts::login(&source, uin).await?;
            services::accounts::adopt(uin)?;
            print_json(&serde_json::to_value(evidence).map_err(|e| e.to_string())?);
            Ok(())
        }
        "qr" => {
            let uin = args.flag("uin").ok_or("Missing --uin")?;
            let result = eazyqq_lib::commands::protocol::account_qrcode(
                uin.into(),
                Some(args.has("refresh")),
            )
            .await?;
            print_json(&serde_json::to_value(result).map_err(|e| e.to_string())?);
            Ok(())
        }
        "configure" => {
            let uin = args.flag("uin").ok_or("Missing --uin")?;
            let enabled = match args.flag("auto-start") {
                Some("true") => true,
                Some("false") => false,
                _ => return Err("Use --auto-start true|false".into()),
            };
            services::instances::configure_instance(uin, enabled)?;
            print_json(&serde_json::json!({"uin":uin,"autoStart":enabled}));
            Ok(())
        }
        "forget" => {
            let uin = args.flag("uin").ok_or("Missing --uin")?;
            accounts::forget(uin)?;
            print_json(&serde_json::json!({"forgotten":uin,"dataPreserved":true}));
            Ok(())
        }
        "start" | "stop" | "login" => {
            let uins: Vec<String> = if args.has("all") {
                services::instances::load_registry()
                    .instances
                    .into_iter()
                    .map(|i| i.uin)
                    .collect()
            } else {
                args.flag("uins")
                    .or_else(|| args.flag("uin"))
                    .ok_or("Use --uins QQ,QQ, --uin QQ or --all")?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            };
            if args.has("dry-run") {
                print_json(&serde_json::json!({"operation":sub,"uins":uins,"dryRun":true}));
                return Ok(());
            }
            let outcomes = accounts::batch(&source, sub, &uins).await?;
            let failed = outcomes.iter().any(|o| !o.ok);
            print_json(&serde_json::to_value(&outcomes).map_err(|e| e.to_string())?);
            if failed {
                Err("One or more account operations failed; inspect each result".into())
            } else {
                Ok(())
            }
        }
        _ => Err("Use accounts list|add|select|qr|configure|forget|start|stop|login".into()),
    }
}
