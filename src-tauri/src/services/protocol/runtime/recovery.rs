//! Explicit recovery of installer-owned legacy runtimes without touching other sessions.
use std::path::{Path, PathBuf};

pub fn missing_payload(dir: &Path) -> Vec<&'static str> {
    ["NapCatWinBootMain.exe", "NapCatWinBootHook.dll", "napcat.mjs"]
        .into_iter().filter(|name| !dir.join(name).is_file()).collect()
}

pub fn restart_target(dir: &Path) -> Result<PathBuf, String> {
    let legacy = crate::services::logging::workspace_root().join("resources/napcat");
    if dir != legacy { return Ok(dir.to_path_buf()); }
    let registry = match std::fs::read(crate::services::instances::instances_path()) {
        Ok(bytes) => serde_json::from_slice::<crate::services::instances::InstancesRegistry>(&bytes)
            .map_err(|e| format!("Account registry is corrupt; recovery preserved the old process: {}", e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e.to_string()),
    };
    let owners: Vec<_> = registry.instances.iter().filter(|instance| super::layout::runtime_dir(instance) == dir).collect();
    if owners.len() > 1 { return Err("Multiple accounts reference the legacy runtime; repair their registration first".into()); }
    let target = owners.first().map(|instance| crate::services::accounts::account_dir(Some(&instance.uin)).join("protocol"))
        .unwrap_or_else(super::layout::unbound_runtime_dir);
    if super::ownership::is_running(&target) {
        return Err("A private protocol session is already running; it was preserved".into());
    }
    let source = super::boot::locate_napcat_dir();
    if source == legacy || !missing_payload(&source).is_empty() {
        return Err("The new protocol payload is incomplete; reinstall the current application".into());
    }
    prepare_replacement(&source, &target)?;
    if let Some(instance) = owners.first() {
        super::layout::sync_onebot_config(&target, &instance.uin, instance.http_port, instance.ws_port)?;
        let path = target.join("config/webui.json");
        let mut config: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        config["port"] = instance.webui_port.into();
        config["autoLoginAccount"] = instance.uin.clone().into();
        crate::services::infra::persistence::write_json(&path, &config)?;
    }
    Ok(target)
}

pub fn record_relocation(old: &Path, new: &Path) -> Result<(), String> {
    if old == new { return Ok(()); }
    crate::services::instances::update_registry(|registry| {
        relocate_registry(registry, old, new);
        Ok(())
    })
}

fn relocate_registry(registry: &mut crate::services::instances::InstancesRegistry, old: &Path, new: &Path) {
    for instance in &mut registry.instances {
        if super::layout::runtime_dir(instance) == old { instance.runtime_dir = Some(new.to_string_lossy().into_owned()); }
    }
}

fn prepare_replacement(source: &Path, target: &Path) -> Result<(), String> {
    if !missing_payload(source).is_empty() {
        return Err("Replacement protocol payload is incomplete".into());
    }
    // Materialize and validate the replacement before stopping the verified old job.
    super::layout::prepare_unbound(&source, &target)?;
    super::boot::resolve(&target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relocation_preserves_identity_ports_and_unrelated_accounts() {
        let mut registry: crate::services::instances::InstancesRegistry = serde_json::from_value(serde_json::json!({"instances":[
            {"uin":"10001","http_port":3200,"ws_port":3201,"webui_port":6300,"auto_start":false,"runtime_dir":"legacy"},
            {"uin":"10002","http_port":3202,"ws_port":3203,"webui_port":6301,"runtime_dir":"other"}
        ]})).unwrap();
        relocate_registry(&mut registry, Path::new("legacy"), Path::new("private"));
        assert_eq!(registry.instances[0].runtime_dir.as_deref(), Some("private"));
        assert_eq!(registry.instances[0].uin, "10001");
        assert_eq!((registry.instances[0].http_port,registry.instances[0].ws_port,registry.instances[0].webui_port), (3200,3201,6300));
        assert!(!registry.instances[0].auto_start);
        assert_eq!(registry.instances[1].runtime_dir.as_deref(), Some("other"));
    }

    #[test]
    fn replacement_uses_private_copies_and_preserves_configuration() {
        let root = crate::services::logging::workspace_root().join("legacy-recovery");
        let source = root.join("installed");
        let target = root.join("private");
        std::fs::create_dir_all(source.join("config")).unwrap();
        std::fs::create_dir_all(target.join("config")).unwrap();
        for name in missing_payload(&source) { std::fs::write(source.join(name), b"fixture").unwrap(); }
        let qq = root.join("QQ.exe");
        std::fs::write(&qq, b"never executed").unwrap();
        std::fs::write(source.join("config/qq_path.txt"), qq.to_string_lossy().as_bytes()).unwrap();
        std::fs::write(target.join("config/webui.json"), br#"{"token":"private-token","port":6200}"#).unwrap();
        prepare_replacement(&source, &target).unwrap();
        assert_eq!(std::fs::read(target.join("config/webui.json")).unwrap(), br#"{"token":"private-token","port":6200}"#);
        std::fs::write(target.join("napcat.mjs"), b"private edit").unwrap();
        assert_eq!(std::fs::read(source.join("napcat.mjs")).unwrap(), b"fixture");
        assert!(!source.join("eazyqq-process.json").exists());
    }

    #[test]
    fn incomplete_replacement_fails_before_touching_the_destination() {
        let root = crate::services::logging::workspace_root().join("legacy-recovery-incomplete");
        let source = root.join("installed");
        let target = root.join("private");
        std::fs::create_dir_all(&source).unwrap();
        assert!(prepare_replacement(&source, &target).is_err());
        assert!(!target.exists());
        assert!(unavailable_message(&source).contains("napcat.mjs"));
        assert_eq!(restart_target(&source).unwrap(), source);
    }
}

pub fn unavailable_message(dir: &Path) -> String {
    let missing = missing_payload(dir);
    if !missing.is_empty() {
        return format!("协议文件缺失：{}。请点击恢复协议，使用新版资源重建运行目录。", missing.join("、"));
    }
    if super::ownership::is_running(dir) {
        "协议进程仍在运行，但二维码服务无法连接。请先查看链路诊断；恢复协议会重新启动当前受管理会话。".into()
    } else {
        "二维码服务尚未启动或连接失败。请查看链路诊断，或点击恢复协议。".into()
    }
}
