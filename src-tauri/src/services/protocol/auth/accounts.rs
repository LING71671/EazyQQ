//! Transport-neutral account orchestration shared by desktop commands and CLI.
use super::session::{probe, validate_uin, LoginEvidence};
use super::{NapCatService, OneBotClient};
use std::path::Path;
use std::sync::OnceLock;
use tokio::sync::Mutex;

pub fn operation_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn clients(
    instance: &crate::services::instances::AccountInstance,
) -> (NapCatService, OneBotClient) {
    let dir = crate::services::protocol::layout::runtime_dir(instance);
    (
        NapCatService::new(
            format!("http://127.0.0.1:{}", instance.webui_port),
            String::new(),
            dir.to_string_lossy().into_owned(),
        ),
        OneBotClient::for_account(
            format!("http://127.0.0.1:{}", instance.http_port),
            Some(instance.uin.clone()),
        ),
    )
}

pub async fn login(source: &Path, uin: &str) -> Result<LoginEvidence, String> {
    validate_uin(uin)?;
    super::boot::set_auto_start(true);
    let _guard = operation_lock()
        .try_lock()
        .map_err(|_| "An account operation is already in progress")?;
    let instance = super::layout::resolve_instance(uin)?;
    crate::services::instances::configure_instance(uin, true)?;
    let (napcat, onebot) = clients(&instance);
    let evidence = probe(&napcat, &onebot).await;
    if evidence.uin.as_deref() == Some(uin) && evidence.logged_in {
        return Ok(evidence);
    }
    if evidence.source == "identity_mismatch" || evidence.logged_in {
        return Err("The selected account endpoint belongs to another QQ account".into());
    }
    if !napcat.is_alive().await {
        let source = source.to_path_buf();
        let account = uin.to_string();
        let outcome =
            tokio::task::spawn_blocking(move || super::boot::start_instance(&source, &account))
                .await
                .map_err(|e| e.to_string())?;
        if !outcome.ok {
            return Err(outcome.detail);
        }
    }
    tokio::time::timeout(std::time::Duration::from_secs(45), async {
        let mut requested = false;
        loop {
            let evidence = probe(&napcat, &onebot).await;
            if evidence.logged_in && evidence.uin.as_deref() == Some(uin) { return Ok(evidence); }
            if evidence.logged_in && evidence.uin.is_some() { return Err("QQ logged into a different account; selection was preserved".into()); }
            if !requested && napcat.is_alive().await {
                match napcat.set_quick_login(uin).await {
                    Ok(_) => requested = true,
                    Err(error) => return Err(format!("QR_REQUIRED: Quick login was rejected: {}", error)),
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        }
    }).await.unwrap_or_else(|_| Err(format!("Account {} was not confirmed within 45 seconds. Open its QR login; the current session was preserved", uin)))
}

/// Record an existing, authenticated default session without moving its running files.
pub fn remember_observed(source: &Path, uin: &str, nickname: Option<&str>) -> Result<(), String> {
    validate_uin(uin)?;
    crate::services::instances::update_registry(|registry| {
        if registry.instances.iter().any(|i| i.uin == uin) {
            return Ok(());
        }
        if registry.instances.iter().any(|i| i.http_port == 3000) {
            return Err("Default endpoint is already assigned to another account".into());
        }
        registry
            .instances
            .push(crate::services::instances::AccountInstance {
                uin: uin.into(),
                nickname: nickname.map(str::to_string),
                http_port: 3000,
                ws_port: 3001,
                webui_port: 6099,
                runtime_dir: Some(source.to_string_lossy().into_owned()),
                auto_start: true,
                child_pid: None,
            });
        Ok(())
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoDto {
    pub uin: String,
    pub nickname: Option<String>,
    pub http_port: u16,
    pub ws_port: u16,
    pub webui_port: u16,
    pub auto_start: bool,
    pub process_managed: bool,
}
impl From<&crate::services::instances::AccountInstance> for AccountInfoDto {
    fn from(instance: &crate::services::instances::AccountInstance) -> Self {
        Self {
            uin: instance.uin.clone(),
            nickname: instance.nickname.clone(),
            http_port: instance.http_port,
            ws_port: instance.ws_port,
            webui_port: instance.webui_port,
            auto_start: instance.auto_start,
            process_managed: super::ownership::is_running(
                &crate::services::protocol::layout::runtime_dir(instance),
            ),
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountReport {
    pub instance: AccountInfoDto,
    pub login: LoginEvidence,
    pub selected: bool,
}

pub async fn status(uin: &str) -> Result<AccountReport, String> {
    validate_uin(uin)?;
    let instance = crate::services::instances::load_registry()
        .instances
        .into_iter()
        .find(|instance| instance.uin == uin)
        .ok_or("Unknown account")?;
    let (napcat, onebot) = clients(&instance);
    let login = tokio::time::timeout(std::time::Duration::from_secs(3), probe(&napcat, &onebot))
        .await
        .unwrap_or_default();
    Ok(AccountReport {
        selected: crate::services::accounts::active().as_deref() == Some(uin),
        instance: AccountInfoDto::from(&instance),
        login,
    })
}

pub async fn list() -> Vec<AccountReport> {
    use futures_util::{stream, StreamExt};
    let selected = crate::services::accounts::active();
    let mut reports: Vec<AccountReport> =
        stream::iter(crate::services::instances::load_registry().instances)
            .map(|instance| {
                let selected = selected.clone();
                async move {
                    let (napcat, onebot) = clients(&instance);
                    let login = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        probe(&napcat, &onebot),
                    )
                    .await
                    .unwrap_or_default();
                    AccountReport {
                        selected: selected.as_deref() == Some(&instance.uin),
                        instance: AccountInfoDto::from(&instance),
                        login,
                    }
                }
            })
            .buffer_unordered(8)
            .collect()
            .await;
    reports.sort_by(|a, b| a.instance.uin.cmp(&b.instance.uin));
    reports
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOutcome {
    pub uin: String,
    pub ok: bool,
    pub detail: String,
}

pub async fn batch(
    source: &Path,
    operation: &str,
    accounts: &[String],
) -> Result<Vec<BatchOutcome>, String> {
    if !matches!(operation, "start" | "stop" | "login") {
        return Err("Supported operations: start, stop, login".into());
    }
    if accounts.is_empty() || accounts.len() > 100 {
        return Err("Select between 1 and 100 accounts".into());
    }
    for uin in accounts {
        validate_uin(uin)?;
    }
    let mut outcomes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for uin in accounts {
        if !seen.insert(uin) {
            continue;
        }
        let (ok, detail) = if operation == "login" {
            match login(source, uin).await {
                Ok(_) => (true, "Login confirmed".into()),
                Err(e) => (false, e),
            }
        } else {
            let source = source.to_path_buf();
            let uin = uin.clone();
            let op = operation.to_string();
            let result = tokio::task::spawn_blocking(move || {
                if op == "start" {
                    super::boot::start_instance(&source, &uin)
                } else {
                    super::boot::stop_instance(&uin)
                }
            })
            .await
            .map_err(|e| e.to_string())?;
            (result.ok, result.detail)
        };
        outcomes.push(BatchOutcome {
            uin: uin.clone(),
            ok,
            detail,
        });
    }
    Ok(outcomes)
}

pub fn forget(uin: &str) -> Result<(), String> {
    validate_uin(uin)?;
    let instance = crate::services::instances::load_registry()
        .instances
        .into_iter()
        .find(|instance| instance.uin == uin)
        .ok_or("Unknown account")?;
    let dir = super::layout::runtime_dir(&instance);
    let _guard =
        crate::services::infra::persistence::FileLock::acquire(&dir.join("eazyqq-lifecycle.lock"))?;
    if super::ownership::is_running(&dir) {
        return Err("Stop the account before forgetting it".into());
    }
    crate::services::instances::forget_instance(uin)
}
