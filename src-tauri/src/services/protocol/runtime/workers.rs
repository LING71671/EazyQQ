//! Independent database/AI/message/scheduler handles for simultaneous accounts.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

struct Worker {
    listener: tauri::async_runtime::JoinHandle<()>,
    scheduler: tauri::async_runtime::JoinHandle<()>,
    _lease: crate::services::infra::persistence::FileLock,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.listener.abort();
        self.scheduler.abort();
    }
}

pub fn start_other_accounts(app: AppHandle, source: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let mut running: HashMap<String, Worker> = HashMap::new();
        let mut attempts: HashMap<String, (std::time::Instant, u32)> = HashMap::new();
        loop {
            let registry = crate::services::instances::load_registry();
            running.retain(|uin, _| {
                registry
                    .instances
                    .iter()
                    .any(|instance| &instance.uin == uin)
            });
            for instance in registry.instances {
                if crate::services::accounts::active().as_deref() == Some(&instance.uin) {
                    continue;
                }
                let (napcat, onebot) = super::accounts::clients(&instance);
                if !napcat.is_alive().await && instance.auto_start {
                    let may_start = attempts
                        .get(&instance.uin)
                        .map(|(last, count)| last.elapsed().as_secs() >= 90 && *count < 3)
                        .unwrap_or(true);
                    if may_start {
                        let source = source.clone();
                        let uin = instance.uin.clone();
                        let outcome = tokio::task::spawn_blocking(move || {
                            super::boot::start_instance(&source, &uin)
                        })
                        .await;
                        let entry = attempts
                            .entry(instance.uin.clone())
                            .or_insert((std::time::Instant::now(), 0));
                        entry.0 = std::time::Instant::now();
                        if !matches!(outcome, Ok(ref result) if result.ok) {
                            entry.1 += 1;
                        }
                    }
                }
                if running.contains_key(&instance.uin) || !napcat.is_alive().await {
                    continue;
                }
                let dir = match crate::services::accounts::ensure_layout(Some(&instance.uin)) {
                    Ok(dir) => dir,
                    Err(_) => continue,
                };
                let lease = match crate::services::infra::persistence::FileLock::try_acquire(
                    &dir.join("message-worker.lock"),
                ) {
                    Ok(lease) => lease,
                    Err(_) => continue,
                };
                let db = match crate::services::db::Database::init(dir.join("eazyqq.db")) {
                    Ok(db) => Arc::new(db),
                    Err(e) => {
                        tracing::error!("account worker database: {}", e);
                        continue;
                    }
                };
                let _ = crate::services::accounts::seed_settings_if_missing(&db, &instance.uin);
                let config = db
                    .get_setting("app_config")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
                let ai = Arc::new(crate::services::ai::AiService::for_account(
                    crate::services::ai::AiRuntimeConfig::from_app_config(
                        config.as_ref().and_then(|v| v.get("ai")),
                        "",
                    ),
                    dir,
                ));
                let onebot = Arc::new(onebot);
                let listener = super::listener::start_onebot_ws_listener(
                    app.clone(),
                    format!("ws://127.0.0.1:{}", instance.ws_port),
                    db.clone(),
                    onebot.clone(),
                    ai.clone(),
                );
                let scheduler = tauri::async_runtime::spawn(async move {
                    loop {
                        if onebot.get_login_info().await.is_ok() {
                            let _ = crate::services::scheduler::tick(&db, &ai, &onebot, None).await;
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    }
                });
                running.insert(
                    instance.uin,
                    Worker {
                        listener,
                        scheduler,
                        _lease: lease,
                    },
                );
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
}
