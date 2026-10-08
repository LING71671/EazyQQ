//! Staged ZIP installs preserve mutable account files and support rollback.
use std::io::{Cursor, Read};
use std::path::Path;

pub fn stage_and_install(bytes: &[u8], staging: &Path, destination: &Path) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root = staging.join(stamp.to_string());
    let extracted = root.join("extracted");
    let backups = root.join("backup");
    std::fs::create_dir_all(&extracted).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if archive.len() > 40_000 {
        return Err("Protocol archive has too many entries".into());
    }
    let mut expanded = 0u64;
    let mut package_root = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let name = entry
            .enclosed_name()
            .ok_or("Protocol archive contains an unsafe path")?;
        if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("Protocol archive contains a symbolic link".into());
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("Archive size overflow")?;
        if expanded > 512 * 1024 * 1024 {
            return Err("Protocol archive exceeds 512 MB".into());
        }
        let path = extracted.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry.by_ref().take(512 * 1024 * 1024 + 1), &mut output)
            .map_err(|e| e.to_string())?;
        if path.file_name().is_some_and(|name| name == "napcat.mjs") {
            package_root = path.parent().map(Path::to_path_buf);
        }
    }
    let source = package_root.ok_or("Protocol archive is missing napcat.mjs")?;
    for name in ["NapCatWinBootMain.exe", "NapCatWinBootHook.dll"] {
        if !source.join(name).is_file() {
            return Err(format!("Protocol archive is missing {}", name));
        }
    }
    let mut files = Vec::new();
    collect(&source, &source, &mut files)?;
    let mut changed = Vec::new();
    for relative in files {
        let target = destination.join(&relative);
        let backup = backups.join(&relative);
        let result = (|| {
            if target.exists() {
                if let Some(parent) = backup.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(&target, &backup).map_err(|e| e.to_string())?;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let incoming = target.with_extension("eazyqq-incoming");
            std::fs::copy(source.join(&relative), &incoming).map_err(|e| e.to_string())?;
            std::fs::rename(&incoming, &target).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            for relative in changed.iter().rev() {
                let backup = backups.join(relative);
                if backup.exists() {
                    let _ = std::fs::copy(backup, destination.join(relative));
                } else {
                    let _ = std::fs::remove_file(destination.join(relative));
                }
            }
            return Err(format!(
                "Protocol install failed: {}. Backup retained at {}",
                error,
                backups.display()
            ));
        }
        changed.push(relative);
    }
    Ok(())
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let first = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy();
        if !crate::services::protocol::layout::is_immutable_resource(&first) {
            continue;
        }
        if !crate::services::protocol::layout::is_immutable_resource(
            &entry.file_name().to_string_lossy(),
        ) {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, files)?;
        } else {
            files.push(relative.to_path_buf());
        }
    }
    Ok(())
}
