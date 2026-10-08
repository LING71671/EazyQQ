//! Fresh protocol probes and a non-destructive, fault-specific recovery policy.
use super::{NapCatService, OneBotClient};
use crate::services::chain::{self, Link};

pub async fn refresh(napcat: &NapCatService, onebot: &OneBotClient) {
    let evidence = super::session::probe(napcat, onebot).await;
    let alive = napcat.is_alive().await;
    if alive {
        chain::record_ok(Link::NapcatWebUi, "WebUI reachable");
    } else {
        chain::record_error(
            Link::NapcatWebUi,
            "WebUI unavailable; QQ session may still be online",
        );
    }
    if evidence.logged_in {
        chain::record_ok(
            Link::QqLogin,
            format!(
                "Authenticated via {} ({})",
                evidence.source,
                evidence.uin.as_deref().unwrap_or("unknown account")
            ),
        );
    } else if evidence.source == "identity_mismatch" {
        chain::record_error(Link::QqLogin, "Protocol endpoint belongs to another account; select the correct account or inspect its port configuration");
    } else if alive {
        chain::record_unknown(Link::QqLogin, "Awaiting QR scan or account selection");
    } else {
        chain::record_unknown(Link::QqLogin, "No current authentication evidence");
    }
    match onebot.get_login_info().await {
        Ok(_) => chain::record_ok(Link::OneBotHttp, "Authenticated OneBot API reachable"),
        Err(e) if evidence.logged_in => chain::record_error(Link::OneBotHttp, e),
        Err(_) => chain::record_unknown(Link::OneBotHttp, "Available after QQ login"),
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub action: String,
    pub attempted: bool,
    pub ok: bool,
    pub detail: String,
}

pub fn recommended_action(link: Link) -> &'static str {
    match link {
        Link::QqLogin => "login",
        Link::AiProvider => "settings",
        Link::NapcatWebUi => "start_if_offline",
        Link::OneBotHttp | Link::OneBotWs => "reconnect",
        Link::Database => "inspect_storage",
        Link::Scheduler | Link::Frontend => "refresh",
    }
}

pub async fn repair(napcat: &NapCatService, onebot: &OneBotClient) -> RepairReport {
    refresh(napcat, onebot).await;
    let Some(failed) = chain::first_break() else {
        return RepairReport {
            action: "none".into(),
            attempted: false,
            ok: true,
            detail: "No failed link requires recovery".into(),
        };
    };
    let action = recommended_action(failed.link);
    if failed.link == Link::NapcatWebUi {
        if super::session::probe(napcat, onebot).await.logged_in {
            return RepairReport { action: "inspect_webui".into(), attempted: false, ok: false, detail: "QQ is authenticated. Its running session was preserved; inspect WebUI configuration".into() };
        }
        if super::ownership::is_running(std::path::Path::new(napcat.napcat_dir())) {
            return RepairReport {
                action: "wait".into(),
                attempted: false,
                ok: false,
                detail:
                    "An owned protocol process is running. Inspect its boot log before restarting"
                        .into(),
            };
        }
        let dir = std::path::PathBuf::from(napcat.napcat_dir());
        let target = onebot.expected_account().map(str::to_string);
        super::boot::set_auto_start(true);
        let outcome = tokio::task::spawn_blocking(move || {
            super::boot::start_with_args(&dir, None, target.as_deref())
        })
        .await;
        if let Ok(outcome) = outcome {
            return RepairReport {
                action: action.into(),
                attempted: outcome.attempted,
                ok: outcome.ok,
                detail: outcome.detail,
            };
        }
    }
    RepairReport {
        action: action.into(),
        attempted: false,
        ok: false,
        detail: format!(
            "{}: {}. The protocol process was preserved",
            failed.label, failed.detail
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_ai_and_database_faults_never_restart_qq() {
        for link in [
            Link::QqLogin,
            Link::AiProvider,
            Link::Database,
            Link::OneBotWs,
        ] {
            assert_ne!(recommended_action(link), "start_if_offline");
        }
    }
}
