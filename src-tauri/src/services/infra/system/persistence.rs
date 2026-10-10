//! Atomic JSON writes and process-wide OS file locks for shared registries.
use fs2::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct FileLock {
    _file: std::fs::File,
}
impl FileLock {
    pub fn try_acquire(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.try_lock_exclusive().map_err(|_| {
            "An account worker is already running in another GUI or CLI process".to_string()
        })?;
        Ok(Self { _file: file })
    }
    pub fn shared(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        FileExt::lock_shared(&file).map_err(|e| e.to_string())?;
        Ok(Self { _file: file })
    }
    pub fn acquire(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.lock_exclusive().map_err(|e| e.to_string())?;
        Ok(Self { _file: file })
    }
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or("Destination has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp: PathBuf = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(750);
        loop {
            match std::fs::rename(&temp, path) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    // Readers without FILE_SHARE_DELETE can briefly prevent replacement.
                    let transient = cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33));
                    if !transient || std::time::Instant::now() >= deadline {
                        return Err(error.to_string());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

#[cfg(all(test,windows))]
mod tests {
    #[test]
    fn atomic_json_replacement_survives_a_short_lived_reader() {
        use std::os::windows::fs::OpenOptionsExt;
        let root=crate::services::logging::workspace_root().join("json-reader-sharing");
        std::fs::create_dir_all(&root).unwrap();
        let path=root.join("state.json");
        super::write_json(&path,&serde_json::json!({"selected":"10001"})).unwrap();
        let reader=std::fs::OpenOptions::new().read(true).share_mode(3).open(&path).unwrap();
        let release=std::thread::spawn(move || {std::thread::sleep(std::time::Duration::from_millis(90));drop(reader);});
        super::write_json(&path,&serde_json::json!({"selected":"10002"})).unwrap();
        release.join().unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()["selected"],"10002");
    }
}
