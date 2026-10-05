use std::sync::Arc;

use eazyqq_lib::services::ai::AiService;
use eazyqq_lib::services::db::Database;
use eazyqq_lib::services::logging;
use eazyqq_lib::services::napcat::NapCatService;
use eazyqq_lib::services::onebot::OneBotClient;

pub const NAPCAT_WEBUI: &str = "http://127.0.0.1:6099";
pub const ONEBOT_HTTP: &str = "http://127.0.0.1:3000";

pub struct Services {
    pub db: Arc<Database>,
    pub napcat: Arc<NapCatService>,
    pub onebot: Arc<OneBotClient>,
    pub ai: Arc<AiService>,
}

impl Services {
    pub fn build() -> Result<Self, String> {
        let root = logging::workspace_root();

        // Resolve the account before touching any data path, exactly as the GUI does -
        // otherwise the CLI would read a different directory than the app writes to.
        let bootstrap = eazyqq_lib::services::accounts::read_bootstrap();
        eazyqq_lib::services::accounts::set_active(bootstrap.last_account.clone());

        let data_dir = logging::data_dir();
        logging::ensure_dir(&data_dir);

        // Relocate pre-isolation data BEFORE creating a database: once an empty
        // `eazyqq.db` exists in the account directory the migration is skipped by design,
        // and the user would appear to have lost all their rules and drafts.
        if let Some(uin) = eazyqq_lib::services::accounts::active() {
            if let Err(e) = eazyqq_lib::services::accounts::migrate_legacy_if_needed(&uin) {
                tracing::warn!("account data migration failed: {}", e);
            }
            eazyqq_lib::services::accounts::migrate_unbound_if_needed(&uin);
            // NapCat's own logs contain message text and live in the shared `napcat/`
            // directory; sweep them under the account like the GUI does.
            let swept = eazyqq_lib::services::accounts::sweep_napcat_artifacts(
                &root.join("napcat"),
                &uin,
            );
            if !swept.is_empty() {
                tracing::info!("swept shared NapCat artefacts: {}", swept.join(", "));
            }
        }

        let db = Arc::new(
            Database::init(data_dir.join("eazyqq.db"))
                .map_err(|e| format!("cannot open SQLite database: {}", e))?,
        );

        eazyqq_lib::services::scheduler::normalize_legacy_intervals(&db);

        if let Some(uin) = eazyqq_lib::services::accounts::active() {
            if let Some(note) = eazyqq_lib::services::accounts::seed_settings_if_missing(&db, &uin)
            {
                tracing::info!("{}", note);
            }
        }

        match eazyqq_lib::services::accounts::active() {
            Some(uin) => tracing::info!("using data for account {} at {}", uin, data_dir.display()),
            None => tracing::warn!(
                "no account recorded yet; using {} (run the app once while logged in to \
                 bind this machine to an account)",
                data_dir.display()
            ),
        }

        let napcat_dir = root.join("napcat");

        // No fallback token: it is read from `napcat/config/webui.json`. A token is
        // generated per installation, so a hardcoded one would fail on any other machine
        // and would ship a credential in the source tree.
        let napcat = Arc::new(NapCatService::new(
            NAPCAT_WEBUI.to_string(),
            String::new(),
            napcat_dir.to_string_lossy().to_string(),
        ));

        let onebot = Arc::new(OneBotClient::new(ONEBOT_HTTP.to_string()));

        let api_key = std::env::var("TOKENRHYTHM_API_KEY")
            .or_else(|_| std::env::var("AI_API_KEY"))
            .ok()
            .or_else(|| db.get_setting("tokenrhythm_api_key").ok().flatten())
            .unwrap_or_default();

        // Same resolution path as the GUI, so the CLI reports what the app really uses.
        let ai_config = {
            let raw = db.get_setting("app_config").ok().flatten();
            let parsed = raw
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
            eazyqq_lib::services::ai::AiRuntimeConfig::from_app_config(
                parsed.as_ref().and_then(|v| v.get("ai")),
                &api_key,
            )
        };
        tracing::info!("active AI provider -> {}", ai_config.describe());
        let ai = Arc::new(AiService::new(ai_config));

        Ok(Services {
            db,
            napcat,
            onebot,
            ai,
        })
    }
}
