use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::services::logging;

pub use crate::services::identity::bootstrap::{
    bootstrap_path, data_root, read_bootstrap, set_webview_compat, write_bootstrap, Bootstrap,
    UNBOUND,
};
pub use crate::services::identity::machine::{account_dir, machine_id, sanitize_account};
pub use crate::services::identity::migration::{
    migrate_legacy_if_needed, migrate_unbound_if_needed, seed_settings_if_missing,
    sweep_napcat_artifacts,
};

fn active_slot() -> &'static Mutex<Option<String>> {
    static S: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// The account this process is serving.
pub fn active() -> Option<String> {
    active_slot().lock().ok().and_then(|s| s.clone())
}

pub fn set_active(uin: Option<String>) {
    if let Ok(mut s) = active_slot().lock() {
        *s = uin;
    }
}

/// The data directory for the active account.
pub fn active_data_dir() -> PathBuf {
    account_dir(active().as_deref())
}

/// Create the directory layout for an account and return its root.
pub fn ensure_layout(uin: Option<&str>) -> Result<PathBuf, String> {
    let dir = account_dir(uin);
    for sub in ["", "logs", "group_files", "diagnostics"] {
        let path = if sub.is_empty() {
            dir.clone()
        } else {
            dir.join(sub)
        };
        logging::ensure_dir(&path);
    }
    Ok(dir)
}

/// Did this account switch since the process started?
pub fn account_changed(observed: &str) -> bool {
    match active() {
        Some(current) => current != observed,
        None => false,
    }
}

/// Adopt an account: persist it and make it active for this process.
pub fn adopt(uin: &str) -> Result<(), String> {
    crate::services::identity::machine::validate_uin(uin)?;
    super::bootstrap::update_bootstrap(|bootstrap| bootstrap.last_account = Some(uin.to_string()))?;
    set_active(Some(uin.to_string()));
    Ok(())
}

/// Clear the active account from bootstrap and in-memory state.
pub fn clear_active() -> Result<(), String> {
    super::bootstrap::update_bootstrap(|bootstrap| bootstrap.last_account = None)?;
    set_active(None);
    Ok(())
}
