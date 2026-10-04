use std::path::{Path, PathBuf};
use std::process::Command;

/// The QQ path NapCat should launch, read from `config/qq_path.txt`.
pub fn configured_qq_path(napcat_dir: &Path) -> Result<PathBuf, String> {
    let cfg = napcat_dir.join("config").join("qq_path.txt");
    if cfg.exists() {
        let raw = std::fs::read_to_string(&cfg).map_err(|e| format!("读取 qq_path.txt 失败: {}", e))?;
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

    // Dynamic auto-detection: running process -> Windows Registry -> standard ProgramFiles
    #[cfg(target_os = "windows")]
    {
        // 1. Check if QQ is currently running
        if let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-Process -Name QQ -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Path -First 1)",
            ])
            .output()
        {
            let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !line.is_empty() {
                let p = PathBuf::from(&line);
                if p.is_file() {
                    let _ = std::fs::create_dir_all(napcat_dir.join("config"));
                    let _ = std::fs::write(&cfg, &line);
                    tracing::info!("dynamically resolved QQ path from running process -> {}", line);
                    return Ok(p);
                }
            }
        }

        // 2. Query Windows Registry for official QQNT install location
        if let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\WOW6432Node\\Tencent\\QQNT','HKCU:\\Software\\Tencent\\QQNT' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Install -First 1)",
            ])
            .output()
        {
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

        // 3. Fallback to standard ProgramFiles environment variables
        for env_var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(prog) = std::env::var(env_var) {
                let candidate = PathBuf::from(prog).join("Tencent").join("QQNT").join("QQ.exe");
                if candidate.is_file() {
                    let path_str = candidate.to_string_lossy().to_string();
                    let _ = std::fs::create_dir_all(napcat_dir.join("config"));
                    let _ = std::fs::write(&cfg, &path_str);
                    tracing::info!("resolved QQ path from standard environment -> {}", path_str);
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

pub fn sync_qqnt_patch(qq_path: &Path, patch_pkg: &Path) {
    let qq_dir = match qq_path.parent() {
        Some(d) => d,
        None => return,
    };

    let mut candidate = qq_dir.join("resources").join("app").join("package.json");
    if !candidate.exists() {
        let versions_dir = qq_dir.join("versions");
        if let Ok(entries) = std::fs::read_dir(versions_dir) {
            for entry in entries.flatten() {
                let p = entry.path().join("resources").join("app").join("package.json");
                if p.exists() {
                    candidate = p;
                    break;
                }
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string(&candidate) {
        if let Ok(source) = serde_json::from_str::<serde_json::Value>(&content) {
            let mut patch = serde_json::Map::new();
            for key in [
                "name", "verHash", "version", "linuxVersion", "linuxVerHash",
                "private", "description", "productName", "author", "homepage",
                "sideEffects", "bin", "buildVersion",
            ] {
                if let Some(v) = source.get(key) {
                    patch.insert(key.to_string(), v.clone());
                }
            }
            patch.insert("main".to_string(), serde_json::json!("./loadNapCat.js"));
            patch.insert("isPureShell".to_string(), serde_json::json!(true));
            patch.insert("isByteCodeShell".to_string(), serde_json::json!(true));
            patch.insert("platform".to_string(), serde_json::json!("win32"));
            patch.insert("eleArch".to_string(), serde_json::json!("x64"));

            let patched_json = serde_json::Value::Object(patch);
            if let Ok(serialized) = serde_json::to_string_pretty(&patched_json) {
                let _ = std::fs::write(patch_pkg, serialized);
                tracing::info!("synced qqnt.json with installed QQNT metadata at {}", candidate.display());
            }
        }
    }
}
