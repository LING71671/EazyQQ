use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::services::identity::bootstrap::data_root;
use crate::services::logging;

pub const DEFAULT_BASE_HTTP_PORT: u16 = 3000;
pub const DEFAULT_BASE_WS_PORT: u16 = 3001;

fn default_auto_start() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInstance {
    pub uin: String,
    #[serde(default)]
    pub nickname: Option<String>,
    pub http_port: u16,
    pub ws_port: u16,
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,
    #[serde(default)]
    pub child_pid: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstancesRegistry {
    pub instances: Vec<AccountInstance>,
}

pub fn instances_path() -> PathBuf {
    data_root().join("instances.json")
}

pub fn load_registry() -> InstancesRegistry {
    let path = instances_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(
                "instances: {} is not valid JSON ({}); using empty registry",
                path.display(),
                e
            );
            InstancesRegistry::default()
        }),
        Err(_) => InstancesRegistry::default(),
    }
}

pub fn save_registry(reg: &InstancesRegistry) -> Result<(), String> {
    let path = instances_path();
    if let Some(parent) = path.parent() {
        logging::ensure_dir(parent);
    }
    let json = serde_json::to_string_pretty(reg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {}", path.display(), e))
}

pub fn allocate_ports(reg: &InstancesRegistry) -> (u16, u16) {
    let used_ports: std::collections::HashSet<u16> = reg
        .instances
        .iter()
        .flat_map(|i| [i.http_port, i.ws_port])
        .collect();

    let mut i = 0u16;
    loop {
        let http = DEFAULT_BASE_HTTP_PORT + i * 2;
        let ws = DEFAULT_BASE_WS_PORT + i * 2;
        if !used_ports.contains(&http) && !used_ports.contains(&ws) {
            return (http, ws);
        }
        i += 1;
    }
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

pub fn get_or_register_instance(
    uin: &str,
    nickname: Option<&str>,
    napcat_dir: &Path,
) -> Result<AccountInstance, String> {
    let mut reg = load_registry();
    if let Some(existing) = reg.instances.iter_mut().find(|i| i.uin == uin) {
        if let Some(nick) = nickname {
            existing.nickname = Some(nick.to_string());
        }
        let inst = existing.clone();
        let _ = save_registry(&reg);
        let _ = sync_onebot_config(napcat_dir, uin, inst.http_port, inst.ws_port);
        return Ok(inst);
    }

    let (http, ws) = allocate_ports(&reg);
    let new_inst = AccountInstance {
        uin: uin.to_string(),
        nickname: nickname.map(|s| s.to_string()),
        http_port: http,
        ws_port: ws,
        auto_start: true,
        child_pid: None,
    };

    reg.instances.push(new_inst.clone());
    save_registry(&reg)?;
    sync_onebot_config(napcat_dir, uin, http, ws)?;

    Ok(new_inst)
}

pub fn update_instance_pid(uin: &str, pid: Option<u32>) -> Result<(), String> {
    let mut reg = load_registry();
    if let Some(inst) = reg.instances.iter_mut().find(|i| i.uin == uin) {
        inst.child_pid = pid;
        save_registry(&reg)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_ports_increments_by_two() {
        let mut reg = InstancesRegistry::default();
        let (h1, w1) = allocate_ports(&reg);
        assert_eq!((h1, w1), (3000, 3001));

        reg.instances.push(AccountInstance {
            uin: "10001".to_string(),
            nickname: None,
            http_port: h1,
            ws_port: w1,
            auto_start: true,
            child_pid: None,
        });

        let (h2, w2) = allocate_ports(&reg);
        assert_eq!((h2, w2), (3002, 3003));
    }
}
