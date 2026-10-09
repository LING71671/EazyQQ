//! Release selection, verified downloads, and installer handoff share one policy.
mod release;
mod download;
mod installer;
pub mod core;
#[cfg(test)]
mod tests;

pub use release::{check, AppUpdateInfo};
pub use download::{prepare, UpdateProgress};
pub use installer::launch;

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent("EazyQQ-Updater")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(300)).build().map_err(|e| e.to_string())
}

pub fn staging_root() -> std::path::PathBuf {
    #[cfg(test)]
    { return crate::services::logging::workspace_root().join("updates"); }
    #[cfg(not(test))]
    {
        if std::env::var_os("EAZYQQ_ROOT").is_some() {
            return crate::services::accounts::data_root().join("updates");
        }
        std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir).join("EazyQQ/updates")
    }
}
