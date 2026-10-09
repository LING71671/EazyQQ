//! Resolve paths in the writer's filesystem view before detached process handoff.
use std::path::{Path, PathBuf};

/// Existing files/directories are resolved by handle, including namespace redirection.
pub fn physical_path(path: &Path) -> Result<PathBuf, String> {
    let resolved = std::fs::canonicalize(path)
        .map_err(|error| format!("Cannot resolve physical path {}: {error}", path.display()))?;
    process_path(&resolved)
}

/// Resolve the parent before handing off a filename that may not exist yet.
pub fn output_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
    let parent = absolute.parent().ok_or("Output path has no parent")?;
    let name = absolute.file_name().ok_or("Output path has no filename")?;
    Ok(physical_path(parent)?.join(name))
}

pub fn physical_directory(path: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
    physical_path(path)
}

/// Electron's package.main and native launcher consumers expect DOS/UNC paths.
#[cfg(windows)]
pub fn process_path(path: &Path) -> Result<PathBuf, String> {
    use std::path::{Component, Prefix};
    if !path.is_absolute() {
        return Err("Process handoff requires an absolute Windows path".into());
    }
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err("Process handoff requires an absolute Windows path".into());
    };
    let mut normalized = match prefix.kind() {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
            PathBuf::from(format!("{}:\\", drive as char))
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            let mut value = PathBuf::from(r"\\");
            value.push(server);
            value.push(share);
            value
        }
        _ => return Err("Device namespace paths cannot be handed to a process".into()),
    };
    for component in components {
        if component != Component::RootDir {
            normalized.push(component.as_os_str());
        }
    }
    Ok(normalized)
}

#[cfg(not(windows))]
pub fn process_path(path: &Path) -> Result<PathBuf, String> {
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn verbatim_disk_and_unc_keep_their_volume_identity() {
        assert_eq!(
            process_path(Path::new(r"\\?\C:\路径\entry.cjs")).unwrap(),
            Path::new(r"C:\路径\entry.cjs")
        );
        assert_eq!(
            process_path(Path::new(r"\\?\UNC\server\share\entry.cjs")).unwrap(),
            Path::new(r"\\server\share\entry.cjs")
        );
        assert!(process_path(Path::new(r"\\.\PhysicalDrive0")).is_err());
    }

    #[test]
    fn output_handoff_resolves_existing_parent_without_creating_the_file() {
        let dir = crate::services::logging::workspace_root().join("physical-output");
        let root = physical_directory(&dir).unwrap();
        let logical = dir.join("future 中文.json");
        let output = output_path(&logical).unwrap();
        assert_eq!(output, root.join("future 中文.json"));
        assert!(!output.exists());
        assert!(physical_path(&logical).is_err());
    }
}
