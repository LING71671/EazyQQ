//! Account-specific protocol installation, endpoint and filesystem boundaries.
use crate::services::instances::AccountInstance;
use crate::services::logging;
use std::path::{Path, PathBuf};

pub fn runtime_dir(instance: &AccountInstance) -> PathBuf {
    instance
        .runtime_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            crate::services::accounts::account_dir(Some(&instance.uin)).join("protocol")
        })
}

/// Refresh stale reservations only before a private runtime is first materialized.
/// Existing configuration and externally managed sessions retain their endpoints.
pub fn resolve_instance(uin: &str) -> Result<AccountInstance, String> {
    let instance = crate::services::instances::get_or_register_instance(uin, None)?;
    if instance.runtime_dir.is_none()
        && !runtime_dir(&instance).join("config/webui.json").exists()
        && [instance.http_port, instance.ws_port, instance.webui_port]
            .iter()
            .any(|port| !crate::services::instances::port_available(*port))
    {
        crate::services::instances::refresh_unused_ports(uin)
    } else {
        Ok(instance)
    }
}

pub fn selected(source: &Path) -> (PathBuf, u16, u16, u16) {
    crate::services::accounts::active()
        .and_then(|uin| {
            crate::services::instances::load_registry()
                .instances
                .into_iter()
                .find(|i| i.uin == uin)
        })
        .map(|i| (runtime_dir(&i), i.http_port, i.ws_port, i.webui_port))
        .unwrap_or_else(|| (source.to_path_buf(), 3000, 3001, 6099))
}

pub fn is_immutable_resource(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !matches!(
        name.as_str(),
        "config" | "logs" | "cache" | "plugins" | "loadnapcat.js" | "loadnapcat.cjs" | "qqnt.json"
    ) && !name.starts_with("eazyqq-")
        && ![
            ".db", ".db-wal", ".db-shm", ".sqlite", ".sqlite3", ".log", ".bak",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn link_resources(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if !is_immutable_resource(&name_text) {
            continue;
        }
        let dest = target.join(&name);
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            link_resources(&entry.path(), &dest)?;
        } else {
            let incoming = dest.with_extension("eazyqq-link");
            std::fs::hard_link(entry.path(), &incoming)
                .or_else(|_| std::fs::copy(entry.path(), &incoming).map(|_| ()))
                .map_err(|e| e.to_string())?;
            std::fs::rename(incoming, dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn prepare(source: &Path, instance: &AccountInstance) -> Result<PathBuf, String> {
    let dir = runtime_dir(instance);
    if dir == source {
        return Ok(dir);
    }
    link_resources(source, &dir)?;
    std::fs::create_dir_all(dir.join("config")).map_err(|e| e.to_string())?;
    let config_path = dir.join("config/webui.json");
    if !config_path.exists() {
        let mut config: serde_json::Value = std::fs::read(source.join("config/webui.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        config["host"] = "127.0.0.1".into();
        config["port"] = instance.webui_port.into();
        config["autoLoginAccount"] = instance.uin.clone().into();
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        config["token"] = random
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>()
            .into();
        std::fs::write(
            config_path,
            serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    if let Ok(path) = crate::services::protocol::patch::configured_qq_path(source) {
        std::fs::write(
            dir.join("config/qq_path.txt"),
            path.to_string_lossy().as_bytes(),
        )
        .map_err(|e| e.to_string())?;
    }
    sync_onebot_config(&dir, &instance.uin, instance.http_port, instance.ws_port)?;
    Ok(dir)
}

pub fn sync_onebot_config(
    napcat_dir: &Path,
    uin: &str,
    http_port: u16,
    ws_port: u16,
) -> Result<(), String> {
    let config_dir = napcat_dir.join("config");
    logging::ensure_dir(&config_dir);

    // 1. Write onebot11_<uin>.json
    let onebot_cfg_path = config_dir.join(format!("onebot11_{}.json", uin));
    let cfg_json = serde_json::json!({
        "network": {
            "httpServers": [
                {
                    "name": "http-server",
                    "enable": true,
                    "port": http_port,
                    "host": "127.0.0.1",
                    "enableCors": true,
                    "enableWebsocket": false,
                    "messagePostFormat": "array",
                    "token": "",
                    "debug": false
                }
            ],
            "httpSseServers": [],
            "httpClients": [],
            "websocketServers": [
                {
                    "name": "websocket-server",
                    "enable": true,
                    "host": "127.0.0.1",
                    "port": ws_port,
                    "messagePostFormat": "array",
                    "reportSelfMessage": false,
                    "token": "",
                    "enableForcePushEvent": true,
                    "debug": false,
                    "heartInterval": 30000
                }
            ],
            "websocketClients": [],
            "plugins": []
        },
        "musicSignUrl": "",
        "enableLocalFile2Url": false,
        "parseMultMsg": false
    });

    let content = serde_json::to_string_pretty(&cfg_json)
        .map_err(|e| format!("序列化 OneBot 配置失败: {}", e))?;
    std::fs::write(&onebot_cfg_path, content)
        .map_err(|e| format!("写入 {} 失败: {}", onebot_cfg_path.display(), e))?;

    // 2. Write napcat_<uin>.json
    let napcat_cfg_path = config_dir.join(format!("napcat_{}.json", uin));
    if !napcat_cfg_path.exists() {
        let napcat_json = serde_json::json!({
            "fileLog": true,
            "consoleLog": true,
            "fileLogLevel": "debug",
            "consoleLogLevel": "info"
        });
        let _ = std::fs::write(
            &napcat_cfg_path,
            serde_json::to_string_pretty(&napcat_json).unwrap_or_default(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutable_files_and_process_receipts_are_never_shared() {
        for name in [
            "guild1.db",
            "guild1.db-wal",
            "cache",
            "logs",
            "plugins",
            "eazyqq-launch.json",
            "eazyqq-process.json",
            "qqnt.json",
            "loadNapCat.cjs",
        ] {
            assert!(!is_immutable_resource(name));
        }
        for name in [
            "napcat.mjs",
            "NapCatWinBootHook.dll",
            "native",
            "static",
            "node_modules",
            "package.json",
        ] {
            assert!(is_immutable_resource(name));
        }
    }
}
