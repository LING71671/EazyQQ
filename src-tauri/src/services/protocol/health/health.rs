//! Fresh protocol probes and a non-destructive, fault-specific recovery policy.
use super::{NapCatService, OneBotClient};
use crate::services::chain::{self, Link};

#[tracing::instrument(name = "eazyqq.protocol.health", skip_all)]
pub async fn refresh(napcat: &NapCatService, onebot: &OneBotClient) {
    let evidence = super::session::probe(napcat, onebot).await;
    let alive = napcat.is_alive().await;
    if alive {
        chain::record_ok(Link::NapcatWebUi, "WebUI 已响应");
    } else {
        chain::record_error(
            Link::NapcatWebUi,
            "WebUI 不可达；QQ 会话可能仍在线，请检查该账号的 WebUI 端口和日志",
        );
    }
    if evidence.logged_in {
        chain::record_ok(
            Link::QqLogin,
            format!(
                "身份已确认：{} ({})，来源 {}",
                evidence.nickname.as_deref().unwrap_or("未知昵称"),
                evidence.uin.as_deref().unwrap_or("未知账号"),
                evidence.source
            ),
        );
    } else if evidence.source == "identity_mismatch" {
        chain::record_error(Link::QqLogin, "协议端点属于另一账号，请选择正确账号或检查端口配置");
    } else if alive {
        chain::record_unknown(Link::QqLogin, "等待扫码或选择账号，当前没有目标身份确认凭据");
    } else {
        chain::record_unknown(Link::QqLogin, "尚无当前身份确认凭据，请先启动协议并完成扫码");
    }
    match onebot.get_login_info().await {
        Ok(_) => chain::record_ok(Link::OneBotHttp, "已确认身份的 OneBot HTTP 接口可用"),
        Err(e) if evidence.logged_in => chain::record_error(Link::OneBotHttp, e),
        Err(_) => chain::record_unknown(Link::OneBotHttp, "等待目标 QQ 身份确认后验证接口"),
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

#[tracing::instrument(name = "eazyqq.protocol.repair", skip_all)]
pub async fn repair(napcat: &NapCatService, onebot: &OneBotClient) -> RepairReport {
    refresh(napcat, onebot).await;
    let Some(failed) = chain::first_break() else {
        return RepairReport {
            action: "none".into(),
            attempted: false,
            ok: true,
            detail: "没有需要自动恢复的失败环节；待扫码、未验证模型及未观察到的组件继续保持待验证".into(),
        };
    };
    let action = recommended_action(failed.link);
    if failed.link == Link::NapcatWebUi {
        if super::session::probe(napcat, onebot).await.logged_in {
            return RepairReport { action: "inspect_webui".into(), attempted: false, ok: false, detail: "QQ 身份已确认，已保留运行会话；请检查 WebUI 配置".into() };
        }
        if super::ownership::is_running(std::path::Path::new(napcat.napcat_dir())) {
            let dir = std::path::PathBuf::from(napcat.napcat_dir());
            if dir == crate::services::logging::workspace_root().join("resources/napcat")
                && !super::recovery::missing_payload(&dir).is_empty()
            {
                let target = onebot.expected_account().map(str::to_string);
                let outcome = tokio::task::spawn_blocking(move || super::boot::restart_with_uin(&dir, target.as_deref())).await;
                if let Ok(outcome) = outcome {
                    return RepairReport { action: "restore_legacy_runtime".into(), attempted: outcome.attempted, ok: outcome.ok, detail: outcome.detail };
                }
            }
            return RepairReport {
                action: "wait".into(),
                attempted: false,
                ok: false,
                detail:
                    "受管理协议进程仍在运行；已保留会话，请检查本次启动日志"
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
            "{}：{}。未操作协议进程，请按该环节的建议处理",
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
