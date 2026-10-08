use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::services::identity::bootstrap::data_root;

pub const DEFAULT_BASE_HTTP_PORT: u16 = 3000;
pub const DEFAULT_BASE_WS_PORT: u16 = 3001;

fn default_webui_port() -> u16 {
    6099
}

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
    #[serde(default = "default_webui_port")]
    pub webui_port: u16,
    #[serde(default)]
    pub runtime_dir: Option<String>,
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
    data_root()
        .join("accounts")
        .join(super::machine::machine_id())
        .join("instances.json")
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
    crate::services::infra::persistence::write_json(&instances_path(), reg)
}

pub fn update_registry<T>(
    operation: impl FnOnce(&mut InstancesRegistry) -> Result<T, String>,
) -> Result<T, String> {
    let _lock = crate::services::infra::persistence::FileLock::acquire(
        &instances_path().with_extension("lock"),
    )?;
    let mut registry = match std::fs::read(instances_path()) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Account registry is corrupt; preserve and repair it: {}", e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => InstancesRegistry::default(),
        Err(e) => return Err(e.to_string()),
    };
    let value = operation(&mut registry)?;
    save_registry(&registry)?;
    Ok(value)
}

pub fn port_available(port: u16) -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(75)).is_err()
        && std::net::TcpListener::bind(address).is_ok()
}

pub fn allocate_ports(reg: &InstancesRegistry) -> (u16, u16) {
    let used: std::collections::HashSet<u16> = reg
        .instances
        .iter()
        .flat_map(|i| [i.http_port, i.ws_port, i.webui_port])
        .collect();
    for http in (3000u16..=65000).step_by(2) {
        if !used.contains(&http) && !used.contains(&(http + 1)) {
            return (http, http + 1);
        }
    }
    (0, 0)
}

pub fn get_or_register_instance(
    uin: &str,
    nickname: Option<&str>,
) -> Result<AccountInstance, String> {
    super::machine::validate_uin(uin)?;
    update_registry(|reg| {
        if let Some(existing) = reg.instances.iter_mut().find(|i| i.uin == uin) {
            if let Some(nick) = nickname {
                existing.nickname = Some(nick.into());
            }
            return Ok(existing.clone());
        }
        let mut ports_registry = reg.clone();
        let (http, ws) = loop {
            let (http, ws) = allocate_ports(&ports_registry);
            if http == 0 {
                return Err("No available OneBot port pair".into());
            }
            if port_available(http) && port_available(ws) {
                break (http, ws);
            }
            ports_registry.instances.push(AccountInstance {
                uin: String::new(),
                nickname: None,
                http_port: http,
                ws_port: ws,
                webui_port: 0,
                runtime_dir: None,
                auto_start: false,
                child_pid: None,
            });
        };
        let webui_port = (6099u16..=65000)
            .find(|port| {
                *port != http
                    && *port != ws
                    && !reg
                        .instances
                        .iter()
                        .any(|i| [i.http_port, i.ws_port, i.webui_port].contains(port))
                    && port_available(*port)
            })
            .ok_or("No available WebUI port")?;
        let instance = AccountInstance {
            uin: uin.into(),
            nickname: nickname.map(str::to_string),
            http_port: http,
            ws_port: ws,
            webui_port,
            runtime_dir: None,
            auto_start: false,
            child_pid: None,
        };
        reg.instances.push(instance.clone());
        Ok(instance)
    })
}

pub fn update_instance_pid(uin: &str, pid: Option<u32>) -> Result<(), String> {
    update_registry(|reg| {
        let instance = reg
            .instances
            .iter_mut()
            .find(|i| i.uin == uin)
            .ok_or("Unknown account")?;
        instance.child_pid = pid;
        Ok(())
    })
}

pub fn configure_instance(uin: &str, auto_start: bool) -> Result<(), String> {
    update_registry(|reg| {
        let instance = reg
            .instances
            .iter_mut()
            .find(|i| i.uin == uin)
            .ok_or("Unknown account")?;
        instance.auto_start = auto_start;
        Ok(())
    })
}

pub fn forget_instance(uin: &str) -> Result<(), String> {
    if crate::services::accounts::active().as_deref() == Some(uin)
        || super::bootstrap::read_bootstrap().last_account.as_deref() == Some(uin)
    {
        return Err("Select another account before forgetting the active one".into());
    }
    update_registry(|reg| {
        if !reg.instances.iter().any(|instance| instance.uin == uin) {
            return Err("Unknown account".into());
        }
        reg.instances.retain(|i| i.uin != uin);
        Ok(())
    })
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
            webui_port: 6099,
            runtime_dir: None,
            auto_start: true,
            child_pid: None,
        });

        let (h2, w2) = allocate_ports(&reg);
        assert_eq!((h2, w2), (3002, 3003));
    }
}

pub fn refresh_unused_ports(uin: &str) -> Result<AccountInstance, String> {
    update_registry(|registry| {
        let old = registry
            .instances
            .iter()
            .find(|instance| instance.uin == uin)
            .cloned()
            .ok_or("Unknown account")?;
        let mut other = registry.clone();
        other.instances.retain(|instance| instance.uin != uin);
        let mut reservation = other.clone();
        let (http_port, ws_port) = loop {
            let (http, ws) = allocate_ports(&reservation);
            if http == 0 {
                return Err("No available OneBot port pair".into());
            }
            if port_available(http) && port_available(ws) {
                break (http, ws);
            }
            let mut blocked = old.clone();
            blocked.http_port = http;
            blocked.ws_port = ws;
            reservation.instances.push(blocked);
        };
        let webui_port = (6099u16..=65000)
            .find(|port| {
                *port != http_port
                    && *port != ws_port
                    && !other.instances.iter().any(|instance| {
                        [instance.http_port, instance.ws_port, instance.webui_port].contains(port)
                    })
                    && port_available(*port)
            })
            .ok_or("No available WebUI port")?;
        let entry = registry
            .instances
            .iter_mut()
            .find(|instance| instance.uin == uin)
            .unwrap();
        entry.http_port = http_port;
        entry.ws_port = ws_port;
        entry.webui_port = webui_port;
        Ok(entry.clone())
    })
}
