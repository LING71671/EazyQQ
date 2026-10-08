//! Headless message and scheduler workers for one account or all registered accounts.
use crate::{args::Args, context::Services};
use futures_util::StreamExt;
use std::sync::Arc;

pub async fn run(svc: Services, args: &Args) -> Result<(), String> {
    if args.has("all") {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut children = Vec::new();
        for instance in eazyqq_lib::services::instances::load_registry().instances {
            let mut command = tokio::process::Command::new(&executable);
            command
                .args(["--account", &instance.uin, "run"])
                .kill_on_drop(true)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null());
            #[cfg(windows)]
            command.creation_flags(0x08000000);
            children.push(command.spawn().map_err(|e| e.to_string())?);
        }
        if children.is_empty() {
            return Err("Register accounts before using run --all".into());
        }
        println!("{}", serde_json::json!({"startingWorkers":children.len()}));
        let mut checks = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                result = tokio::signal::ctrl_c() => { result.map_err(|e| e.to_string())?; break; },
                _ = checks.tick() => {
                    let mut alive = 0;
                    for child in &mut children { if child.try_wait().map_err(|e| e.to_string())?.is_none() { alive += 1; } }
                    if alive == 0 { return Err("No account workers remain active; inspect worker startup errors or existing GUI/CLI ownership".into()); }
                }
            }
        }
        for child in &mut children {
            let _ = child.kill().await;
        }
        return Ok(());
    }
    let uin = svc
        .onebot
        .expected_account()
        .ok_or("Select --account QQ before starting a worker")?
        .to_string();
    let instance = eazyqq_lib::services::instances::load_registry()
        .instances
        .into_iter()
        .find(|i| i.uin == uin)
        .ok_or("Account is not registered")?;
    let _lease = eazyqq_lib::services::infra::persistence::FileLock::try_acquire(
        &eazyqq_lib::services::accounts::active_data_dir().join("message-worker.lock"),
    )?;
    let svc = Arc::new(svc);
    let scheduler = svc.clone();
    let scheduler_task = tokio::spawn(async move {
        loop {
            if scheduler.onebot.get_login_info().await.is_ok() {
                let _ = eazyqq_lib::services::scheduler::tick(
                    &scheduler.db,
                    &scheduler.ai,
                    &scheduler.onebot,
                    None,
                )
                .await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
    let listener = svc.clone();
    let listener_task = tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) =
                tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}", instance.ws_port))
                    .await
            {
                let (_, mut read) = stream.split();
                while let Some(Ok(message)) = read.next().await {
                    if let Ok(text) = message.to_text() {
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(text) {
                            if eazyqq_lib::services::protocol::session::account_id(
                                &event["self_id"],
                            )
                            .as_deref()
                                == Some(&uin)
                            {
                                eazyqq_lib::services::infra::runtime_config::refresh_ai(
                                    &listener.db,
                                    &listener.ai,
                                );
                                let _ = eazyqq_lib::services::protocol::handle_onebot_event(
                                    None,
                                    &event,
                                    &listener.db,
                                    &listener.onebot,
                                    &listener.ai,
                                    true,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    });
    println!(
        "{}",
        serde_json::json!({"worker":"running","account":instance.uin})
    );
    let result = tokio::signal::ctrl_c().await.map_err(|e| e.to_string());
    listener_task.abort();
    scheduler_task.abort();
    result
}
