use std::fs;
use std::path::PathBuf;
use serde_json::Value;

/// Try to detect an API key from standard environment variables or OpenCode local credentials.
pub fn detect_api_key() -> Option<String> {
    if let Ok(k) = std::env::var("OPENCODE_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_string());
        }
    }
    if let Ok(k) = std::env::var("OPENCODE_GO_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_string());
        }
    }
    for env_var in ["LLM_API_KEY", "OPENAI_API_KEY", "AI_API_KEY"] {
        if let Ok(k) = std::env::var(env_var) {
            if !k.trim().is_empty() {
                return Some(k.trim().to_string());
            }
        }
    }

    let mut candidates = Vec::new();

    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let home_p = PathBuf::from(&home);
        candidates.push(home_p.join(".local").join("share").join("opencode").join("auth.json"));
        candidates.push(home_p.join(".opencode").join("auth.json"));
        candidates.push(home_p.join(".config").join("opencode").join("auth.json"));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(&local_app_data).join("opencode").join("auth.json"));
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(&app_data).join("opencode").join("auth.json"));
    }

    for path in candidates {
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(key) = json.get("opencode-go").and_then(|v| v.get("key")).and_then(|v| v.as_str()) {
                        if !key.trim().is_empty() {
                            return Some(key.trim().to_string());
                        }
                    }
                    if let Some(key) = json.get("opencode").and_then(|v| v.get("key")).and_then(|v| v.as_str()) {
                        if !key.trim().is_empty() {
                            return Some(key.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn detect_opencode_auth_key() -> Option<String> {
    detect_api_key()
}
