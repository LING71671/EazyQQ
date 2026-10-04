use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::services::logging;

/// Directory used before a QQ account is known.
pub const UNBOUND: &str = "_unbound";

/// App-level state that must be readable without knowing the account.
///
/// `rename_all` is required: this file is hand-editable and read by other tools, so its
/// field names are camelCase. Without it serde looks for `last_account` and silently gets
/// `None` from a file that says `lastAccount`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    /// The account whose data directory this process should use.
    #[serde(default)]
    pub last_account: Option<String>,
    /// Adds `--no-sandbox` to the WebView2 command line. App-level because it must be
    /// applied before the window exists, and the account is not known yet at that point.
    #[serde(default)]
    pub webview_compat_mode: bool,
}

pub fn data_root() -> PathBuf {
    logging::workspace_root().join("EazyQQ_Data")
}

pub fn bootstrap_path() -> PathBuf {
    data_root().join("bootstrap.json")
}

/// Read the bootstrap file. A missing or unreadable file is not an error: it simply means
/// this is a fresh install, which is a perfectly normal state.
pub fn read_bootstrap() -> Bootstrap {
    match std::fs::read_to_string(bootstrap_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(
                "accounts: {} is not valid JSON ({}); starting from defaults",
                bootstrap_path().display(),
                e
            );
            Bootstrap::default()
        }),
        Err(_) => Bootstrap::default(),
    }
}

pub fn write_bootstrap(b: &Bootstrap) -> Result<(), String> {
    let path = bootstrap_path();
    if let Some(parent) = path.parent() {
        logging::ensure_dir(parent);
    }
    let text = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("写入 {} 失败: {}", path.display(), e))
}

/// Update just the WebView compatibility flag, leaving the account untouched.
pub fn set_webview_compat(enabled: bool) -> Result<(), String> {
    let mut b = read_bootstrap();
    b.webview_compat_mode = enabled;
    write_bootstrap(&b)
}
