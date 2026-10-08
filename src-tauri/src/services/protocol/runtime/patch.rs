use std::path::{Path, PathBuf};
use std::process::Command;

/// The QQ path NapCat should launch, read from `config/qq_path.txt`.
pub fn configured_qq_path(napcat_dir: &Path) -> Result<PathBuf, String> {
    let cfg = napcat_dir.join("config").join("qq_path.txt");
    if cfg.exists() {
        let raw =
            std::fs::read_to_string(&cfg).map_err(|e| format!("读取 qq_path.txt 失败: {}", e))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("qq_path.txt 为空".to_string());
        }
        let path = PathBuf::from(trimmed);
        if path.exists() {
            return Ok(path);
        } else {
            return Err(format!("qq_path.txt 中的路径不存在: {}", trimmed));
        }
    }

    // Dynamic auto-detection: standard ProgramFiles -> running process -> Windows Registry
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // 1. Check standard ProgramFiles environment variables first (fastest, zero-subprocess)
        for env_var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(prog) = std::env::var(env_var) {
                let candidate = PathBuf::from(prog)
                    .join("Tencent")
                    .join("QQNT")
                    .join("QQ.exe");
                if candidate.is_file() {
                    let path_str = candidate.to_string_lossy().to_string();
                    let _ = std::fs::create_dir_all(napcat_dir.join("config"));
                    let _ = std::fs::write(&cfg, &path_str);
                    tracing::info!("resolved QQ path from standard environment -> {}", path_str);
                    return Ok(candidate);
                }
            }
        }

        // 2. Check if QQ is currently running (silent, no window)
        let mut running_cmd = Command::new("powershell");
        running_cmd.args([
            "-NoProfile",
            "-Command",
            "(Get-Process -Name QQ -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Path -First 1)",
        ]);
        running_cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = running_cmd.output() {
            let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !line.is_empty() {
                let p = PathBuf::from(&line);
                if p.is_file() {
                    let _ = std::fs::create_dir_all(napcat_dir.join("config"));
                    let _ = std::fs::write(&cfg, &line);
                    tracing::info!(
                        "dynamically resolved QQ path from running process -> {}",
                        line
                    );
                    return Ok(p);
                }
            }
        }

        // 3. Query Windows Registry for official QQNT install location (silent, no window)
        let mut reg_cmd = Command::new("powershell");
        reg_cmd.args([
            "-NoProfile",
            "-Command",
            "(Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\WOW6432Node\\Tencent\\QQNT','HKCU:\\Software\\Tencent\\QQNT' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Install -First 1)",
        ]);
        reg_cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = reg_cmd.output() {
            let dir_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !dir_str.is_empty() {
                let candidate = PathBuf::from(&dir_str).join("QQ.exe");
                if candidate.is_file() {
                    let path_str = candidate.to_string_lossy().to_string();
                    let _ = std::fs::create_dir_all(napcat_dir.join("config"));
                    let _ = std::fs::write(&cfg, &path_str);
                    tracing::info!("dynamically resolved QQ path from registry -> {}", path_str);
                    return Ok(candidate);
                }
            }
        }
    }

    Err(format!(
        "未在 {} 找到有效的 qq_path.txt，且未能自动探测到本地 QQNT 安装位置（请先启动一次 QQ 或在 NapCat 目录设置 qq_path.txt）",
        cfg.display()
    ))
}

pub fn sync_qqnt_patch(qq_path: &Path, patch_pkg: &Path) -> Result<(), String> {
    let qq_dir = qq_path
        .parent()
        .ok_or("QQ executable has no parent directory")?;
    let direct = qq_dir.join("resources/app/package.json");
    let candidate = if direct.is_file() {
        direct
    } else {
        let mut versions: Vec<_> = std::fs::read_dir(qq_dir.join("versions"))
            .map_err(|e| format!("QQ version metadata unavailable: {}", e))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join("resources/app/package.json").is_file())
            .collect();
        versions
            .sort_by_key(|entry| semver::Version::parse(&entry.file_name().to_string_lossy()).ok());
        versions
            .last()
            .ok_or("QQ installation has no readable package metadata")?
            .path()
            .join("resources/app/package.json")
    };
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&candidate).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if source["version"].as_str().is_none_or(|s| s.is_empty()) {
        return Err("QQ package metadata has no version".into());
    }
    let mut patch = serde_json::Map::new();
    for key in [
        "name",
        "verHash",
        "version",
        "linuxVersion",
        "linuxVerHash",
        "private",
        "description",
        "productName",
        "author",
        "homepage",
        "sideEffects",
        "bin",
        "buildVersion",
    ] {
        if let Some(value) = source.get(key) {
            patch.insert(key.into(), value.clone());
        }
    }
    patch.insert("main".into(), "./loadNapCat.js".into());
    patch.insert("isPureShell".into(), true.into());
    patch.insert("isByteCodeShell".into(), true.into());
    patch.insert("platform".into(), "win32".into());
    patch.insert("eleArch".into(), "x64".into());
    crate::services::infra::persistence::write_json(patch_pkg, &patch)
}
