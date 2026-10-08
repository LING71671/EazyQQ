//! Configuration adapter used by long-lived workers after CLI or GUI settings changes.
pub fn refresh_ai(db: &crate::services::db::Database, ai: &crate::services::ai::AiService) -> bool {
    let config = db
        .get_setting("app_config")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let Some(config) = config else {
        return false;
    };
    let fallback = db
        .get_setting("ai_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let resolved =
        crate::services::ai::AiRuntimeConfig::from_app_config(config.get("ai"), &fallback);
    if resolved == ai.current() {
        return false;
    }
    ai.reconfigure(resolved);
    true
}
